//! A loopback SOCKS5 server (RFC 1928, CONNECT only, no auth) for the app's Rust HTTP clients.
//! The route is chosen per connection, so it can change while the app runs.

use std::{
    io,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr},
    sync::Arc,
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, copy_bidirectional},
    net::{TcpListener, TcpStream},
    sync::watch,
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
    route: watch::Sender<Route>,
    acceptor: JoinHandle<()>,
}

impl SocksServer {
    pub async fn bind(route: Route) -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let address = listener.local_addr()?;
        let route = watch::Sender::new(route);
        let acceptor = tokio::spawn(accept_loop(listener, route.subscribe()));
        Ok(Self {
            address,
            route,
            acceptor,
        })
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// Also closes every connection opened on the previous route, relayed or still connecting.
    /// Otherwise a pooled HTTP connection would keep carrying traffic the old way
    /// (docs/report.md §6.3, rule 4).
    pub fn set_route(&self, route: Route) {
        self.route.send_replace(route);
    }
}

impl Drop for SocksServer {
    fn drop(&mut self) {
        self.acceptor.abort();
    }
}

async fn accept_loop(listener: TcpListener, routes: watch::Receiver<Route>) {
    loop {
        let client = match listener.accept().await {
            Ok((client, _)) => client,
            Err(error) => {
                // Usually descriptor exhaustion; the listener itself stays usable.
                tracing::warn!("SOCKS accept failed: {error}");
                continue;
            }
        };
        // Taking the route marks it seen, so any later change ends this connection.
        let mut changes = routes.clone();
        let route = changes.borrow_and_update().clone();
        tokio::spawn(async move {
            tokio::select! {
                result = serve(client, route) => {
                    if let Err(error) = result {
                        tracing::debug!("SOCKS connection ended: {error}");
                    }
                }
                _ = changes.changed() => tracing::debug!("SOCKS connection closed: the route changed"),
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    /// A local server that answers every connection with `hello` and then keeps it open.
    async fn holding_server() -> SocketAddr {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let mut open = Vec::new();
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                stream.write_all(b"hello").await.unwrap();
                open.push(stream);
            }
        });
        address
    }

    /// Opens a CONNECT through the proxy and returns the stream with the reply code.
    async fn socks_connect(proxy: SocketAddr, target: SocketAddr) -> (TcpStream, u8) {
        let mut stream = TcpStream::connect(proxy).await.unwrap();
        stream.write_all(&[VERSION, 1, NO_AUTH]).await.unwrap();
        let mut method = [0u8; 2];
        stream.read_exact(&mut method).await.unwrap();
        let SocketAddr::V4(target) = target else {
            unreachable!("the test server is IPv4")
        };
        let mut request = vec![VERSION, CONNECT, 0, 1];
        request.extend_from_slice(&target.ip().octets());
        request.extend_from_slice(&target.port().to_be_bytes());
        stream.write_all(&request).await.unwrap();
        let mut reply = [0u8; 10];
        stream.read_exact(&mut reply).await.unwrap();
        (stream, reply[1])
    }

    #[tokio::test]
    async fn changing_the_route_closes_relayed_connections() {
        let target = holding_server().await;
        let socks = SocksServer::bind(Route::Direct).await.unwrap();
        let (mut stream, code) = socks_connect(socks.address(), target).await;
        assert_eq!(code, Reply::Succeeded as u8);
        let mut greeting = [0u8; 5];
        stream.read_exact(&mut greeting).await.unwrap();
        assert_eq!(&greeting, b"hello");

        socks.set_route(Route::Blocked);
        let mut rest = Vec::new();
        let read = tokio::time::timeout(Duration::from_secs(5), stream.read_to_end(&mut rest))
            .await
            .expect("the relayed connection stayed open");
        assert!(read.is_err() || rest.is_empty());

        let (_, code) = socks_connect(socks.address(), target).await;
        assert_eq!(code, Reply::NotAllowed as u8);
    }

    #[tokio::test]
    async fn connections_on_the_current_route_stay_open() {
        let target = holding_server().await;
        let socks = SocksServer::bind(Route::Blocked).await.unwrap();
        socks.set_route(Route::Direct);
        let (mut stream, code) = socks_connect(socks.address(), target).await;
        assert_eq!(code, Reply::Succeeded as u8);
        let mut greeting = [0u8; 5];
        stream.read_exact(&mut greeting).await.unwrap();
        let mut more = [0u8; 1];
        let still_open =
            tokio::time::timeout(Duration::from_millis(300), stream.read(&mut more)).await;
        assert!(still_open.is_err(), "the connection was closed");
    }
}
