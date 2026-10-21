//! The few Mullvad API calls needed to turn an account number into a WireGuard configuration.
//! Paths and payloads follow mullvadvpn-app's `mullvad-api` crate.

use std::{collections::BTreeMap, net::Ipv4Addr};

use base64::{Engine, engine::general_purpose::STANDARD};
use gotatun::x25519::{PublicKey, StaticSecret};
use serde::{Deserialize, Serialize};

use super::TunnelConfig;

const API: &str = "https://api.mullvad.net";
/// Mullvad's in-tunnel resolver (`wireguard.ipv4_gateway` in the relay list).
const TUNNEL_DNS: Ipv4Addr = Ipv4Addr::new(10, 64, 0, 1);
const WIREGUARD_PORT: u16 = 51820;

/// Error codes of the Mullvad API, as mullvadvpn-app's `mullvad-api` names them.
pub const INVALID_ACCOUNT: &str = "INVALID_ACCOUNT";
pub const MAX_DEVICES_REACHED: &str = "MAX_DEVICES_REACHED";
pub const DEVICE_NOT_FOUND: &str = "DEVICE_NOT_FOUND";

#[derive(Debug, thiserror::Error)]
pub enum MullvadError {
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("Mullvad API returned {status} ({code})")]
    Api { status: u16, code: String },
    #[error("unexpected Mullvad response: {0}")]
    Response(String),
}

impl MullvadError {
    pub fn is_code(&self, expected: &str) -> bool {
        matches!(self, Self::Api { code, .. } if code == expected)
    }
}

/// A WireGuard key registered on the account. Kept in the credential store so restarts reuse it
/// instead of using up another of the account's device slots.
#[derive(Serialize, Deserialize)]
pub struct RegisteredDevice {
    pub id: String,
    pub name: String,
    private_key: String,
    pub ipv4: Ipv4Addr,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Relay {
    pub hostname: String,
    pub location: String,
    pub active: bool,
    pub ipv4_addr_in: Ipv4Addr,
    pub public_key: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Location {
    pub country: String,
    pub city: String,
}

/// The WireGuard part of Mullvad's relay list, with the names of the locations it uses.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RelayList {
    pub relays: Vec<Relay>,
    /// Keyed by location code, as in [`Relay::location`] (`jp-tyo`).
    pub locations: BTreeMap<String, Location>,
}

/// A fetch of the relay list made with the entity tag of the copy at hand.
pub enum Fetched {
    Changed {
        list: RelayList,
        etag: Option<String>,
    },
    NotModified,
}

pub struct Mullvad {
    http: reqwest::Client,
    base: String,
}

impl Mullvad {
    pub fn new(http: reqwest::Client) -> Self {
        Self {
            http,
            base: API.to_owned(),
        }
    }

    #[cfg(test)]
    pub fn with_base(http: reqwest::Client, base: String) -> Self {
        Self {
            http,
            base: base.trim_end_matches('/').to_owned(),
        }
    }

    pub async fn access_token(&self, account_number: &str) -> Result<String, MullvadError> {
        #[derive(Deserialize)]
        struct Token {
            access_token: String,
        }
        let response = self
            .http
            .post(format!("{}/auth/v1/token", self.base))
            .json(&serde_json::json!({ "account_number": account_number }))
            .send()
            .await?;
        Ok(checked(response).await?.json::<Token>().await?.access_token)
    }

    pub async fn register_device(
        &self,
        access_token: &str,
    ) -> Result<RegisteredDevice, MullvadError> {
        #[derive(Deserialize)]
        struct Created {
            id: String,
            name: String,
            ipv4_address: String,
        }
        let mut secret = [0u8; 32];
        getrandom::fill(&mut secret).expect("the OS random source is unavailable");
        let private_key = StaticSecret::from(secret);
        let public_key = PublicKey::from(&private_key);

        let response = self
            .http
            .post(format!("{}/accounts/v1/devices", self.base))
            .bearer_auth(access_token)
            .json(&serde_json::json!({ "pubkey": STANDARD.encode(public_key.as_bytes()), "hijack_dns": false }))
            .send()
            .await?;
        let created: Created = checked(response).await?.json().await?;
        let ipv4 = created
            .ipv4_address
            .split('/')
            .next()
            .and_then(|ip| ip.parse().ok())
            .ok_or_else(|| {
                MullvadError::Response(format!("bad tunnel address {}", created.ipv4_address))
            })?;
        Ok(RegisteredDevice {
            id: created.id,
            name: created.name,
            private_key: STANDARD.encode(secret),
            ipv4,
        })
    }

