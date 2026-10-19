//! Plugs the network stack's packet queues into gotatun's WireGuard device in place of a TUN
//! interface, so handshakes, rekeying and keepalives stay in gotatun's audited code.

use std::{io, iter, net::SocketAddr, sync::Arc};

use bytes::BytesMut;
use gotatun::{
    device::{Device, DeviceBuilder, Peer},
    packet::{Ip, Packet, PacketBufPool},
    tun::{IpRecv, IpSend, MtuWatcher},
    udp::socket::UdpSocketFactory,
    x25519::{PublicKey, StaticSecret},
};
use tokio::sync::mpsc;

use super::{
    config::TunnelConfig,
    netstack::{MTU, NetStack},
};

pub type WgDevice = Device<(UdpSocketFactory, ToStack, FromStack)>;

/// Decrypted packets from the peer, handed to the network stack.
pub struct ToStack {
    queue: mpsc::UnboundedSender<BytesMut>,
    stack: Arc<NetStack>,
}

impl IpSend for ToStack {
    async fn send(&mut self, packet: Packet<Ip>) -> io::Result<()> {
        let packet = packet.into_bytes();
        self.queue
            .send(BytesMut::from(&packet[..]))
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "network stack stopped"))?;
        self.stack.notify();
        Ok(())
    }
}

/// Packets the network stack wants to send, handed to WireGuard for encryption.
pub struct FromStack {
    queue: mpsc::UnboundedReceiver<BytesMut>,
}

const MAX_BATCH: usize = 64;

impl IpRecv for FromStack {
    async fn recv<'a>(
        &'a mut self,
        _pool: &mut PacketBufPool,
    ) -> io::Result<impl Iterator<Item = Packet<Ip>> + Send + 'a> {
        let first =
            self.queue.recv().await.ok_or_else(|| {
                io::Error::new(io::ErrorKind::UnexpectedEof, "network stack stopped")
            })?;
        let rest = iter::from_fn(|| self.queue.try_recv().ok()).take(MAX_BATCH - 1);
        // smoltcp only emits well-formed IPv4 packets, so a parse failure should not happen. If one
        // does, dropping the packet is deliberate: gotatun ends its send loop on any error from
        // `recv`, which would silently stop the tunnel.
        Ok(iter::once(first)
            .chain(rest)
            .filter_map(|bytes| Packet::from_bytes(bytes).try_into_ip().ok()))
    }

    fn mtu(&self) -> MtuWatcher {
        MtuWatcher::new(MTU as u16)
    }
}

pub async fn start(
    config: &TunnelConfig,
    endpoint: SocketAddr,
    stack: Arc<NetStack>,
    to_stack: mpsc::UnboundedSender<BytesMut>,
    from_stack: mpsc::UnboundedReceiver<BytesMut>,
) -> Result<WgDevice, gotatun::device::Error> {
    let mut peer = Peer::new(PublicKey::from(config.peer_public_key))
        .with_endpoint(endpoint)
        .with_allowed_ip("0.0.0.0/0".parse().expect("valid network literal"));
    peer.keepalive = config.keepalive;
    if let Some(preshared_key) = config.preshared_key {
        peer = peer.with_preshared_key(preshared_key);
    }

    DeviceBuilder::new()
        .with_default_udp()
        .with_ip_pair(
            ToStack {
                queue: to_stack,
                stack,
            },
            FromStack { queue: from_stack },
        )
        .with_private_key(StaticSecret::from(config.private_key))
        .with_peer(peer)
        .build()
        .await
}
