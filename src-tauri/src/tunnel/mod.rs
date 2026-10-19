//! In-app WireGuard tunnel: only this app's traffic goes through it, with no OS VPN interface
//! and no elevated privileges. See docs/report.md §5.

mod config;
pub mod mullvad;
mod netstack;
pub mod socks;
mod wg;

use std::{io, sync::Arc};

use tokio::{net::lookup_host, task::JoinHandle};

pub use config::{ConfigError, TunnelConfig};
pub use netstack::{NetStack, TcpStream};

#[derive(Debug, thiserror::Error)]
pub enum TunnelError {
    #[error("could not resolve the VPN server address: {0}")]
    Endpoint(io::Error),
    #[error("could not start WireGuard: {0}")]
    WireGuard(#[from] gotatun::device::Error),
}

pub struct Tunnel {
    stack: Arc<NetStack>,
    device: wg::WgDevice,
    driver: JoinHandle<()>,
}

impl Tunnel {
    pub async fn start(config: &TunnelConfig) -> Result<Self, TunnelError> {
        let endpoint = lookup_host(&config.endpoint)
            .await
            .map_err(TunnelError::Endpoint)?
            .find(|address| address.is_ipv4())
            .ok_or_else(|| TunnelError::Endpoint(io::Error::new(io::ErrorKind::NotFound, "no IPv4 address")))?;

        let (stack, ports) = NetStack::new(config.address, config.dns);
        let device = wg::start(config, endpoint, Arc::clone(&stack), ports.to_stack, ports.from_stack).await?;
        let driver = tokio::spawn(Arc::clone(&stack).drive());
        Ok(Self { stack, device, driver })
    }

    pub fn stack(&self) -> &Arc<NetStack> {
        &self.stack
    }

    pub async fn stop(self) {
        self.driver.abort();
        self.device.stop().await;
    }
}
