//! The e621 API. Only Rust calls it: a request from the webview would carry a browser
//! User-Agent and bypass the throttle (docs/report.md §1).

mod client;
mod endpoints;
pub mod models;

use serde::Serialize;

use client::Client;
pub use client::Credentials;
pub use endpoints::{PAGE_SIZE, PostSearch};

const BASE: &str = "https://e621.net/";

/// Why a request failed. The UI words it; the details only go to the log.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, thiserror::Error)]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum ApiError {
    #[error("the credentials were rejected")]
    Unauthorized,
    #[error("access denied")]
    Forbidden,
    #[error("not found")]
    NotFound,
    #[error("past the last page reachable by number")]
    PageLimit,
    #[error("rate limited")]
    RateLimited,
    #[error("e621 is unavailable")]
    Unavailable,
    #[error("could not reach e621")]
    Network,
    #[error("unexpected response")]
    Decode,
    #[error("unexpected status {status}")]
    Http { status: u16 },
}

impl ApiError {
    fn from_status(status: reqwest::StatusCode) -> Self {
        match status.as_u16() {
            401 => Self::Unauthorized,
            403 => Self::Forbidden,
            404 => Self::NotFound,
            410 => Self::PageLimit,
            429 => Self::RateLimited,
            503 => Self::Unavailable,
            status => Self::Http { status },
        }
    }
}

pub struct Api {
    client: Client,
}

impl Api {
    /// `http` should be the app's e621 client, so API requests take the same route and kill
    /// switch as everything else.
    pub fn new(http: reqwest::Client) -> Self {
        Self::with_base(http, BASE.to_owned())
    }

    fn with_base(http: reqwest::Client, base: String) -> Self {
        Self {
            client: Client::new(http, base),
        }
    }

    pub fn set_credentials(&self, credentials: Option<Credentials>) {
        self.client.set_credentials(credentials);
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        net::Ipv4Addr,
        sync::{Arc, Mutex},
        time::Duration,
    };

