//! Sends e621 API requests: one start per second, the e621 User-Agent, Basic auth, and retries
//! for rate limiting (docs/report.md §5).

use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

use reqwest::{StatusCode, header};
use serde::de::DeserializeOwned;
use tokio::{
    sync::{Mutex, watch},
    time::{Instant, sleep, sleep_until},
};

use super::ApiError;
use crate::network::{self, UserAgent};

/// e621 asks for at most one request per second over time; two per second is the hard limit.
const MIN_INTERVAL: Duration = Duration::from_secs(1);
const MAX_RETRIES: u32 = 3;
const FIRST_BACKOFF: Duration = Duration::from_secs(1);
/// A longer `Retry-After` is answered with an error instead of a request that hangs that long.
const MAX_RETRY_AFTER: Duration = Duration::from_secs(30);

pub struct Credentials {
    pub username: String,
    pub api_key: String,
}

pub(super) struct Client {
    http: reqwest::Client,
    base: String,
    next_start: Mutex<Instant>,
    credentials: RwLock<Option<Arc<Credentials>>>,
    /// Kept in step with the credentials for the requests that do not go through this client.
    user_agent: UserAgent,
    /// The credentials e621 last answered 401 to while they were the current ones.
    rejected: watch::Sender<Option<Arc<Credentials>>>,
}

impl Client {
    pub(super) fn new(http: reqwest::Client, base: String, user_agent: UserAgent) -> Self {
        Self {
            http,
            base,
            next_start: Mutex::new(Instant::now()),
            credentials: RwLock::new(None),
            user_agent,
            rejected: watch::Sender::new(None),
        }
    }

    pub(super) fn credentials(&self) -> Option<Arc<Credentials>> {
        self.credentials
            .read()
            .expect("credentials lock poisoned")
            .clone()
    }

    pub(super) fn set_credentials(&self, credentials: Option<Arc<Credentials>>) {
        let mut current = self.credentials.write().expect("credentials lock poisoned");
        self.user_agent.set_user(
            credentials
                .as_ref()
                .map(|credentials| credentials.username.as_str()),
        );
        *current = credentials;
    }

    pub(super) fn watch_rejections(&self) -> watch::Receiver<Option<Arc<Credentials>>> {
        self.rejected.subscribe()
    }

    /// With the current credentials. A 401 to them is also published to
    /// [`Client::watch_rejections`], unless they were replaced while the request was out.
    pub(super) async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<T, ApiError> {
        let credentials = self.credentials();
        let result = self.get_as(path, query, credentials.as_deref()).await;
        if let (Err(ApiError::Unauthorized), Some(credentials)) = (&result, credentials)
            && self
                .credentials()
                .is_some_and(|current| Arc::ptr_eq(&current, &credentials))
        {
            self.rejected.send_replace(Some(credentials));
        }
        result
    }

    /// With the given credentials instead of the current ones, as when checking them before
    /// they are saved.
    pub(super) async fn get_as<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, &str)],
        credentials: Option<&Credentials>,
    ) -> Result<T, ApiError> {
        let mut retries = 0;
        loop {
            self.wait_for_turn().await;
            let response = self
                .request(path, query, credentials)
                .send()
                .await
                .map_err(|error| {
                    tracing::warn!("API request {path} failed: {error}");
                    ApiError::Network
                })?;
            let status = response.status();
            if matches!(
                status,
                StatusCode::TOO_MANY_REQUESTS | StatusCode::SERVICE_UNAVAILABLE
            ) {
                let delay = retry_after(&response).unwrap_or(FIRST_BACKOFF * 2u32.pow(retries));
                if retries < MAX_RETRIES && delay <= MAX_RETRY_AFTER {
                    tracing::info!("API answered {status} for {path}, retrying in {delay:?}");
                    retries += 1;
                    sleep(delay).await;
                    continue;
                }
            }
            if !status.is_success() {
                tracing::warn!("API answered {status} for {path}");
                return Err(ApiError::from_status(status));
            }
            let body = response.bytes().await.map_err(|error| {
                tracing::warn!("API response body for {path} failed: {error}");
                ApiError::Network
            })?;
            return serde_json::from_slice(&body).map_err(|error| {
                tracing::warn!("API response for {path} did not decode: {error}");
                ApiError::Decode
            });
        }
    }

    /// Spaces request starts by [`MIN_INTERVAL`]. The lock is held only to claim a start time, so
    /// a slow response does not hold back the next request.
    async fn wait_for_turn(&self) {
        let start = {
            let mut next_start = self.next_start.lock().await;
            let start = (*next_start).max(Instant::now());
            *next_start = start + MIN_INTERVAL;
            start
        };
        sleep_until(start).await;
    }

    fn request(
        &self,
        path: &str,
        query: &[(&str, &str)],
        credentials: Option<&Credentials>,
    ) -> reqwest::RequestBuilder {
        let user_agent = credentials.map_or_else(
            || network::USER_AGENT.to_owned(),
            |credentials| network::user_agent_for(&credentials.username),
        );
        let request = self
            .http
            .get(format!("{}{path}", self.base))
            .query(query)
            .header(header::USER_AGENT, user_agent);
        match credentials {
            Some(credentials) => {
                request.basic_auth(&credentials.username, Some(&credentials.api_key))
            }
            None => request,
        }
    }
}

/// Only the delay-seconds form; a date falls back to the backoff.
fn retry_after(response: &reqwest::Response) -> Option<Duration> {
    let seconds = response
        .headers()
        .get(header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()?;
    Some(Duration::from_secs(seconds))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn spaces_request_starts_a_second_apart() {
        let client = Client::new(reqwest::Client::new(), String::new(), UserAgent::new());
        let started = Instant::now();
        let turn = async || {
            client.wait_for_turn().await;
            started.elapsed()
        };
        let (a, b, c) = tokio::join!(turn(), turn(), turn());
        let mut starts = [a, b, c];
        starts.sort();
        assert_eq!(starts, [0, 1, 2].map(Duration::from_secs));

        sleep(Duration::from_secs(5)).await;
        assert_eq!(turn().await, Duration::from_secs(7));
    }
}
