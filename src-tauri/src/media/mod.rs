//! Serves e621 media files to the webview through the app's own connection.
//!
//! static1.e621.net answers 403 to requests that carry a browser's cross-site fetch metadata
//! (`Sec-Fetch-Dest: image|video`, `Sec-Fetch-Mode: no-cors`, `Sec-Fetch-Site: cross-site`), which
//! every `<img>`/`<video>` in the webview sends. Fetching from Rust, as an API client with its own
//! User-Agent, is how the files are meant to be downloaded. See docs/research.md §7.

mod blocks;

use std::sync::Arc;

use tauri::{
    AppHandle, Manager, UriSchemeResponder,
    http::{HeaderValue, Request, Response, StatusCode, header},
};

use blocks::{BLOCK_SIZE, Block, BlockCache};

pub const SCHEME: &str = "media";
const ORIGIN: &str = "https://static1.e621.net/";
const CACHE_CAPACITY: u64 = 256 * 1024 * 1024;

pub struct Media {
    /// The app's e621 client, so media follows the same route and kill switch as the API.
    http: reqwest::Client,
    origin: String,
    blocks: BlockCache,
}

pub fn handle(app: &AppHandle, request: Request<Vec<u8>>, responder: UriSchemeResponder) {
    let media = Arc::clone(app.state::<Arc<Media>>().inner());
    tauri::async_runtime::spawn(async move {
        let response = media.respond(&request).await.unwrap_or_else(empty_response);
        responder.respond(response);
    });
}

impl Media {
    pub fn new(http: reqwest::Client) -> Self {
        Self::with_origin(http, ORIGIN.to_owned())
    }

    fn with_origin(http: reqwest::Client, origin: String) -> Self {
        Self {
            http,
            origin,
            blocks: BlockCache::new(CACHE_CAPACITY),
        }
    }

    async fn respond(
        self: &Arc<Self>,
        request: &Request<Vec<u8>>,
    ) -> Result<Response<Vec<u8>>, StatusCode> {
        let path = request.uri().path().trim_start_matches('/');
        if !is_media_path(path) {
            return Err(StatusCode::NOT_FOUND);
        }
        // A range in a form this does not handle is ignored, which RFC 9110 §14.2 allows: the
        // whole file is sent instead.
        let range = request
            .headers()
            .get(header::RANGE)
            .and_then(|value| value.to_str().ok())
            .and_then(ByteRange::parse);
        tracing::debug!("media {path} range={range:?}");
        match range {
            Some(range) => self.partial(path, range).await,
            None => self.whole(path).await,
        }
    }

    async fn whole(&self, path: &str) -> Result<Response<Vec<u8>>, StatusCode> {
        let response = self
            .http
            .get(format!("{}{path}", self.origin))
            .send()
            .await
            .map_err(|error| {
                tracing::warn!("media request for {path} failed: {error}");
                StatusCode::BAD_GATEWAY
            })?;
        let status = response.status();
        if !status.is_success() {
            tracing::warn!("static1 answered {status} for {path}");
            return Err(upstream_error(status));
        }
        let mut builder = Response::builder().status(status.as_u16());
        if let Some(value) = response.headers().get(header::CONTENT_TYPE) {
            builder = builder.header(header::CONTENT_TYPE, copy_header(value)?);
        }
        let body = response.bytes().await.map_err(|error| {
            tracing::warn!("media body for {path} failed: {error}");
            StatusCode::BAD_GATEWAY
        })?;
        builder
            .body(body.to_vec())
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    }

