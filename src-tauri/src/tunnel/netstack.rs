//! A userspace TCP/IP stack (smoltcp) whose IP packets travel through WireGuard instead of an OS
//! network interface. One task drives the stack; connections are plain `AsyncRead + AsyncWrite`.

use std::{
    future::poll_fn,
    io,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    pin::Pin,
    sync::{Arc, Mutex, MutexGuard},
    task::{Context, Poll},
    time::Duration,
};

use bytes::BytesMut;
use smoltcp::{
    iface::{Config, Interface, SocketHandle, SocketSet},
    phy::{self, DeviceCapabilities, Medium},
    socket::{dns, tcp},
    time::Instant,
    wire::{DnsQueryType, HardwareAddress, IpAddress, IpCidr},
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    sync::{Notify, mpsc},
};

/// Below Mullvad's 1380 so packets survive links with extra encapsulation.
pub const MTU: usize = 1280;
const TCP_BUFFER_SIZE: usize = 512 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const DNS_TIMEOUT: Duration = Duration::from_secs(10);
const IDLE_POLL: Duration = Duration::from_secs(1);
/// The peer answers every keep-alive, so going this long without any packet from it means the
/// path is gone (the tunnel died, say). smoltcp only ends a connection on the timeout; keep-alives
/// alone would wait forever.
const KEEP_ALIVE: smoltcp::time::Duration = smoltcp::time::Duration::from_secs(15);
const SILENCE_TIMEOUT: smoltcp::time::Duration = smoltcp::time::Duration::from_secs(45);

pub struct NetStack {
    state: Mutex<State>,
    /// Wakes the driver after anything that may let the stack make progress.
    wake: Notify,
}

struct State {
    iface: Interface,
    sockets: SocketSet<'static>,
    device: phy::Tracer<PacketQueues>,
    dns: SocketHandle,
    next_port: u16,
    closing: Vec<SocketHandle>,
}

/// Packets decrypted by WireGuard arrive on `inbound`; packets the stack emits leave on `outbound`.
struct PacketQueues {
    inbound: mpsc::UnboundedReceiver<BytesMut>,
    outbound: mpsc::UnboundedSender<BytesMut>,
}

/// The WireGuard side of the stack's packet queues.
pub struct PacketPorts {
    pub to_stack: mpsc::UnboundedSender<BytesMut>,
    pub from_stack: mpsc::UnboundedReceiver<BytesMut>,
}

impl NetStack {
    pub fn new(address: Ipv4Addr, dns_server: Ipv4Addr) -> (Arc<Self>, PacketPorts) {
        let (to_stack, inbound) = mpsc::unbounded_channel();
        let (outbound, from_stack) = mpsc::unbounded_channel();
        // Tunnel problems show up only as packets that never come back, so every packet can be
        // logged with `RUST_LOG=tunnel_packets=trace`.
        let mut device = phy::Tracer::new(
            PacketQueues { inbound, outbound },
            |_, packet| tracing::trace!(target: "tunnel_packets", "{packet}"),
        );

        let mut config = Config::new(HardwareAddress::Ip);
        // TCP initial sequence numbers and DNS query IDs come from this seed; smoltcp's default of
        // 0 would repeat them on every launch.
        config.random_seed = getrandom::u64().expect("the OS random source is unavailable");
        let mut iface = Interface::new(config, &mut device, Instant::now());
        iface.update_ip_addrs(|addrs| {
            addrs
                .push(IpCidr::new(IpAddress::Ipv4(address), 32))
                .expect("a fresh interface has room for one address");
        });
        // On a point-to-point tunnel every destination is "via the peer"; the gateway value only
        // has to be some address, it is never resolved.
        iface
            .routes_mut()
            .add_default_ipv4_route(dns_server)
            .expect("a fresh route table has room for one route");

        let mut sockets = SocketSet::new(Vec::new());
        let dns = sockets.add(dns::Socket::new(&[IpAddress::Ipv4(dns_server)], Vec::new()));

        let stack = Arc::new(Self {
            state: Mutex::new(State {
                iface,
                sockets,
                device,
                dns,
                next_port: random_ephemeral_port(),
                closing: Vec::new(),
            }),
            wake: Notify::new(),
        });
        (
            stack,
            PacketPorts {
                to_stack,
                from_stack,
            },
        )
    }

    /// Runs the stack until the returned future is dropped.
    pub async fn drive(self: Arc<Self>) {
        loop {
            let delay = self.lock().poll();
            tokio::select! {
                _ = self.wake.notified() => {}
                _ = tokio::time::sleep(delay) => {}
            }
        }
    }

    pub fn notify(&self) {
        self.wake.notify_one();
    }

