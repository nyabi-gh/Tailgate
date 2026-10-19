//! A loopback SOCKS5 server (RFC 1928, CONNECT only, no auth). Both the Rust HTTP client and the
//! webview use it, so switching between direct and tunnelled connections never needs a restart.

use std::{
    io,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr},
    sync::{Arc, RwLock},
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, copy_bidirectional},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

use super::NetStack;

#[derive(Clone)]
pub enum Route {
    Direct,
    Tunnel(Arc<NetStack>),
    /// The tunnel is selected but not up. Refusing here is the kill switch: nothing falls back to
    /// a direct connection.
    Blocked,
}

pub struct SocksServer {
    address: SocketAddr,
    route: Arc<RwLock<Route>>,
    acceptor: JoinHandle<()>,
}

impl SocksServer {
    pub async fn bind(route: Route) -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let address = listener.local_addr()?;
        let route = Arc::new(RwLock::new(route));
        let acceptor = tokio::spawn(accept_loop(listener, Arc::clone(&route)));
        Ok(Self {
            address,
            route,
            acceptor,
        })
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn set_route(&self, route: Route) {
        *self.route.write().expect("route lock poisoned") = route;
    }
}

impl Drop for SocksServer {
    fn drop(&mut self) {
        self.acceptor.abort();
    }
}

async fn accept_loop(listener: TcpListener, route: Arc<RwLock<Route>>) {
    loop {
        let client = match listener.accept().await {
            Ok((client, _)) => client,
            Err(error) => {
                // Usually descriptor exhaustion; the listener itself stays usable.
                tracing::warn!("SOCKS accept failed: {error}");
                continue;
            }
        };
        let route = route.read().expect("route lock poisoned").clone();
        tokio::spawn(async move {
            if let Err(error) = serve(client, route).await {
                tracing::debug!("SOCKS connection ended: {error}");
            }
        });
    }
}

const VERSION: u8 = 5;
const NO_AUTH: u8 = 0;
const NO_ACCEPTABLE_METHOD: u8 = 0xff;
const CONNECT: u8 = 1;

#[repr(u8)]
#[derive(Clone, Copy)]
enum Reply {
    Succeeded = 0,
    GeneralFailure = 1,
    NotAllowed = 2,
    HostUnreachable = 4,
    ConnectionRefused = 5,
    TtlExpired = 6,
    CommandNotSupported = 7,
    AddressTypeNotSupported = 8,
}

async fn serve(mut client: TcpStream, route: Route) -> io::Result<()> {
    let mut header = [0u8; 2];
    client.read_exact(&mut header).await?;
    let mut methods = vec![0u8; header[1] as usize];
    client.read_exact(&mut methods).await?;
    if header[0] != VERSION || !methods.contains(&NO_AUTH) {
        client.write_all(&[VERSION, NO_ACCEPTABLE_METHOD]).await?;
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported SOCKS greeting",
        ));
    }
    client.write_all(&[VERSION, NO_AUTH]).await?;

    let mut request = [0u8; 4];
    client.read_exact(&mut request).await?;
    if request[1] != CONNECT {
        return reply(&mut client, Reply::CommandNotSupported).await;
    }
    let host = match request[3] {
        1 => {
            let mut octets = [0u8; 4];
            client.read_exact(&mut octets).await?;
            Ipv4Addr::from(octets).to_string()
        }
        3 => {
            let length = client.read_u8().await?;
            let mut name = vec![0u8; length as usize];
            client.read_exact(&mut name).await?;
            String::from_utf8(name)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "host is not UTF-8"))?
        }
        4 => {
            let mut octets = [0u8; 16];
            client.read_exact(&mut octets).await?;
            Ipv6Addr::from(octets).to_string()
        }
        _ => return reply(&mut client, Reply::AddressTypeNotSupported).await,
    };
    let port = client.read_u16().await?;
    tracing::debug!("SOCKS CONNECT {host}:{port}");

    match route {
        Route::Direct => match TcpStream::connect((host.as_str(), port)).await {
            Ok(mut upstream) => {
                reply(&mut client, Reply::Succeeded).await?;
                copy_bidirectional(&mut client, &mut upstream)
                    .await
                    .map(|_| ())
            }
            Err(error) => {
                reply(&mut client, reply_for(&error)).await?;
                Err(error)
            }
        },
        Route::Tunnel(stack) => {
            let connected = async {
                let ip = stack.resolve(&host).await?;
                stack.connect(SocketAddr::from((ip, port))).await
            };
            match connected.await {
                Ok(mut upstream) => {
                    tracing::debug!("SOCKS tunnel connected {host}:{port}");
                    reply(&mut client, Reply::Succeeded).await?;
                    let (sent, received) = copy_bidirectional(&mut client, &mut upstream).await?;
                    tracing::debug!("SOCKS closed {host}:{port} sent={sent} received={received}");
                    Ok(())
                }
                Err(error) => {
                    reply(&mut client, reply_for(&error)).await?;
                    Err(error)
                }
            }
        }
        Route::Blocked => reply(&mut client, Reply::NotAllowed).await,
    }
}

fn reply_for(error: &io::Error) -> Reply {
    match error.kind() {
        io::ErrorKind::ConnectionRefused => Reply::ConnectionRefused,
        io::ErrorKind::NotFound | io::ErrorKind::HostUnreachable => Reply::HostUnreachable,
        io::ErrorKind::TimedOut => Reply::TtlExpired,
        _ => Reply::GeneralFailure,
    }
}

async fn reply(client: &mut TcpStream, code: Reply) -> io::Result<()> {
    // The bound address is meaningless for a CONNECT relay, so it is reported as 0.0.0.0:0.
    client
        .write_all(&[VERSION, code as u8, 0, 1, 0, 0, 0, 0, 0, 0])
        .await
}