    /// Answers from a single cached block, cut to the end of that block; the player asks again
    /// for the rest.
    async fn partial(
        self: &Arc<Self>,
        path: &str,
        range: ByteRange,
    ) -> Result<Response<Vec<u8>>, StatusCode> {
        let start = match range {
            ByteRange::From(start) | ByteRange::Within(start, _) => start,
            ByteRange::Last(len) => self.block(path, 0).await?.file_len.saturating_sub(len),
        };
        let index = start / BLOCK_SIZE;
        let block = self.block(path, index).await?;
        if start >= block.file_len {
            return Response::builder()
                .status(StatusCode::RANGE_NOT_SATISFIABLE)
                .header(header::CONTENT_RANGE, format!("bytes */{}", block.file_len))
                .body(Vec::new())
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
        }
        let block_start = index * BLOCK_SIZE;
        let block_end = block_start + block.data.len() as u64 - 1;
        let end = match range {
            ByteRange::Within(_, end) => end.min(block_end),
            ByteRange::From(_) | ByteRange::Last(_) => block_end,
        };
        if end == block_end && block_end + 1 < block.file_len {
            self.prefetch(path, index + 1);
        }

        let mut builder = Response::builder()
            .status(StatusCode::PARTIAL_CONTENT)
            .header(
                header::CONTENT_RANGE,
                format!("bytes {start}-{end}/{}", block.file_len),
            )
            .header(header::ACCEPT_RANGES, "bytes");
        if let Some(content_type) = &block.content_type {
            builder = builder.header(header::CONTENT_TYPE, content_type);
        }
        let offset = (start - block_start) as usize;
        let len = (end - start + 1) as usize;
        builder
            .body(block.data[offset..offset + len].to_vec())
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    }

    async fn block(&self, path: &str, index: u64) -> Result<Block, StatusCode> {
        self.blocks
            .get(path, index, async || self.fetch_block(path, index).await)
            .await
    }

    fn prefetch(self: &Arc<Self>, path: &str, index: u64) {
        let media = Arc::clone(self);
        let path = path.to_owned();
        // A failure is logged by `fetch_block` and not cached; the player's own request for the
        // block tries again.
        tokio::spawn(async move { media.block(&path, index).await.ok() });
    }

    async fn fetch_block(&self, path: &str, index: u64) -> Result<Block, StatusCode> {
        let first = index * BLOCK_SIZE;
        let last = first + BLOCK_SIZE - 1;
        let response = self
            .http
            .get(format!("{}{path}", self.origin))
            .header(header::RANGE, format!("bytes={first}-{last}"))
            .send()
            .await
            .map_err(|error| {
                tracing::warn!("media request for block {index} of {path} failed: {error}");
                StatusCode::BAD_GATEWAY
            })?;
        let status = response.status();
        if status != reqwest::StatusCode::PARTIAL_CONTENT {
            tracing::warn!("static1 answered {status} for block {index} of {path}");
            return Err(upstream_error(status));
        }
        let Some((start, end, file_len)) = response
            .headers()
            .get(header::CONTENT_RANGE)
            .and_then(|value| value.to_str().ok())
            .and_then(parse_content_range)
            .filter(|&(start, end, file_len)| {
                start == first && end < file_len && end == last.min(file_len - 1)
            })
        else {
            tracing::warn!(
                "static1 answered block {index} of {path} with content range {:?}",
                response.headers().get(header::CONTENT_RANGE)
            );
            return Err(StatusCode::BAD_GATEWAY);
        };
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .map(copy_header)
            .transpose()?;
        let data = response.bytes().await.map_err(|error| {
            tracing::warn!("media body for block {index} of {path} failed: {error}");
            StatusCode::BAD_GATEWAY
        })?;
        if data.len() as u64 != end - start + 1 {
            tracing::warn!(
                "block {index} of {path} has {} bytes, expected {}",
                data.len(),
                end - start + 1
            );
            return Err(StatusCode::BAD_GATEWAY);
        }
        Ok(Block {
            data,
            file_len,
            content_type,
        })
    }
}

/// The single-range forms of an HTTP `Range` header.
#[derive(Clone, Copy, Debug, PartialEq)]
enum ByteRange {
    From(u64),
    Within(u64, u64),
    Last(u64),
}

impl ByteRange {
    fn parse(header: &str) -> Option<Self> {
        let (start, end) = header.strip_prefix("bytes=")?.split_once('-')?;
        match (start, end) {
            ("", len) => len.parse().ok().map(Self::Last),
            (start, "") => start.parse().ok().map(Self::From),
            (start, end) => {
                let (start, end) = (start.parse().ok()?, end.parse().ok()?);
                (start <= end).then_some(Self::Within(start, end))
            }
        }
    }
}

/// `bytes <start>-<end>/<length>`, the only form a 206 for a single range carries.
fn parse_content_range(value: &str) -> Option<(u64, u64, u64)> {
    let (range, file_len) = value.strip_prefix("bytes ")?.split_once('/')?;
    let (start, end) = range.split_once('-')?;
    Some((
        start.parse().ok()?,
        end.parse().ok()?,
        file_len.parse().ok()?,
    ))
}

