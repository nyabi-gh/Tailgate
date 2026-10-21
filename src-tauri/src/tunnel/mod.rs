//! In-app WireGuard tunnel: only this app's traffic goes through it, with no OS VPN interface
//! and no elevated privileges. See docs/report.md §6.

mod config;
pub mod mullvad;
mod netstack;
pub mod socks;
mod wg;

use std::{io, sync::Arc, time::Duration};

use gotatun::x25519::PublicKey;
use tokio::{net::lookup_host, task::JoinHandle};

pub use config::{ConfigError, TunnelConfig};
pub use netstack::{NetStack, TcpStream};

/// gotatun re-sends a handshake initiation every 5 s (plus jitter); this leaves room for one retry.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
/// gotatun only rekeys when it sends data, so an idle tunnel never handshakes by itself. Past
/// WireGuard's Rekey-After-Time the health check handshakes instead, which proves the server
/// still answers.
const REKEY_AFTER_TIME: Duration = Duration::from_secs(120);
const HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(5);
/// Room for about four initiations, each re-sent after 5 s.
const HEALTH_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, thiserror::Error)]
pub enum TunnelError {
    #[error("could not resolve the VPN server address: {0}")]
    Endpoint(io::Error),
    #[error("could not start WireGuard: {0}")]
    WireGuard(#[from] gotatun::device::Error),
    #[error("the VPN server did not answer the handshake")]
    Handshake,
}

pub struct Tunnel {
    stack: Arc<NetStack>,
    device: wg::WgDevice,
    driver: JoinHandle<()>,
    peer: PublicKey,
}

/// Why a running tunnel is considered gone.
#[derive(Debug, thiserror::Error)]
pub enum TunnelLost {
    #[error("the VPN server stopped answering handshakes")]
    Silent,
    #[error("WireGuard stopped: {0}")]
    WireGuard(gotatun::device::Error),
    #[error("WireGuard stopped on an unrecoverable error")]
    Fatal,
}

impl Tunnel {
    /// Returns once a WireGuard handshake with the peer has completed. Creating the device does
    /// not contact the server, so without this an unreachable server would pass for a working
    /// tunnel.
    pub async fn start(config: &TunnelConfig) -> Result<Self, TunnelError> {
        let endpoint = lookup_host(&config.endpoint)
            .await
            .map_err(TunnelError::Endpoint)?
            .find(|address| address.is_ipv4())
            .ok_or_else(|| {
                TunnelError::Endpoint(io::Error::new(io::ErrorKind::NotFound, "no IPv4 address"))
            })?;

        let (stack, ports) = NetStack::new(config.address, config.dns);
        let device = wg::start(
            config,
            endpoint,
            Arc::clone(&stack),
            ports.to_stack,
            ports.from_stack,
        )
        .await?;
        let driver = tokio::spawn(Arc::clone(&stack).drive());
        let tunnel = Self {
            stack,
            device,
            driver,
            peer: PublicKey::from(config.peer_public_key),
        };

        let handshake = tokio::time::timeout(
            HANDSHAKE_TIMEOUT,
            tunnel.device.force_handshake(&tunnel.peer),
        )
        .await;
        match handshake {
            Ok(Ok(())) => Ok(tunnel),
            Ok(Err(error)) => {
                tunnel.stop().await;
                Err(error.into())
            }
            Err(_) => {
                tunnel.stop().await;
                Err(TunnelError::Handshake)
            }
        }
    }

    pub fn stack(&self) -> &Arc<NetStack> {
        &self.stack
    }

    /// Resolves once the tunnel can no longer carry traffic (docs/report.md §6.3, rule 5).
    pub async fn lost(&mut self) -> TunnelLost {
        loop {
            tokio::select! {
                () = self.device.wait() => return TunnelLost::Fatal,
                () = tokio::time::sleep(HEALTH_CHECK_INTERVAL) => {}
            }
            let last_handshake = self
                .device
                .read(async |device| {
                    device
                        .peers()
                        .await
                        .into_iter()
                        .find(|peer| peer.peer.public_key == self.peer)
                        .and_then(|peer| peer.stats.last_handshake)
                })
                .await;
            if last_handshake.is_some_and(|age| age < REKEY_AFTER_TIME) {
                continue;
            }
            match tokio::time::timeout(
                HEALTH_HANDSHAKE_TIMEOUT,
                self.device.force_handshake(&self.peer),
            )
            .await
            {
                Ok(Ok(())) => tracing::debug!("tunnel health check: handshake completed"),
                Ok(Err(error)) => return TunnelLost::WireGuard(error),
                Err(_) => return TunnelLost::Silent,
            }
        }
    }

    pub async fn stop(self) {
        self.driver.abort();
        self.device.stop().await;
    }
}