    pub async fn resolve(&self, host: &str) -> io::Result<Ipv4Addr> {
        if let Ok(ip) = host.parse::<Ipv4Addr>() {
            return Ok(ip);
        }
        let query = {
            let mut state = self.lock();
            let State {
                iface,
                sockets,
                dns,
                ..
            } = &mut *state;
            sockets
                .get_mut::<dns::Socket>(*dns)
                .start_query(iface.context(), host, DnsQueryType::A)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error.to_string()))?
        };
        self.notify();

        let answer = tokio::time::timeout(
            DNS_TIMEOUT,
            poll_fn(|cx| {
                let mut state = self.lock();
                let dns = state.dns;
                let socket = state.sockets.get_mut::<dns::Socket>(dns);
                match socket.get_query_result(query) {
                    Ok(addresses) => Poll::Ready(Ok(addresses)),
                    Err(dns::GetQueryResultError::Pending) => {
                        socket.register_query_waker(query, cx.waker());
                        Poll::Pending
                    }
                    Err(dns::GetQueryResultError::Failed) => Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        format!("DNS lookup failed for {host}"),
                    ))),
                }
            }),
        )
        .await;

        let addresses = match answer {
            Ok(result) => result?,
            Err(_) => {
                let mut state = self.lock();
                let dns = state.dns;
                state
                    .sockets
                    .get_mut::<dns::Socket>(dns)
                    .cancel_query(query);
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("DNS lookup timed out for {host}"),
                ));
            }
        };
        addresses
            .first()
            .map(|address| match address {
                IpAddress::Ipv4(ip) => *ip,
            })
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("no IPv4 address for {host}"),
                )
            })
    }

    pub async fn connect(self: &Arc<Self>, address: SocketAddr) -> io::Result<TcpStream> {
        let IpAddr::V4(ip) = address.ip() else {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "the tunnel only carries IPv4",
            ));
        };
        let handle = {
            let mut state = self.lock();
            let mut socket = tcp::Socket::new(
                tcp::SocketBuffer::new(vec![0; TCP_BUFFER_SIZE]),
                tcp::SocketBuffer::new(vec![0; TCP_BUFFER_SIZE]),
            );
            socket.set_nagle_enabled(false);
            socket.set_keep_alive(Some(KEEP_ALIVE));
            socket.set_timeout(Some(SILENCE_TIMEOUT));
            let local_port = state.take_port();
            let State { iface, .. } = &mut *state;
            socket
                .connect(
                    iface.context(),
                    (IpAddress::Ipv4(ip), address.port()),
                    local_port,
                )
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error.to_string()))?;
            state.sockets.add(socket)
        };
        // From here on, dropping the stream closes and reaps the socket.
        let stream = TcpStream {
            stack: Arc::clone(self),
            handle,
        };
        self.notify();

        let established = poll_fn(|cx| {
            let mut state = self.lock();
            let socket = state.sockets.get_mut::<tcp::Socket>(handle);
            match socket.state() {
                tcp::State::Established => Poll::Ready(Ok(())),
                tcp::State::Closed | tcp::State::TimeWait => Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::ConnectionRefused,
                    format!("connection to {address} was refused"),
                ))),
                _ => {
                    socket.register_send_waker(cx.waker());
                    Poll::Pending
                }
            }
        });
        match tokio::time::timeout(CONNECT_TIMEOUT, established).await {
            Ok(result) => result.map(|()| stream),
            Err(_) => Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("connection to {address} timed out"),
            )),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // A panic while holding the lock leaves smoltcp in an unknown state; there is nothing
        // sensible to recover, so propagate it.
        self.state.lock().expect("network stack lock poisoned")
    }
}

impl State {
    /// Processes queued packets and timers; returns how long the stack may sleep.
    fn poll(&mut self) -> Duration {
        let now = Instant::now();
        self.iface.poll(now, &mut self.device, &mut self.sockets);
        self.reap_closed();
        self.iface
            .poll_delay(now, &self.sockets)
            .map(|delay| Duration::from_micros(delay.total_micros()))
            .unwrap_or(IDLE_POLL)
            .min(IDLE_POLL)
    }

    fn reap_closed(&mut self) {
        let sockets = &mut self.sockets;
        self.closing.retain(|&handle| {
            let done = matches!(
                sockets.get::<tcp::Socket>(handle).state(),
                tcp::State::Closed | tcp::State::TimeWait
            );
            if done {
                sockets.remove(handle);
            }
            !done
        });
    }

    fn take_port(&mut self) -> u16 {
        let port = self.next_port;
        self.next_port = if port == u16::MAX {
            EPHEMERAL_PORTS_START
        } else {
            port + 1
        };
        port
    }
}

const EPHEMERAL_PORTS_START: u16 = 49152;

fn random_ephemeral_port() -> u16 {
    let mut bytes = [0u8; 2];
    getrandom::fill(&mut bytes).expect("the OS random source is unavailable");
    EPHEMERAL_PORTS_START + u16::from_le_bytes(bytes) % (u16::MAX - EPHEMERAL_PORTS_START)
}