/// Only file paths under static1's `data/` tree; anything else could turn the scheme into a way
/// to make the app request arbitrary URLs.
fn is_media_path(path: &str) -> bool {
    path.starts_with("data/")
        && !path.contains("..")
        && path
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"/._-".contains(&byte))
}

fn upstream_error(status: reqwest::StatusCode) -> StatusCode {
    if status.is_success() {
        return StatusCode::BAD_GATEWAY;
    }
    StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY)
}

fn copy_header(value: &reqwest::header::HeaderValue) -> Result<HeaderValue, StatusCode> {
    HeaderValue::from_bytes(value.as_bytes()).map_err(|_| StatusCode::BAD_GATEWAY)
}

fn empty_response(status: StatusCode) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .body(Vec::new())
        .expect("static response parts are valid")
}

#[cfg(test)]
mod tests {
    use std::{net::Ipv4Addr, time::Duration};

    use bytes::Bytes;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
        sync::mpsc,
    };

    use super::*;

    const PATH: &str = "data/sample/36/a9/36a9487818d5a477adb0beed756b5c10_720p.mp4";

    fn file(len: u64) -> Bytes {
        (0..len).map(|i| (i % 251) as u8).collect()
    }

    /// A static1 stand-in that answers single ranges like it does and reports each request's
    /// `Range` header (empty when there is none).
    async fn serve(file: Bytes) -> (Arc<Media>, mpsc::UnboundedReceiver<String>) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let origin = format!("http://{}/", listener.local_addr().unwrap());
        let (requests, received) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                tokio::spawn(answer(stream, file.clone(), requests.clone()));
            }
        });
        let http = reqwest::Client::builder().no_proxy().build().unwrap();
        (Arc::new(Media::with_origin(http, origin)), received)
    }

    async fn answer(mut stream: TcpStream, file: Bytes, requests: mpsc::UnboundedSender<String>) {
        let mut head = Vec::new();
        let mut buffer = [0; 1024];
        while !head.ends_with(b"\r\n\r\n") {
            let read = stream.read(&mut buffer).await.unwrap();
            assert_ne!(read, 0, "the client closed before finishing its request");
            head.extend_from_slice(&buffer[..read]);
        }
        let head = String::from_utf8(head).unwrap();
        let range = head
            .lines()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("range: ")
                    .map(str::to_owned)
            })
            .unwrap_or_default();
        requests.send(range.clone()).unwrap();

        let len = file.len() as u64;
        let (status, extra, body) = match ByteRange::parse(&range) {
            None => ("200 OK", String::new(), file),
            Some(ByteRange::Within(start, _)) | Some(ByteRange::From(start)) if start >= len => (
                "416 Range Not Satisfiable",
                format!("content-range: bytes */{len}\r\n"),
                Bytes::new(),
            ),
            Some(ByteRange::Within(start, end)) => {
                let end = end.min(len - 1);
                (
                    "206 Partial Content",
                    format!("content-range: bytes {start}-{end}/{len}\r\n"),
                    file.slice(start as usize..=end as usize),
                )
            }
            Some(other) => panic!("the cache asked for {other:?}"),
        };
        let response = format!(
            "HTTP/1.1 {status}\r\ncontent-type: video/mp4\r\ncontent-length: {}\r\n{extra}connection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(response.as_bytes()).await.unwrap();
        stream.write_all(&body).await.unwrap();
    }

    async fn get(media: &Arc<Media>, range: Option<&str>) -> Response<Vec<u8>> {
        let mut request = Request::builder().uri(format!("media://localhost/{PATH}"));
        if let Some(range) = range {
            request = request.header(header::RANGE, range);
        }
        media
            .respond(&request.body(Vec::new()).unwrap())
            .await
            .unwrap_or_else(empty_response)
    }

    fn content_range(response: &Response<Vec<u8>>) -> &str {
        response.headers()[header::CONTENT_RANGE].to_str().unwrap()
    }

    async fn next_request(requests: &mut mpsc::UnboundedReceiver<String>) -> String {
        tokio::time::timeout(Duration::from_secs(5), requests.recv())
            .await
            .expect("no request reached the server")
            .unwrap()
    }

    #[tokio::test]
    async fn answers_small_ranges_from_one_upstream_request() {
        let file = file(3 * BLOCK_SIZE);
        let (media, mut requests) = serve(file.clone()).await;
        for (start, end) in [(0, 7), (8, 31), (1000, 1023), (500_000, 500_015)] {
            let response = get(&media, Some(&format!("bytes={start}-{end}"))).await;
            assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
            assert_eq!(
                content_range(&response),
                format!("bytes {start}-{end}/{}", file.len())
            );
            assert_eq!(response.headers()[header::CONTENT_TYPE], "video/mp4");
            assert_eq!(response.body(), &file[start..=end]);
        }
        assert_eq!(
            next_request(&mut requests).await,
            format!("bytes=0-{}", BLOCK_SIZE - 1)
        );
        assert!(requests.try_recv().is_err());
    }

    #[tokio::test]
    async fn cuts_open_ranges_at_the_block_end_and_prefetches_the_next_block() {
        let file = file(2 * BLOCK_SIZE + 10);
        let (media, mut requests) = serve(file.clone()).await;
        let start = BLOCK_SIZE + 5;
        let response = get(&media, Some(&format!("bytes={start}-"))).await;
        let end = 2 * BLOCK_SIZE - 1;
        assert_eq!(
            content_range(&response),
            format!("bytes {start}-{end}/{}", file.len())
        );
        assert_eq!(response.body(), &file[start as usize..=end as usize]);
        assert_eq!(
            next_request(&mut requests).await,
            format!("bytes={BLOCK_SIZE}-{end}")
        );
        assert_eq!(
            next_request(&mut requests).await,
            format!("bytes={}-{}", 2 * BLOCK_SIZE, 3 * BLOCK_SIZE - 1)
        );
    }

    #[tokio::test]
    async fn serves_the_short_last_block_and_suffix_ranges() {
        let file = file(BLOCK_SIZE + 10);
        let (media, _requests) = serve(file.clone()).await;
        let response = get(&media, Some("bytes=-4")).await;
        assert_eq!(
            content_range(&response),
            format!("bytes {}-{}/{}", BLOCK_SIZE + 6, BLOCK_SIZE + 9, file.len())
        );
        assert_eq!(response.body(), &file[file.len() - 4..]);
    }

    #[tokio::test]
    async fn rejects_ranges_past_the_end() {
        let (media, _requests) = serve(file(10)).await;
        let response = get(&media, Some("bytes=-0")).await;
        assert_eq!(response.status(), StatusCode::RANGE_NOT_SATISFIABLE);
        assert_eq!(content_range(&response), "bytes */10");
        let response = get(&media, Some("bytes=20-")).await;
        assert_eq!(response.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    }

    #[tokio::test]
    async fn sends_whole_files_without_a_range() {
        let file = file(BLOCK_SIZE + 10);
        let (media, mut requests) = serve(file.clone()).await;
        let response = get(&media, None).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.body(), &file[..]);
        assert_eq!(next_request(&mut requests).await, "");
    }

    #[test]
    fn parses_single_byte_ranges() {
        assert_eq!(ByteRange::parse("bytes=0-"), Some(ByteRange::From(0)));
        assert_eq!(ByteRange::parse("bytes=2-9"), Some(ByteRange::Within(2, 9)));
        assert_eq!(ByteRange::parse("bytes=-500"), Some(ByteRange::Last(500)));
        assert_eq!(ByteRange::parse("bytes=9-2"), None);
        assert_eq!(ByteRange::parse("bytes=0-1,5-9"), None);
        assert_eq!(ByteRange::parse("items=0-1"), None);
    }

    #[test]
    fn parses_content_ranges() {
        assert_eq!(
            parse_content_range("bytes 0-1048575/8234567"),
            Some((0, 1_048_575, 8_234_567))
        );
        assert_eq!(parse_content_range("bytes */100"), None);
    }

    #[test]
    fn accepts_only_static_data_paths() {
        assert!(is_media_path(
            "data/f0/9b/f09b5a771b5861c9c207db6d0fa7bce4.png"
        ));
        assert!(is_media_path(PATH));
        assert!(!is_media_path("data/../../etc/passwd"));
        assert!(!is_media_path("posts.json"));
        assert!(!is_media_path("data/a?b=c"));
        assert!(!is_media_path("data/%2e%2e/x"));
    }
}