    pub async fn remove_device(
        &self,
        access_token: &str,
        device_id: &str,
    ) -> Result<(), MullvadError> {
        let response = self
            .http
            .delete(format!("{}/accounts/v1/devices/{device_id}", self.base))
            .bearer_auth(access_token)
            .send()
            .await?;
        checked(response).await.map(|_| ())
    }

    /// The account's paid-until time, as the API gives it (RFC 3339).
    pub async fn account_expiry(&self, access_token: &str) -> Result<String, MullvadError> {
        #[derive(Deserialize)]
        struct AccountData {
            expiry: String,
        }
        let response = self
            .http
            .get(format!("{}/accounts/v1/accounts/me", self.base))
            .bearer_auth(access_token)
            .send()
            .await?;
        Ok(checked(response).await?.json::<AccountData>().await?.expiry)
    }

    pub async fn relays(&self) -> Result<RelayList, MullvadError> {
        match self.relays_since(None).await? {
            Fetched::Changed { list, .. } => Ok(list),
            Fetched::NotModified => Err(MullvadError::Response(
                "304 for an unconditional request".into(),
            )),
        }
    }

    /// With the tag of a saved copy, an unchanged list costs no download (about 600 KB otherwise).
    pub async fn relays_since(&self, etag: Option<&str>) -> Result<Fetched, MullvadError> {
        #[derive(Deserialize)]
        struct Response {
            locations: BTreeMap<String, Location>,
            wireguard: WireGuard,
        }
        #[derive(Deserialize)]
        struct WireGuard {
            relays: Vec<Relay>,
        }
        let mut request = self.http.get(format!("{}/app/v1/relays", self.base));
        if let Some(etag) = etag {
            request = request.header(reqwest::header::IF_NONE_MATCH, etag);
        }
        let response = request.send().await?;
        if response.status() == reqwest::StatusCode::NOT_MODIFIED {
            return Ok(Fetched::NotModified);
        }
        let response = checked(response).await?;
        let etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let body: Response = response.json().await?;
        Ok(Fetched::Changed {
            list: RelayList {
                relays: body.wireguard.relays,
                locations: body.locations,
            },
            etag,
        })
    }
}

impl RegisteredDevice {
    pub fn tunnel_config(&self, relay: &Relay) -> Result<TunnelConfig, MullvadError> {
        let private_key = super::config::parse_key("PrivateKey", &self.private_key)
            .map_err(|error| MullvadError::Response(error.to_string()))?;
        let peer_public_key =
            super::config::parse_key("PublicKey", &relay.public_key).map_err(|error| {
                MullvadError::Response(format!("relay {}: {error}", relay.hostname))
            })?;
        Ok(TunnelConfig {
            private_key,
            address: self.ipv4,
            dns: TUNNEL_DNS,
            peer_public_key,
            preshared_key: None,
            endpoint: format!("{}:{WIREGUARD_PORT}", relay.ipv4_addr_in),
            keepalive: Some(25),
        })
    }
}

async fn checked(response: reqwest::Response) -> Result<reqwest::Response, MullvadError> {
    if response.status().is_success() {
        return Ok(response);
    }
    #[derive(Deserialize)]
    struct ApiError {
        code: Option<String>,
    }
    let status = response.status().as_u16();
    let code = response
        .json::<ApiError>()
        .await
        .ok()
        .and_then(|body| body.code)
        .unwrap_or_else(|| "no error code".into());
    Err(MullvadError::Api { status, code })
}
