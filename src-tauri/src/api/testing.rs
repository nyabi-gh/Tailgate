//! An e621 stand-in for tests.

use std::{
    collections::VecDeque,
    net::Ipv4Addr,
    sync::{Arc, Mutex},
};

use serde_json::Value;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::mpsc,
    time::Instant,
};

#[derive(Debug)]
pub(crate) struct Received {
    pub(crate) at: Instant,
    pub(crate) method: String,
    pub(crate) target: String,
    pub(crate) user_agent: String,
    pub(crate) authorization: Option<String>,
    pub(crate) body: String,
}

pub(crate) fn json_response(status: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
}

pub(crate) fn ok(body: &Value) -> String {
    json_response("200 OK", &body.to_string())
}

/// An e621 stand-in that answers each request with the next of `responses`.
pub(crate) async fn serve(responses: Vec<String>) -> (String, mpsc::UnboundedReceiver<Received>) {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    let responses = Arc::new(Mutex::new(VecDeque::from(responses)));
    let (sender, received) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            let responses = Arc::clone(&responses);
            let sender = sender.clone();
            tokio::spawn(async move {
                let mut request = Vec::new();
                let mut buffer = [0; 1024];
                let end = loop {
                    if let Some(at) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                        break at + 4;
                    }
                    let read = stream.read(&mut buffer).await.unwrap();
                    assert_ne!(read, 0, "the client closed before finishing its request");
                    request.extend_from_slice(&buffer[..read]);
                };
                let head = String::from_utf8(request[..end].to_vec()).unwrap();
                let header = |name: &str| {
                    head.lines().find_map(|line| {
                        let (key, value) = line.split_once(": ")?;
                        key.eq_ignore_ascii_case(name).then(|| value.to_owned())
                    })
                };
                let length: usize =
                    header("content-length").map_or(0, |length| length.parse().unwrap());
                while request.len() < end + length {
                    let read = stream.read(&mut buffer).await.unwrap();
                    assert_ne!(read, 0, "the client closed before finishing its body");
                    request.extend_from_slice(&buffer[..read]);
                }
                sender
                    .send(Received {
                        at: Instant::now(),
                        method: head.split(' ').next().unwrap().to_owned(),
                        target: head.split(' ').nth(1).unwrap().to_owned(),
                        user_agent: header("user-agent").unwrap_or_default(),
                        authorization: header("authorization"),
                        body: String::from_utf8(request[end..end + length].to_vec()).unwrap(),
                    })
                    .unwrap();
                let response = responses
                    .lock()
                    .unwrap()
                    .pop_front()
                    .expect("more requests than scripted responses");
                stream.write_all(response.as_bytes()).await.unwrap();
            });
        }
    });
    (base, received)
}