pub struct TcpStream {
    stack: Arc<NetStack>,
    handle: SocketHandle,
}

impl TcpStream {
    fn with_socket<R>(&self, f: impl FnOnce(&mut tcp::Socket<'static>) -> R) -> R {
        let mut state = self.stack.lock();
        f(state.sockets.get_mut::<tcp::Socket>(self.handle))
    }
}

impl AsyncRead for TcpStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let result = self.with_socket(|socket| {
            if socket.can_recv() {
                let read = socket
                    .recv_slice(buf.initialize_unfilled())
                    .map_err(|error| {
                        io::Error::new(io::ErrorKind::ConnectionReset, error.to_string())
                    })?;
                buf.advance(read);
                Poll::Ready(Ok(true))
            } else if !socket.may_recv() {
                Poll::Ready(Ok(false))
            } else {
                socket.register_recv_waker(cx.waker());
                Poll::Pending
            }
        });
        // Reading frees receive window, which the peer only learns about on the next poll.
        if let Poll::Ready(Ok(true)) = result {
            self.stack.notify();
        }
        result.map_ok(|_| ())
    }
}

impl AsyncWrite for TcpStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<io::Result<usize>> {
        let result =
            self.with_socket(|socket| {
                if socket.can_send() {
                    Poll::Ready(socket.send_slice(data).map_err(|error| {
                        io::Error::new(io::ErrorKind::BrokenPipe, error.to_string())
                    }))
                } else if !socket.may_send() {
                    Poll::Ready(Err(io::ErrorKind::BrokenPipe.into()))
                } else {
                    socket.register_send_waker(cx.waker());
                    Poll::Pending
                }
            });
        if let Poll::Ready(Ok(_)) = result {
            self.stack.notify();
        }
        result
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.with_socket(|socket| socket.close());
        self.stack.notify();
        Poll::Ready(Ok(()))
    }
}

impl Drop for TcpStream {
    fn drop(&mut self) {
        let mut state = self.stack.lock();
        state.sockets.get_mut::<tcp::Socket>(self.handle).close();
        state.closing.push(self.handle);
        drop(state);
        self.stack.notify();
    }
}

impl phy::Device for PacketQueues {
    type RxToken<'a> = RxToken;
    type TxToken<'a> = TxToken<'a>;

    fn receive(&mut self, _timestamp: Instant) -> Option<(RxToken, TxToken<'_>)> {
        let packet = self.inbound.try_recv().ok()?;
        Some((RxToken(packet), TxToken(&self.outbound)))
    }

    fn transmit(&mut self, _timestamp: Instant) -> Option<TxToken<'_>> {
        Some(TxToken(&self.outbound))
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut capabilities = DeviceCapabilities::default();
        capabilities.medium = Medium::Ip;
        capabilities.max_transmission_unit = MTU;
        capabilities
    }
}

pub struct RxToken(BytesMut);

impl phy::RxToken for RxToken {
    fn consume<R, F: FnOnce(&[u8]) -> R>(self, f: F) -> R {
        f(&self.0)
    }
}

pub struct TxToken<'a>(&'a mpsc::UnboundedSender<BytesMut>);

impl phy::TxToken for TxToken<'_> {
    fn consume<R, F: FnOnce(&mut [u8]) -> R>(self, len: usize, f: F) -> R {
        let mut packet = BytesMut::zeroed(len);
        let result = f(&mut packet);
        // The receiver only goes away when the tunnel is shutting down; the packet is moot then.
        let _ = self.0.send(packet);
        result
    }
}

#[cfg(test)]
mod tests {
    use smoltcp::wire::{Ipv4Packet, TcpPacket};

    use super::*;

    /// The sequence number in the SYN a fresh stack sends for its first connection.
    async fn first_syn_sequence_number() -> i32 {
        let (stack, mut ports) =
            NetStack::new(Ipv4Addr::new(10, 0, 0, 2), Ipv4Addr::new(10, 0, 0, 1));
        let driver = tokio::spawn(Arc::clone(&stack).drive());
        let syn = tokio::select! {
            packet = ports.from_stack.recv() => packet.expect("the stack sends a SYN"),
            _ = stack.connect(SocketAddr::from(([10, 0, 0, 1], 80))) => {
                unreachable!("nothing answers, so the connection cannot finish first")
            }
        };
        driver.abort();
        let ip = Ipv4Packet::new_checked(&syn[..]).unwrap();
        TcpPacket::new_checked(ip.payload()).unwrap().seq_number().0
    }

    #[tokio::test]
    async fn stacks_do_not_share_initial_sequence_numbers() {
        assert_ne!(
            first_syn_sequence_number().await,
            first_syn_sequence_number().await
        );
    }
}
