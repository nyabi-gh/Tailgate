//! Owns the loopback SOCKS proxy that every e621 request of the app goes through, and the in-app
//! tunnel behind it when one is configured.

use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

use serde::Serialize;
use tokio::sync::{Mutex, watch};

use crate::{
    secrets,
    tunnel::{
        Tunnel, TunnelError,
        mullvad::{Mullvad, MullvadError, RegisteredDevice},
        socks::{Route, SocksServer},
    },
};

pub const USER_AGENT: &str = concat!(
    "Tailgate/",
    env!("CARGO_PKG_VERSION"),
    " (by Nyabi on e621)"
);

/// What e621 asks requests to carry: the project and the e621 user it runs for.
pub fn user_agent_for(username: &str) -> String {
    format!(
        "Tailgate/{} (by {username} on e621)",
        env!("CARGO_PKG_VERSION")
    )
}

/// The User-Agent of the signed-in user, shared by everything that requests from e621 outside
/// the API client (media).
#[derive(Clone)]
pub struct UserAgent(Arc<RwLock<String>>);

impl UserAgent {
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(USER_AGENT.to_owned())))
    }

    pub fn get(&self) -> String {
        self.0.read().expect("user agent lock poisoned").clone()
    }

    pub fn set_user(&self, username: Option<&str>) {
        *self.0.write().expect("user agent lock poisoned") =
            username.map_or_else(|| USER_AGENT.to_owned(), user_agent_for);
    }
}

impl Default for UserAgent {
    fn default() -> Self {
        Self::new()
    }
}

/// Until the connection settings screen exists (report §12, step 9), the relay is fixed.
const DEFAULT_RELAY_LOCATION: &str = "jp-tyo";
/// Longer than the tunnel's own DNS and connect limits together (25 s), so those report first.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
/// Between two reads, not for the whole response: a large original may take minutes.
const READ_TIMEOUT: Duration = Duration::from_secs(30);
/// Whole requests, the relay list (about 600 KB) included.
const MULLVAD_TIMEOUT: Duration = Duration::from_secs(30);

/// What the UI needs to know before it may load anything remote.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum Connection {
    Direct,
    Connecting,
    Tunnel { relay: String },
    Failed { code: FailureCode },
}

/// Why no route could be opened. The UI words it; the details only go to the log.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FailureCode {
    CredentialStore,
    SavedDevice,
    RelayList,
    NoRelay,
    Tunnel,
    Handshake,
}

