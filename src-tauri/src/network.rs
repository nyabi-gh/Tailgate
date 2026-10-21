//! Owns the loopback SOCKS proxy that every e621 request of the app goes through, the in-app
//! tunnel behind it, and the connection settings that choose between them.

use std::{
    io,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::Duration,
};

use serde::Serialize;
use tokio::sync::{Mutex, Notify, watch};

use crate::{
    relays::{self, LocationChoice, Origin, Relays},
    secrets,
    settings::{ConnectionMode, ConnectionSettings, DEFAULT_LOCATION, Settings, WireGuardSummary},
    tunnel::{
        ConfigError, Tunnel, TunnelConfig, TunnelError, TunnelLost,
        mullvad::{
            DEVICE_NOT_FOUND, INVALID_ACCOUNT, MAX_DEVICES_REACHED, Mullvad, MullvadError,
            RegisteredDevice, RelayList,
        },
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

/// Longer than the tunnel's own DNS and connect limits together (25 s), so those report first.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
/// Between two reads, not for the whole response: a large original may take minutes.
const READ_TIMEOUT: Duration = Duration::from_secs(30);
/// Whole requests, the relay list (about 600 KB) included.
const MULLVAD_TIMEOUT: Duration = Duration::from_secs(30);
const RETRY_FIRST: Duration = Duration::from_secs(1);
const RETRY_MAX: Duration = Duration::from_secs(60);
/// A wg-quick file is a few hundred bytes; anything far larger is not one.
const MAX_CONFIG_SIZE: u64 = 64 * 1024;

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
    Settings,
    SavedDevice,
    NoWireGuardConfig,
    SavedConfig,
    MullvadSignedOut,
    RelayList,
    NoRelay,
    Tunnel,
    Handshake,
    ConnectionLost,
}

#[derive(Debug, thiserror::Error)]
enum ConnectError {
    #[error("could not read the credential store: {0}")]
    CredentialStore(#[from] keyring_core::Error),
    #[error("{0}")]
    Settings(String),
    #[error("the saved Mullvad device is unreadable: {0}")]
    SavedDevice(#[from] serde_json::Error),
    #[error("no WireGuard configuration has been imported")]
    NoWireGuardConfig,
    #[error("the saved WireGuard configuration is unusable: {0}")]
    SavedConfig(ConfigError),
    #[error("not signed in to Mullvad")]
    MullvadSignedOut,
    #[error("could not get the relay list: {0}")]
    RelayList(MullvadError),
    #[error("no active relay at {0}")]
    NoRelay(String),
    #[error("could not configure the tunnel: {0}")]
    TunnelConfig(MullvadError),
    #[error(transparent)]
    Tunnel(#[from] TunnelError),
    #[error("the tunnel was lost: {0}")]
    Lost(TunnelLost),
}

impl ConnectError {
    fn code(&self) -> FailureCode {
        match self {
            Self::CredentialStore(_) => FailureCode::CredentialStore,
            Self::Settings(_) => FailureCode::Settings,
            Self::SavedDevice(_) => FailureCode::SavedDevice,
            Self::NoWireGuardConfig => FailureCode::NoWireGuardConfig,
            Self::SavedConfig(_) => FailureCode::SavedConfig,
            Self::MullvadSignedOut => FailureCode::MullvadSignedOut,
            Self::RelayList(_) => FailureCode::RelayList,
            Self::NoRelay(_) => FailureCode::NoRelay,
            Self::Tunnel(TunnelError::Handshake) => FailureCode::Handshake,
            Self::TunnelConfig(_) | Self::Tunnel(_) => FailureCode::Tunnel,
            Self::Lost(_) => FailureCode::ConnectionLost,
        }
    }

    /// Worth trying again by itself: the network may come back. The others give the same
    /// answer until the user changes something.
    fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::RelayList(_) | Self::TunnelConfig(_) | Self::Tunnel(_) | Self::Lost(_)
        )
    }

    /// What a relay list that has gone out of date would cause: the relay removed, or its key
    /// changed.
    fn may_be_outdated_list(&self) -> bool {
        matches!(
            self,
            Self::NoRelay(_) | Self::TunnelConfig(_) | Self::Tunnel(TunnelError::Handshake)
        )
    }
}

/// Why a change on the settings screen did not happen. The UI words it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, thiserror::Error)]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum SettingsChangeError {
    #[error("could not save the settings")]
    Settings,
    #[error("the credential store failed")]
    CredentialStore,
    #[error("could not reach Mullvad")]
    Network,
    #[error("Mullvad does not know this account number")]
    InvalidAccount,
    #[error("the Mullvad account has no free device slot")]
    TooManyDevices,
    #[error("Mullvad refused the request")]
    Mullvad,
    #[error("the file could not be read")]
    UnreadableFile,
    #[error("`{key}` is missing or invalid")]
    InvalidConfig { key: &'static str, missing: bool },
    #[error("no such location")]
    UnknownLocation,
}

impl From<MullvadError> for SettingsChangeError {
    fn from(error: MullvadError) -> Self {
        tracing::warn!("Mullvad request failed: {error}");
        match error {
            MullvadError::Network(_) => Self::Network,
            // Mullvad answers too short a number with INVALID_INPUT instead.
            _ if error.is_code(INVALID_ACCOUNT) || error.is_code("INVALID_INPUT") => {
                Self::InvalidAccount
            }
            _ if error.is_code(MAX_DEVICES_REACHED) => Self::TooManyDevices,
            _ => Self::Mullvad,
        }
    }
}

impl From<ConfigError> for SettingsChangeError {
    fn from(error: ConfigError) -> Self {
        match error {
            ConfigError::Missing(key) => Self::InvalidConfig { key, missing: true },
            ConfigError::Invalid { key, .. } => Self::InvalidConfig {
                key,
                missing: false,
            },
        }
    }
}

/// What the settings screen shows of the connection. No secrets.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionDetails {
    /// `None` until the settings have been read, or when they cannot be.
    pub mode: Option<ConnectionMode>,
    pub location: String,
    pub wireguard: Option<WireGuardSummary>,
    /// The registered device's name, once known; `None` also when signed out.
    pub mullvad_device: Option<String>,
}

enum Opened {
    Direct,
    Tunnel { tunnel: Tunnel, relay: String },
}

pub struct Network {
    socks: SocksServer,
    /// Every request to e621 goes through this client, and so through the proxy and its route.
    http: reqwest::Client,
    /// Talks to Mullvad directly: these calls are what sets up the route the proxy would need.
    mullvad: Mullvad,
    relays: Relays,
    settings: Arc<Settings>,
    /// The settings the last connection attempt went by, the first-run default included.
    effective: std::sync::Mutex<Option<ConnectionSettings>>,
    mullvad_device: std::sync::Mutex<Option<String>>,
    /// Asked for once per run; the account number has to come out of the credential store.
    mullvad_expiry: std::sync::Mutex<Option<String>>,
    connection: watch::Sender<Connection>,
    /// Wakes [`Network::run`] to connect again now.
    reconnect: Notify,
    /// Signing in or out of Mullvad and importing a configuration take turns.
    changing: Mutex<()>,
}

impl Network {
    /// Binds the proxy blocked: nothing goes out until [`Network::run`] has read the saved
    /// settings and opened a route.
    pub async fn start(settings: Arc<Settings>, relay_cache: PathBuf) -> io::Result<Self> {
        let mullvad = Mullvad::new(
            reqwest::Client::builder()
                .user_agent(USER_AGENT)
                .timeout(MULLVAD_TIMEOUT)
                .build()
                .map_err(io::Error::other)?,
        );
        Self::with_mullvad(settings, relay_cache, mullvad).await
    }

    async fn with_mullvad(
        settings: Arc<Settings>,
        relay_cache: PathBuf,
        mullvad: Mullvad,
    ) -> io::Result<Self> {
        let socks = SocksServer::bind(Route::Blocked).await?;
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .proxy(
                reqwest::Proxy::all(format!("socks5h://{}", socks.address()))
                    .map_err(io::Error::other)?,
            )
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(READ_TIMEOUT)
            .build()
            .map_err(io::Error::other)?;
        Ok(Self {
            socks,
            http,
            mullvad,
            relays: Relays::new(relay_cache),
            settings,
            effective: std::sync::Mutex::new(None),
            mullvad_device: std::sync::Mutex::new(None),
            mullvad_expiry: std::sync::Mutex::new(None),
            connection: watch::Sender::new(Connection::Connecting),
            reconnect: Notify::new(),
            changing: Mutex::new(()),
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

    /// Drops the current route and connects again without waiting for a retry.
    pub fn reconnect(&self) {
        self.reconnect.notify_one();
    }

    /// Keeps a route open for as long as the app runs: opens the one the settings ask for, watches
    /// a tunnel for loss, and tries again after failures that may pass (doubling waits, at most a
    /// minute). Whenever there is no route the proxy is blocked; nothing falls back to a direct
    /// connection.
    pub async fn run(self: Arc<Self>) {
        let mut tunnel = None;
        let mut failures = 0;
        loop {
            self.close_route(&mut tunnel).await;
            self.connection.send_replace(Connection::Connecting);
            let failure = match self.open_route().await {
                Ok(Opened::Direct) => {
                    failures = 0;
                    self.socks.set_route(Route::Direct);
                    self.connection.send_replace(Connection::Direct);
                    self.reconnect.notified().await;
                    continue;
                }
                Ok(Opened::Tunnel {
                    tunnel: mut up,
                    relay,
                }) => {
                    failures = 0;
                    self.socks.set_route(Route::Tunnel(Arc::clone(up.stack())));
                    tracing::info!("tunnel up via {relay}");
                    self.connection.send_replace(Connection::Tunnel { relay });
                    let lost = tokio::select! {
                        lost = up.lost() => Some(lost),
                        () = self.reconnect.notified() => None,
                    };
                    tunnel = Some(up);
                    match lost {
                        Some(lost) => ConnectError::Lost(lost),
                        None => continue,
                    }
                }
                Err(error) => error,
            };

            tracing::error!("no connection: {failure}");
            self.close_route(&mut tunnel).await;
            self.connection.send_replace(Connection::Failed {
                code: failure.code(),
            });
            if failure.is_transient() {
                let delay = retry_delay(failures);
                failures += 1;
                tracing::info!("connecting again in {delay:?}");
                tokio::select! {
                    () = tokio::time::sleep(delay) => {}
                    () = self.reconnect.notified() => {}
                }
            } else {
                self.reconnect.notified().await;
            }
        }
    }

    /// Blocks first, which also ends every connection on the old route (docs/report.md §6.3).
    async fn close_route(&self, tunnel: &mut Option<Tunnel>) {
        self.socks.set_route(Route::Blocked);
        if let Some(previous) = tunnel.take() {
            previous.stop().await;
        }
    }

    async fn open_route(self: &Arc<Self>) -> Result<Opened, ConnectError> {
        let settings = self.resolve_settings().await?;
        match settings.mode {
            ConnectionMode::Direct => Ok(Opened::Direct),
            ConnectionMode::WireGuard => {
                let text = read_secret(secrets::WIREGUARD_CONFIG)
                    .await?
                    .ok_or(ConnectError::NoWireGuardConfig)?;
                let config =
                    TunnelConfig::parse_wg_quick(&text).map_err(ConnectError::SavedConfig)?;
                let tunnel = Tunnel::start(&config).await?;
                let relay = settings
                    .wireguard
                    .as_ref()
                    .and_then(|summary| Path::new(&summary.file_name).file_stem())
                    .map_or(config.endpoint, |stem| stem.to_string_lossy().into_owned());
                Ok(Opened::Tunnel { tunnel, relay })
            }
            ConnectionMode::Mullvad => self.open_mullvad(&settings.location).await,
        }
    }

    /// The saved settings, or on the first run what the app did before it had any: the tunnel
    /// when a Mullvad device is saved, direct otherwise. An unreadable file is an error, not a
    /// first run.
    async fn resolve_settings(&self) -> Result<ConnectionSettings, ConnectError> {
        let resolved = match self.settings.connection() {
            Ok(Some(settings)) => Ok(settings),
            Ok(None) => read_secret(secrets::MULLVAD_DEVICE)
                .await
                .map(|device| {
                    ConnectionSettings::new(if device.is_some() {
                        ConnectionMode::Mullvad
                    } else {
                        ConnectionMode::Direct
                    })
                })
                .map_err(ConnectError::from),
            Err(error) => Err(ConnectError::Settings(error)),
        };
        *self.effective.lock().expect("settings lock poisoned") = resolved.as_ref().ok().cloned();
        resolved
    }

    async fn open_mullvad(self: &Arc<Self>, location: &str) -> Result<Opened, ConnectError> {
        let saved = read_secret(secrets::MULLVAD_DEVICE).await?;
        let device = saved
            .map(|device| serde_json::from_str::<RegisteredDevice>(&device))
            .transpose()?;
        *self.mullvad_device.lock().expect("device lock poisoned") =
            device.as_ref().map(|device| device.name.clone());
        let device = device.ok_or(ConnectError::MullvadSignedOut)?;

        let (list, origin) = self
            .relays
            .get(&self.mullvad)
            .await
            .map_err(ConnectError::RelayList)?;
        match start_relay(&device, &list, location).await {
            Ok(opened) => {
                if origin == Origin::Saved {
                    let network = Arc::clone(self);
                    tokio::spawn(async move {
                        if let Err(error) = network.relays.refresh(&network.mullvad).await {
                            tracing::warn!("could not refresh the relay list: {error}");
                        }
                    });
                }
                Ok(opened)
            }
            Err(error) if origin == Origin::Saved && error.may_be_outdated_list() => {
                tracing::warn!("{error}; trying again with a fresh relay list");
                let list = self
                    .relays
                    .refresh(&self.mullvad)
                    .await
                    .map_err(ConnectError::RelayList)?;
                start_relay(&device, &list, location).await
            }
            Err(error) => Err(error),
        }
    }

    pub fn details(&self) -> ConnectionDetails {
        let effective = self
            .effective
            .lock()
            .expect("settings lock poisoned")
            .clone();
        ConnectionDetails {
            mode: effective.as_ref().map(|settings| settings.mode),
            location: effective.as_ref().map_or_else(
                || DEFAULT_LOCATION.to_owned(),
                |settings| settings.location.clone(),
            ),
            wireguard: effective.and_then(|settings| settings.wireguard),
            mullvad_device: self
                .mullvad_device
                .lock()
                .expect("device lock poisoned")
                .clone(),
        }
    }

    pub fn set_mode(&self, mode: ConnectionMode) -> Result<(), SettingsChangeError> {
        self.change_settings(|settings| settings.mode = mode)?;
        self.reconnect();
        Ok(())
    }

    pub async fn set_location(&self, code: &str) -> Result<(), SettingsChangeError> {
        let (list, _) = self.relays.get(&self.mullvad).await?;
        if relays::pick(&list, code).is_none() {
            return Err(SettingsChangeError::UnknownLocation);
        }
        let saved = self.change_settings(|settings| settings.location = code.to_owned())?;
        if saved.mode == ConnectionMode::Mullvad {
            self.reconnect();
        }
        Ok(())
    }

    pub async fn locations(&self) -> Result<Vec<LocationChoice>, SettingsChangeError> {
        let (list, _) = self.relays.get(&self.mullvad).await?;
        Ok(relays::choices(&list))
    }

    /// Reads and checks the file in Rust, so the private key never passes through the webview,
    /// then switches to it.
    pub async fn import_wireguard(&self, path: PathBuf) -> Result<(), SettingsChangeError> {
        let _changing = self.changing.lock().await;
        let file = path.clone();
        let text = tokio::task::spawn_blocking(move || {
            use std::io::Read;
            let mut text = String::new();
            std::fs::File::open(file)?
                .take(MAX_CONFIG_SIZE)
                .read_to_string(&mut text)?;
            Ok::<_, io::Error>(text)
        })
        .await
        .expect("reading the configuration panicked")
        .map_err(|error| {
            tracing::warn!("could not read {}: {error}", path.display());
            SettingsChangeError::UnreadableFile
        })?;
        let config = TunnelConfig::parse_wg_quick(&text)?;
        write_secret(secrets::WIREGUARD_CONFIG, text).await?;
        let summary = WireGuardSummary {
            file_name: path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            endpoint: config.endpoint,
        };
        self.change_settings(|settings| {
            settings.mode = ConnectionMode::WireGuard;
            settings.wireguard = Some(summary);
        })?;
        self.reconnect();
        Ok(())
    }

    /// Registers a new device on the account and switches to Mullvad. Nothing is saved unless
    /// every step worked.
    pub async fn mullvad_sign_in(&self, account_number: &str) -> Result<(), SettingsChangeError> {
        let _changing = self.changing.lock().await;
        let account_number: String = account_number
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        let token = self.mullvad.access_token(&account_number).await?;

        // A device left from before (an interrupted sign-out, another account) would keep one of
        // the account's few device slots.
        if let Some(previous) = read_secret(secrets::MULLVAD_DEVICE)
            .await
            .map_err(store_error)?
            && let Ok(previous) = serde_json::from_str::<RegisteredDevice>(&previous)
        {
            match self.mullvad.remove_device(&token, &previous.id).await {
                Ok(()) => {}
                Err(error) if error.is_code(DEVICE_NOT_FOUND) => {}
                Err(error) => tracing::warn!("could not remove the previous device: {error}"),
            }
        }

        let device = self.mullvad.register_device(&token).await?;
        let json = serde_json::to_string(&device).expect("the device serializes");
        let saved = match write_secret(secrets::MULLVAD_ACCOUNT, account_number).await {
            Ok(()) => write_secret(secrets::MULLVAD_DEVICE, json).await,
            Err(error) => Err(error),
        };
        if let Err(error) = saved {
            // Without its saved key the device is of no use; give its slot back.
            if let Err(error) = self.mullvad.remove_device(&token, &device.id).await {
                tracing::warn!("could not remove the unsaved device: {error}");
            }
            return Err(error);
        }
        tracing::info!("registered Mullvad device {}", device.name);
        *self.mullvad_device.lock().expect("device lock poisoned") = Some(device.name);
        *self.mullvad_expiry.lock().expect("expiry lock poisoned") = self
            .mullvad
            .account_expiry(&token)
            .await
            .inspect_err(|error| tracing::warn!("could not get the account expiry: {error}"))
            .ok();

        self.change_settings(|settings| settings.mode = ConnectionMode::Mullvad)?;
        self.reconnect();
        Ok(())
    }

    /// Removes the device from the account first: if that fails, nothing changes, so the slot
    /// is not left taken by a key the app no longer has. The connection stays on Mullvad and so
    /// blocked; it does not turn direct by itself.
    pub async fn mullvad_sign_out(&self) -> Result<(), SettingsChangeError> {
        let _changing = self.changing.lock().await;
        let device = read_secret(secrets::MULLVAD_DEVICE)
            .await
            .map_err(store_error)?;
        let account = read_secret(secrets::MULLVAD_ACCOUNT)
            .await
            .map_err(store_error)?;
        if let Some(device) = device {
            match (serde_json::from_str::<RegisteredDevice>(&device), account) {
                (Ok(device), Some(account)) => {
                    let token = self.mullvad.access_token(account.trim()).await?;
                    match self.mullvad.remove_device(&token, &device.id).await {
                        Ok(()) => {}
                        Err(error) if error.is_code(DEVICE_NOT_FOUND) => {}
                        Err(error) => return Err(error.into()),
                    }
                }
                _ => tracing::warn!(
                    "signing out without removing the device: the saved account or device is unreadable"
                ),
            }
        }
        delete_secret(secrets::MULLVAD_DEVICE).await?;
        delete_secret(secrets::MULLVAD_ACCOUNT).await?;
        *self.mullvad_device.lock().expect("device lock poisoned") = None;
        *self.mullvad_expiry.lock().expect("expiry lock poisoned") = None;
        self.reconnect();
        Ok(())
    }

    /// When the Mullvad account is paid until (RFC 3339), or `None` when signed out.
    pub async fn mullvad_expiry(&self) -> Result<Option<String>, SettingsChangeError> {
        if let Some(expiry) = self
            .mullvad_expiry
            .lock()
            .expect("expiry lock poisoned")
            .clone()
        {
            return Ok(Some(expiry));
        }
        let Some(account) = read_secret(secrets::MULLVAD_ACCOUNT)
            .await
            .map_err(store_error)?
        else {
            return Ok(None);
        };
        let token = self.mullvad.access_token(account.trim()).await?;
        let expiry = self.mullvad.account_expiry(&token).await?;
        *self.mullvad_expiry.lock().expect("expiry lock poisoned") = Some(expiry.clone());
        Ok(Some(expiry))
    }

    /// Saves a change to what the connection settings are now, and returns the result.
    fn change_settings(
        &self,
        change: impl FnOnce(&mut ConnectionSettings),
    ) -> Result<ConnectionSettings, SettingsChangeError> {
        let mut effective = self.effective.lock().expect("settings lock poisoned");
        let mut next = self
            .settings
            .connection()
            .ok()
            .flatten()
            .or_else(|| effective.clone())
            .unwrap_or_else(|| ConnectionSettings::new(ConnectionMode::Direct));
        change(&mut next);
        self.settings
            .set_connection(next.clone())
            .map_err(|error| {
                tracing::error!("{error}");
                SettingsChangeError::Settings
            })?;
        *effective = Some(next.clone());
        Ok(next)
    }
}

async fn start_relay(
    device: &RegisteredDevice,
    list: &RelayList,
    location: &str,
) -> Result<Opened, ConnectError> {
    let relay =
        relays::pick(list, location).ok_or_else(|| ConnectError::NoRelay(location.to_owned()))?;
    let config = device
        .tunnel_config(relay)
        .map_err(ConnectError::TunnelConfig)?;
    let tunnel = Tunnel::start(&config).await?;
    Ok(Opened::Tunnel {
        tunnel,
        relay: relay.hostname.clone(),
    })
}

fn retry_delay(failures: u32) -> Duration {
    RETRY_FIRST
        .saturating_mul(2u32.saturating_pow(failures))
        .min(RETRY_MAX)
}

// On macOS these can wait for the user to answer a keychain access prompt, so they run off the
// async workers.

async fn read_secret(name: &'static str) -> Result<Option<String>, keyring_core::Error> {
    tokio::task::spawn_blocking(move || secrets::get(name))
        .await
        .expect("reading the credential store panicked")
}

async fn write_secret(name: &'static str, value: String) -> Result<(), SettingsChangeError> {
    tokio::task::spawn_blocking(move || secrets::set(name, &value))
        .await
        .expect("writing the credential store panicked")
        .map_err(store_error)
}

async fn delete_secret(name: &'static str) -> Result<(), SettingsChangeError> {
    tokio::task::spawn_blocking(move || secrets::delete(name))
        .await
        .expect("writing the credential store panicked")
        .map_err(store_error)
}

fn store_error(error: keyring_core::Error) -> SettingsChangeError {
    tracing::error!("credential store: {error}");
    SettingsChangeError::CredentialStore
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use keyring_core::mock;
    use serde_json::json;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    use super::*;
    use crate::{
        api::testing::{Received, json_response, serve},
        secrets::testing::{entry, mock_store},
        settings::testing::TempDir,
    };

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

    struct Fixture {
        network: Arc<Network>,
        mullvad: tokio::sync::mpsc::UnboundedReceiver<Received>,
        dir: TempDir,
    }

    /// A network with its settings in a fresh directory and Mullvad answering with `responses`.
    async fn fixture(settings: Option<&str>, responses: Vec<String>) -> Fixture {
        let dir = TempDir::new();
        if let Some(settings) = settings {
            std::fs::write(dir.join("settings.json"), settings).unwrap();
        }
        let (base, mullvad) = serve(responses).await;
        let http = reqwest::Client::builder().no_proxy().build().unwrap();
        let network = Network::with_mullvad(
            Arc::new(Settings::load(dir.join("settings.json"))),
            dir.join("relays.json"),
            Mullvad::with_base(http, base),
        )
        .await
        .unwrap();
        Fixture {
            network: Arc::new(network),
            mullvad,
            dir,
        }
    }

    fn running(network: &Arc<Network>) {
        tokio::spawn(Arc::clone(network).run());
    }

    async fn settles(network: &Network) -> Connection {
        let mut changes = network.watch_connection();
        let settled = changes.wait_for(|connection| *connection != Connection::Connecting);
        tokio::time::timeout(Duration::from_secs(5), settled)
            .await
            .expect("the connection never settled")
            .unwrap()
            .clone()
    }

    fn failed(code: FailureCode) -> Connection {
        Connection::Failed { code }
    }

    fn mode(mode: &str) -> String {
        json!({ "connection": { "mode": mode } }).to_string()
    }

    fn save_device() {
        entry(secrets::MULLVAD_DEVICE)
            .set_password(
                &json!({
                    "id": "device-1",
                    "name": "tidy pig",
                    "private_key": "yAnz5TF+lXXJte14tji3zlMNq+hd2rYUIgJBgB3fBmk=",
                    "ipv4": "10.64.1.2",
                })
                .to_string(),
            )
            .unwrap();
    }

    fn unavailable() -> String {
        json_response("503 Service Unavailable", "{}")
    }

    #[tokio::test]
    async fn starts_blocked() {
        let Fixture { network, .. } = fixture(None, vec![]).await;
        assert_eq!(network.connection(), Connection::Connecting);
        assert!(!reaches_local_server(&network).await);
    }

    #[tokio::test]
    async fn opens_direct_on_a_first_run_without_a_device() {
        let _store = mock_store().await;
        let Fixture { network, .. } = fixture(None, vec![]).await;
        running(&network);
        assert_eq!(settles(&network).await, Connection::Direct);
        assert!(reaches_local_server(&network).await);
        assert_eq!(network.details().mode, Some(ConnectionMode::Direct));
    }

    #[tokio::test]
    async fn a_first_run_with_a_saved_device_uses_mullvad() {
        let _store = mock_store().await;
        save_device();
        let Fixture { network, .. } = fixture(None, vec![unavailable(); 3]).await;
        running(&network);
        assert_eq!(settles(&network).await, failed(FailureCode::RelayList));
        assert!(!reaches_local_server(&network).await);
        let details = network.details();
        assert_eq!(details.mode, Some(ConnectionMode::Mullvad));
        assert_eq!(details.mullvad_device.as_deref(), Some("tidy pig"));
    }

    #[tokio::test]
    async fn stays_blocked_when_the_credential_store_fails() {
        let _store = mock_store().await;
        let entry = entry(secrets::MULLVAD_DEVICE);
        let credential: &mock::Cred = entry.as_any().downcast_ref().unwrap();
        credential.set_error(keyring_core::Error::NoStorageAccess("access denied".into()));
        let Fixture { network, .. } = fixture(None, vec![]).await;
        running(&network);
        assert_eq!(
            settles(&network).await,
            failed(FailureCode::CredentialStore)
        );
        assert!(!reaches_local_server(&network).await);
    }

    #[tokio::test]
    async fn stays_blocked_when_the_settings_are_damaged() {
        let _store = mock_store().await;
        let Fixture { network, .. } = fixture(Some("{ nope"), vec![]).await;
        running(&network);
        assert_eq!(settles(&network).await, failed(FailureCode::Settings));
        assert!(!reaches_local_server(&network).await);
        assert_eq!(network.details().mode, None);
    }

    #[tokio::test]
    async fn stays_blocked_when_the_saved_device_is_unreadable() {
        let _store = mock_store().await;
        entry(secrets::MULLVAD_DEVICE)
            .set_password("not a device")
            .unwrap();
        let Fixture { network, .. } = fixture(Some(&mode("mullvad")), vec![]).await;
        running(&network);
        assert_eq!(settles(&network).await, failed(FailureCode::SavedDevice));
        assert!(!reaches_local_server(&network).await);
    }

    #[tokio::test]
    async fn mullvad_without_an_account_stays_blocked() {
        let _store = mock_store().await;
        let Fixture { network, .. } = fixture(Some(&mode("mullvad")), vec![]).await;
        running(&network);
        assert_eq!(
            settles(&network).await,
            failed(FailureCode::MullvadSignedOut)
        );
        assert!(!reaches_local_server(&network).await);
    }

    #[tokio::test]
    async fn wireguard_without_a_usable_configuration_stays_blocked() {
        let _store = mock_store().await;
        let Fixture { network, .. } = fixture(Some(&mode("wireGuard")), vec![]).await;
        running(&network);
        assert_eq!(
            settles(&network).await,
            failed(FailureCode::NoWireGuardConfig)
        );

        entry(secrets::WIREGUARD_CONFIG)
            .set_password("[Interface]\n")
            .unwrap();
        let mut changes = network.watch_connection();
        network.reconnect();
        let damaged =
            changes.wait_for(|connection| *connection == failed(FailureCode::SavedConfig));
        tokio::time::timeout(Duration::from_secs(5), damaged)
            .await
            .unwrap()
            .unwrap();
        assert!(!reaches_local_server(&network).await);
    }

    #[tokio::test]
    async fn changing_the_mode_saves_it_and_reconnects() {
        let _store = mock_store().await;
        let Fixture { network, dir, .. } = fixture(Some(&mode("mullvad")), vec![]).await;
        running(&network);
        assert_eq!(
            settles(&network).await,
            failed(FailureCode::MullvadSignedOut)
        );

        let mut changes = network.watch_connection();
        network.set_mode(ConnectionMode::Direct).unwrap();
        let direct = changes.wait_for(|connection| *connection == Connection::Direct);
        tokio::time::timeout(Duration::from_secs(5), direct)
            .await
            .unwrap()
            .unwrap();
        assert!(reaches_local_server(&network).await);
        let saved = Settings::load(dir.join("settings.json")).connection();
        assert_eq!(saved.unwrap().unwrap().mode, ConnectionMode::Direct);

        network.set_mode(ConnectionMode::Mullvad).unwrap();
        let blocked =
            changes.wait_for(|connection| *connection == failed(FailureCode::MullvadSignedOut));
        tokio::time::timeout(Duration::from_secs(5), blocked)
            .await
            .unwrap()
            .unwrap();
        assert!(!reaches_local_server(&network).await);
    }

    #[tokio::test]
    async fn retries_a_failure_that_may_pass() {
        let _store = mock_store().await;
        save_device();
        let Fixture {
            network,
            mut mullvad,
            ..
        } = fixture(Some(&mode("mullvad")), vec![unavailable(); 3]).await;
        running(&network);
        let first = mullvad.recv().await.unwrap();
        let second = tokio::time::timeout(Duration::from_secs(5), mullvad.recv())
            .await
            .expect("no second attempt")
            .unwrap();
        assert_eq!(second.target, "/app/v1/relays");
        assert!(second.at - first.at >= Duration::from_millis(900));
    }

    #[test]
    fn retry_waits_double_up_to_a_minute() {
        let delays: Vec<u64> = (0..9).map(|n| retry_delay(n).as_secs()).collect();
        assert_eq!(delays, [1, 2, 4, 8, 16, 32, 60, 60, 60]);
        assert_eq!(retry_delay(u32::MAX), RETRY_MAX);
    }

    const CONFIG: &str = "[Interface]\n\
        PrivateKey = yAnz5TF+lXXJte14tji3zlMNq+hd2rYUIgJBgB3fBmk=\n\
        Address = 10.66.1.2/32\n\
        DNS = 10.64.0.1\n\
        [Peer]\n\
        PublicKey = xTIBA5rboUvnH4htodjb6e697QjLERt1NAB4mZqp8Dg=\n\
        Endpoint = 127.0.0.1:9\n";

    #[tokio::test]
    async fn imports_a_configuration_into_the_credential_store() {
        let _store = mock_store().await;
        let Fixture { network, dir, .. } = fixture(None, vec![]).await;
        let path = dir.join("se-sto-wg-001.conf");
        std::fs::write(&path, CONFIG).unwrap();
        network.import_wireguard(path).await.unwrap();

        assert_eq!(
            secrets::get(secrets::WIREGUARD_CONFIG).unwrap().as_deref(),
            Some(CONFIG)
        );
        let saved = Settings::load(dir.join("settings.json"))
            .connection()
            .unwrap()
            .unwrap();
        assert_eq!(saved.mode, ConnectionMode::WireGuard);
        assert_eq!(
            saved.wireguard,
            Some(WireGuardSummary {
                file_name: "se-sto-wg-001.conf".into(),
                endpoint: "127.0.0.1:9".into(),
            })
        );
    }

    #[tokio::test]
    async fn refuses_a_configuration_without_dns() {
        let _store = mock_store().await;
        let Fixture { network, dir, .. } = fixture(None, vec![]).await;
        let path = dir.join("broken.conf");
        std::fs::write(&path, CONFIG.replace("DNS = 10.64.0.1\n", "")).unwrap();
        assert_eq!(
            network.import_wireguard(path).await,
            Err(SettingsChangeError::InvalidConfig {
                key: "DNS",
                missing: true
            })
        );
        assert_eq!(secrets::get(secrets::WIREGUARD_CONFIG).unwrap(), None);
        assert!(!dir.join("settings.json").exists());
    }

    fn token() -> String {
        json_response(
            "200 OK",
            r#"{"access_token":"token-1","expiry":"2026-10-22T00:00:00Z"}"#,
        )
    }

    fn api_error(status: &str, code: &str) -> String {
        json_response(status, &json!({ "code": code }).to_string())
    }

    #[tokio::test]
    async fn signing_in_registers_a_device_and_switches_to_mullvad() {
        let _store = mock_store().await;
        let Fixture {
            network,
            mut mullvad,
            dir,
        } = fixture(
            Some(&mode("direct")),
            vec![
                token(),
                json_response(
                    "201 Created",
                    r#"{"id":"device-2","name":"brave owl","ipv4_address":"10.64.2.3/32","pubkey":"x"}"#,
                ),
                json_response("200 OK", r#"{"id":"a","expiry":"2027-03-02T00:00:00+00:00"}"#),
            ],
        )
        .await;
        network
            .mullvad_sign_in("1234 5678 9012 3456")
            .await
            .unwrap();

        let token_request = mullvad.recv().await.unwrap();
        assert_eq!(
            token_request.body,
            r#"{"account_number":"1234567890123456"}"#
        );
        assert_eq!(
            mullvad.recv().await.unwrap().header("authorization"),
            Some("Bearer token-1")
        );
        assert_eq!(
            secrets::get(secrets::MULLVAD_ACCOUNT).unwrap().as_deref(),
            Some("1234567890123456")
        );
        let device: serde_json::Value =
            serde_json::from_str(&secrets::get(secrets::MULLVAD_DEVICE).unwrap().unwrap()).unwrap();
        assert_eq!(device["name"], "brave owl");
        assert_eq!(device["ipv4"], "10.64.2.3");
        assert_eq!(
            network.details().mullvad_device.as_deref(),
            Some("brave owl")
        );
        assert_eq!(
            network.mullvad_expiry().await.unwrap().as_deref(),
            Some("2027-03-02T00:00:00+00:00")
        );
        let saved = Settings::load(dir.join("settings.json"))
            .connection()
            .unwrap()
            .unwrap();
        assert_eq!(saved.mode, ConnectionMode::Mullvad);
    }

    #[tokio::test]
    async fn sign_in_failures_save_nothing() {
        let _store = mock_store().await;
        let Fixture { network, dir, .. } = fixture(
            None,
            vec![
                api_error("400 Bad Request", "INVALID_ACCOUNT"),
                token(),
                api_error("400 Bad Request", "MAX_DEVICES_REACHED"),
            ],
        )
        .await;
        assert_eq!(
            network.mullvad_sign_in("0000000000000000").await,
            Err(SettingsChangeError::InvalidAccount)
        );
        assert_eq!(
            network.mullvad_sign_in("1234567890123456").await,
            Err(SettingsChangeError::TooManyDevices)
        );
        assert_eq!(secrets::get(secrets::MULLVAD_ACCOUNT).unwrap(), None);
        assert_eq!(secrets::get(secrets::MULLVAD_DEVICE).unwrap(), None);
        assert!(!dir.join("settings.json").exists());
    }

    #[tokio::test]
    async fn signing_out_removes_the_device_then_the_saved_entries() {
        let _store = mock_store().await;
        save_device();
        entry(secrets::MULLVAD_ACCOUNT)
            .set_password("1234567890123456\n")
            .unwrap();
        let Fixture {
            network,
            mut mullvad,
            ..
        } = fixture(
            Some(&mode("mullvad")),
            vec![token(), "HTTP/1.1 204 No Content\r\n\r\n".into()],
        )
        .await;
        network.mullvad_sign_out().await.unwrap();

        mullvad.recv().await.unwrap();
        let removal = mullvad.recv().await.unwrap();
        assert_eq!(removal.method, "DELETE");
        assert_eq!(removal.target, "/accounts/v1/devices/device-1");
        assert_eq!(secrets::get(secrets::MULLVAD_DEVICE).unwrap(), None);
        assert_eq!(secrets::get(secrets::MULLVAD_ACCOUNT).unwrap(), None);
    }

    #[tokio::test]
    async fn a_failed_device_removal_keeps_the_sign_in() {
        let _store = mock_store().await;
        save_device();
        entry(secrets::MULLVAD_ACCOUNT)
            .set_password("1234567890123456")
            .unwrap();
        let Fixture { network, .. } =
            fixture(Some(&mode("mullvad")), vec![token(), unavailable()]).await;
        assert_eq!(
            network.mullvad_sign_out().await,
            Err(SettingsChangeError::Mullvad)
        );
        assert!(secrets::get(secrets::MULLVAD_DEVICE).unwrap().is_some());
        assert!(secrets::get(secrets::MULLVAD_ACCOUNT).unwrap().is_some());
    }
}