    use serde_json::{Value, json};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::mpsc,
        time::Instant,
    };

    use super::*;
    use crate::network::USER_AGENT;

    #[derive(Debug)]
    struct Received {
        at: Instant,
        target: String,
        user_agent: String,
        authorization: Option<String>,
    }

    fn json_response(status: &str, body: &str) -> String {
        format!(
            "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    fn ok(body: &Value) -> String {
        json_response("200 OK", &body.to_string())
    }

    /// An e621 stand-in that answers each request with the next of `responses`.
    async fn serve(responses: Vec<String>) -> (Api, mpsc::UnboundedReceiver<Received>) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let base = format!("http://{}/", listener.local_addr().unwrap());
        let responses = Arc::new(Mutex::new(VecDeque::from(responses)));
        let (sender, received) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                let responses = Arc::clone(&responses);
                let sender = sender.clone();
                tokio::spawn(async move {
                    let mut head = Vec::new();
                    let mut buffer = [0; 1024];
                    while !head.ends_with(b"\r\n\r\n") {
                        let read = stream.read(&mut buffer).await.unwrap();
                        assert_ne!(read, 0, "the client closed before finishing its request");
                        head.extend_from_slice(&buffer[..read]);
                    }
                    let head = String::from_utf8(head).unwrap();
                    let header = |name: &str| {
                        head.lines().find_map(|line| {
                            let (key, value) = line.split_once(": ")?;
                            key.eq_ignore_ascii_case(name).then(|| value.to_owned())
                        })
                    };
                    sender
                        .send(Received {
                            at: Instant::now(),
                            target: head.split(' ').nth(1).unwrap().to_owned(),
                            user_agent: header("user-agent").unwrap_or_default(),
                            authorization: header("authorization"),
                        })
                        .unwrap();
                    let response = responses
                        .lock()
                        .unwrap()
                        .pop_front()
                        .expect("more requests than scripted responses");
                    stream.write_all(response.as_bytes()).await.unwrap();
                });
            }
        });
        let http = reqwest::Client::builder().no_proxy().build().unwrap();
        (Api::with_base(http, base), received)
    }

    fn anonymous() -> Value {
        serde_json::from_str(include_str!("../../tests/fixtures/users_me_anonymous.json")).unwrap()
    }

    /// `ids.len()` copies of a fixture post with the given ids.
    fn posts(ids: impl IntoIterator<Item = u64>) -> Value {
        let fixture: Value =
            serde_json::from_str(include_str!("../../tests/fixtures/posts_image.json")).unwrap();
        let post = &fixture["posts"][0];
        let posts: Vec<Value> = ids
            .into_iter()
            .map(|id| {
                let mut post = post.clone();
                post["id"] = json!(id);
                post
            })
            .collect();
        json!({ "posts": posts })
    }

    fn drain(received: &mut mpsc::UnboundedReceiver<Received>) -> Vec<Received> {
        let mut all = Vec::new();
        while let Ok(request) = received.try_recv() {
            all.push(request);
        }
        all
    }

    #[tokio::test(start_paused = true)]
    async fn identifies_the_user_in_the_user_agent_and_authorization() {
        let (api, mut received) = serve(vec![ok(&anonymous()), ok(&anonymous())]).await;
        assert_eq!(api.me().await, Ok(None));
        api.set_credentials(Some(Credentials {
            username: "alice".into(),
            api_key: "key".into(),
        }));
        api.me().await.unwrap();
        let requests = drain(&mut received);
        assert_eq!(requests[0].target, "/users/me.json");
        assert_eq!(requests[0].user_agent, USER_AGENT);
        assert_eq!(requests[0].authorization, None);
        assert_eq!(
            requests[1].user_agent,
            format!("Tailgate/{} (by alice on e621)", env!("CARGO_PKG_VERSION"))
        );
        // base64("alice:key")
        assert_eq!(
            requests[1].authorization.as_deref(),
            Some("Basic YWxpY2U6a2V5")
        );
    }

    #[tokio::test(start_paused = true)]
    async fn retries_after_the_delay_the_server_asks_for() {
        let busy = "HTTP/1.1 503 Service Unavailable\r\nretry-after: 3\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";
        let (api, mut received) = serve(vec![busy.into(), ok(&anonymous())]).await;
        assert_eq!(api.me().await, Ok(None));
        let requests = drain(&mut received);
        assert_eq!(requests.len(), 2);
        assert!(requests[1].at - requests[0].at >= Duration::from_secs(3));
    }

    #[tokio::test(start_paused = true)]
    async fn gives_up_after_three_retries() {
        let limited = json_response("429 Too Many Requests", "{}");
        let (api, mut received) = serve(vec![limited; 4]).await;
        assert_eq!(api.me().await, Err(ApiError::RateLimited));
        assert_eq!(drain(&mut received).len(), 4);
    }

    #[tokio::test(start_paused = true)]
    async fn does_not_wait_out_long_retry_after_delays() {
        let busy = "HTTP/1.1 503 Service Unavailable\r\nretry-after: 600\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";
        let (api, mut received) = serve(vec![busy.into()]).await;
        assert_eq!(api.me().await, Err(ApiError::Unavailable));
        assert_eq!(drain(&mut received).len(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn reports_failures_as_codes() {
        let (api, _received) = serve(vec![
            json_response("401 Unauthorized", "{}"),
            json_response("410 Gone", "{}"),
            json_response("418 I'm a teapot", "{}"),
            json_response("200 OK", "not json"),
        ])
        .await;
        assert_eq!(api.me().await, Err(ApiError::Unauthorized));
        assert_eq!(api.me().await, Err(ApiError::PageLimit));
        assert_eq!(api.me().await, Err(ApiError::Http { status: 418 }));
        assert_eq!(api.me().await, Err(ApiError::Decode));
    }

    #[tokio::test]
    async fn reports_unreachable_servers_as_network_errors() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let base = format!("http://{}/", listener.local_addr().unwrap());
        drop(listener);
        let api = Api::with_base(reqwest::Client::builder().no_proxy().build().unwrap(), base);
        assert_eq!(api.me().await, Err(ApiError::Network));
    }

    #[tokio::test(start_paused = true)]
    async fn pages_by_the_lowest_id_and_skips_posts_seen_before() {
        let first: Vec<u64> = (101..=200).rev().collect();
        let (api, mut received) = serve(vec![ok(&posts(first)), ok(&posts([101, 100, 99]))]).await;
        let mut search = PostSearch::new("  wolf   solo ");
        assert_eq!(search.next_page(&api).await.unwrap().len(), PAGE_SIZE);
        assert!(!search.is_done());
        let ids: Vec<u64> = search
            .next_page(&api)
            .await
            .unwrap()
            .iter()
            .map(|post| post.id)
            .collect();
        assert_eq!(ids, [100, 99]);
        assert!(search.is_done());
        assert!(search.next_page(&api).await.unwrap().is_empty());

        let requests = drain(&mut received);
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].target, "/posts.json?tags=wolf+solo&limit=100");
        assert_eq!(
            requests[1].target,
            "/posts.json?tags=wolf+solo&limit=100&page=b101"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn pages_by_number_when_the_search_sets_an_order() {
        let (api, mut received) = serve(vec![ok(&posts(1..=100)), ok(&posts(101..=150))]).await;
        let mut search = PostSearch::new("wolf Order:score");
        search.next_page(&api).await.unwrap();
        search.next_page(&api).await.unwrap();
        assert!(search.is_done());
        let requests = drain(&mut received);
        assert_eq!(
            requests[0].target,
            "/posts.json?tags=wolf+Order%3Ascore&limit=100&page=1"
        );
        assert_eq!(
            requests[1].target,
            "/posts.json?tags=wolf+Order%3Ascore&limit=100&page=2"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn stops_at_the_last_page_reachable_by_number() {
        let (api, mut received) = serve(vec![ok(&posts(1..=100))]).await;
        let mut search = PostSearch::new("order:random");
        search.skip_to_page(750);
        search.next_page(&api).await.unwrap();
        assert!(search.is_done());
        assert!(drain(&mut received)[0].target.ends_with("&page=750"));
    }

    #[tokio::test(start_paused = true)]
    async fn asks_for_the_same_page_again_after_an_error() {
        let (api, mut received) = serve(vec![
            json_response("500 Internal Server Error", "{}"),
            ok(&posts([5])),
        ])
        .await;
        let mut search = PostSearch::new("wolf");
        assert!(search.next_page(&api).await.is_err());
        assert_eq!(search.next_page(&api).await.unwrap()[0].id, 5);
        let requests = drain(&mut received);
        assert_eq!(requests[0].target, requests[1].target);
    }
}