#[derive(Debug, thiserror::Error)]
enum ConnectError {
    #[error("could not read the credential store: {0}")]
    CredentialStore(#[from] keyring_core::Error),
    #[error("the saved Mullvad device is unreadable: {0}")]
    SavedDevice(#[from] serde_json::Error),
    #[error("could not get the relay list: {0}")]
    RelayList(MullvadError),
    #[error("no active relay at {0}")]
    NoRelay(&'static str),
    #[error("could not configure the tunnel: {0}")]
    TunnelConfig(MullvadError),
    #[error(transparent)]
    Tunnel(#[from] TunnelError),
}

impl ConnectError {
    fn code(&self) -> FailureCode {
        match self {
            Self::CredentialStore(_) => FailureCode::CredentialStore,
            Self::SavedDevice(_) => FailureCode::SavedDevice,
            Self::RelayList(_) => FailureCode::RelayList,
            Self::NoRelay(_) => FailureCode::NoRelay,
            Self::Tunnel(TunnelError::Handshake) => FailureCode::Handshake,
            Self::TunnelConfig(_) | Self::Tunnel(_) => FailureCode::Tunnel,
        }
    }
}

pub struct Network {
    socks: SocksServer,
    /// Every request to e621 goes through this client, and so through the proxy and its route.
    http: reqwest::Client,
    /// Talks to Mullvad directly: these calls are what sets up the route the proxy would need.
    mullvad: Mullvad,
    tunnel: Mutex<Option<Tunnel>>,
    connection: watch::Sender<Connection>,
}

impl Network {
    /// Binds the proxy blocked: nothing goes out until [`Network::connect_saved`] has read the
    /// saved settings and opened a route.
    pub async fn start() -> std::io::Result<Self> {
        let socks = SocksServer::bind(Route::Blocked).await?;
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .proxy(
                reqwest::Proxy::all(format!("socks5h://{}", socks.address()))
                    .map_err(std::io::Error::other)?,
            )
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(READ_TIMEOUT)
            .build()
            .map_err(std::io::Error::other)?;
        let mullvad = Mullvad::new(
            reqwest::Client::builder()
                .user_agent(USER_AGENT)
                .timeout(MULLVAD_TIMEOUT)
                .build()
                .map_err(std::io::Error::other)?,
        );
        Ok(Self {
            socks,
            http,
            mullvad,
            tunnel: Mutex::new(None),
            connection: watch::Sender::new(Connection::Connecting),
        })
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

    /// Opens the route the saved settings ask for: the tunnel for a saved Mullvad device, direct
    /// when there is none. Any failure leaves the proxy blocked and is published as the
    /// connection state; nothing falls back to a direct connection.
    pub async fn connect_saved(&self) {
        if let Err(error) = self.try_connect_saved().await {
            tracing::error!("could not connect: {error}");
            self.connection
                .send_replace(Connection::Failed { code: error.code() });
        }
    }

    async fn try_connect_saved(&self) -> Result<(), ConnectError> {
        // On macOS this can wait for the user to answer a keychain access prompt.
        let saved = tokio::task::spawn_blocking(|| secrets::get(secrets::MULLVAD_DEVICE))
            .await
            .expect("reading the credential store panicked")?;
        let Some(device) = saved else {
            self.socks.set_route(Route::Direct);
            self.connection.send_replace(Connection::Direct);
            return Ok(());
        };
        let device: RegisteredDevice = serde_json::from_str(&device)?;
        let relays = self
            .mullvad
            .relays()
            .await
            .map_err(ConnectError::RelayList)?;
        let relay = relays
            .iter()
            .find(|relay| relay.active && relay.location.starts_with(DEFAULT_RELAY_LOCATION))
            .ok_or(ConnectError::NoRelay(DEFAULT_RELAY_LOCATION))?;
        let config = device
            .tunnel_config(relay)
            .map_err(ConnectError::TunnelConfig)?;

        let tunnel = Tunnel::start(&config).await?;
        self.socks.set_route(Route::Tunnel(tunnel.stack().clone()));
        tracing::info!("tunnel up via {}", relay.hostname);
        self.connection.send_replace(Connection::Tunnel {
            relay: relay.hostname.clone(),
        });
        if let Some(previous) = self.tunnel.lock().await.replace(tunnel) {
            previous.stop().await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use keyring_core::mock;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    use super::*;
    use crate::secrets::testing::{entry, mock_store};

    /// Whether a request through the app's client gets to a server on this machine, which only
    /// an open route allows.
    async fn reaches_local_server(network: &Network) -> bool {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            // Reading the request first keeps the close from turning into a reset.
            let mut request = Vec::new();
            let mut buffer = [0; 1024];
            while !request.ends_with(b"\r\n\r\n") {
                let read = stream.read(&mut buffer).await.unwrap();
                assert_ne!(read, 0, "the client closed before finishing its request");
                request.extend_from_slice(&buffer[..read]);
            }
            stream
                .write_all(b"HTTP/1.1 204 No Content\r\nconnection: close\r\n\r\n")
                .await
                .unwrap();
        });
        network
            .http()
            .get(format!("http://{address}/"))
            .send()
            .await
            .is_ok()
    }

    #[tokio::test]
    async fn starts_blocked() {
        let network = Network::start().await.unwrap();
        assert_eq!(network.connection(), Connection::Connecting);
        assert!(!reaches_local_server(&network).await);
    }

    #[tokio::test]
    async fn opens_direct_when_no_device_is_saved() {
        let _store = mock_store().await;
        let network = Network::start().await.unwrap();
        network.connect_saved().await;
        assert_eq!(network.connection(), Connection::Direct);
        assert!(reaches_local_server(&network).await);
    }

    #[tokio::test]
    async fn stays_blocked_when_the_credential_store_fails() {
        let _store = mock_store().await;
        let entry = entry(secrets::MULLVAD_DEVICE);
        let credential: &mock::Cred = entry.as_any().downcast_ref().unwrap();
        credential.set_error(keyring_core::Error::NoStorageAccess("access denied".into()));
        let network = Network::start().await.unwrap();
        network.connect_saved().await;
        assert_eq!(
            network.connection(),
            Connection::Failed {
                code: FailureCode::CredentialStore
            }
        );
        assert!(!reaches_local_server(&network).await);
    }

    #[tokio::test]
    async fn stays_blocked_when_the_saved_device_is_unreadable() {
        let _store = mock_store().await;
        entry(secrets::MULLVAD_DEVICE)
            .set_password("not a device")
            .unwrap();
        let network = Network::start().await.unwrap();
        network.connect_saved().await;
        assert_eq!(
            network.connection(),
            Connection::Failed {
                code: FailureCode::SavedDevice
            }
        );
        assert!(!reaches_local_server(&network).await);
    }
}
