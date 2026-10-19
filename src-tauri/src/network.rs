//! Owns the loopback SOCKS proxy that every e621 request of the app goes through, and the in-app
//! tunnel behind it when one is configured.

use std::error::Error;

use serde::Serialize;
use tokio::sync::{Mutex, watch};

use crate::{
    secrets,
    tunnel::{
        Tunnel,
        mullvad::{Mullvad, RegisteredDevice},
        socks::{Route, SocksServer},
    },
};

pub const USER_AGENT: &str = concat!("Tailgate/", env!("CARGO_PKG_VERSION"), " (by Nyabi on e621)");

/// Until the connection settings screen exists (report §11, step 9), the relay is fixed.
const DEFAULT_RELAY_LOCATION: &str = "jp-tyo";

/// What the UI needs to know before it may load anything remote.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum Connection {
    Direct,
    Connecting,
    Tunnel { relay: String },
    Failed { reason: String },
}

pub struct Network {
    socks: SocksServer,
    /// Every request to e621 goes through this client, and so through the proxy and its route.
    http: reqwest::Client,
    tunnel: Mutex<Option<Tunnel>>,
    connection: watch::Sender<Connection>,
}

impl Network {
    /// Binds the proxy. With a saved Mullvad device it starts out blocked, so nothing leaks
    /// directly before [`Network::connect_saved`] brings the tunnel up.
    pub async fn start() -> std::io::Result<Self> {
        let has_device = matches!(secrets::get(secrets::MULLVAD_DEVICE), Ok(Some(_)));
        let (route, connection) = if has_device {
            (Route::Blocked, Connection::Connecting)
        } else {
            (Route::Direct, Connection::Direct)
        };
        let socks = SocksServer::bind(route).await?;
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .proxy(reqwest::Proxy::all(format!("socks5h://{}", socks.address())).map_err(std::io::Error::other)?)
            .build()
            .map_err(std::io::Error::other)?;
        Ok(Self { socks, http, tunnel: Mutex::new(None), connection: watch::Sender::new(connection) })
    }

    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    pub fn connection(&self) -> Connection {
        self.connection.borrow().clone()
    }

    pub fn watch_connection(&self) -> watch::Receiver<Connection> {
        self.connection.subscribe()
    }

    /// Brings up the tunnel for the saved Mullvad device, if any. On failure the proxy stays
    /// blocked and the failure is published as the connection state.
    pub async fn connect_saved(&self) {
        if let Err(error) = self.try_connect_saved().await {
            tracing::error!("tunnel failed to start: {error}");
            self.connection.send_replace(Connection::Failed { reason: error.to_string() });
        }
    }

    async fn try_connect_saved(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let Some(device) = secrets::get(secrets::MULLVAD_DEVICE)? else {
            return Ok(());
        };
        let device: RegisteredDevice = serde_json::from_str(&device)?;
        // Talks to Mullvad directly: this is what sets up the route the proxy would need.
        let mullvad = Mullvad::new(reqwest::Client::builder().user_agent(USER_AGENT).build()?);
        let relays = mullvad.relays().await?;
        let relay = relays
            .iter()
            .find(|relay| relay.active && relay.location.starts_with(DEFAULT_RELAY_LOCATION))
            .ok_or("no active relay at the default location")?;

        let tunnel = Tunnel::start(&device.tunnel_config(relay)?).await?;
        self.socks.set_route(Route::Tunnel(tunnel.stack().clone()));
        tracing::info!("tunnel up via {}", relay.hostname);
        self.connection.send_replace(Connection::Tunnel { relay: relay.hostname.clone() });
        if let Some(previous) = self.tunnel.lock().await.replace(tunnel) {
            previous.stop().await;
        }
        Ok(())
    }
}
