//! Serves e621 media files to the webview through the app's own connection.
//!
//! static1.e621.net answers 403 to requests that carry a browser's cross-site fetch metadata
//! (`Sec-Fetch-Dest: image|video`, `Sec-Fetch-Mode: no-cors`, `Sec-Fetch-Site: cross-site`), which
//! every `<img>`/`<video>` in the webview sends. Fetching from Rust, as an API client with its own
//! User-Agent, is how the files are meant to be downloaded. See docs/research.md §7.

use std::sync::Arc;

use tauri::{
    AppHandle, Manager, UriSchemeResponder,
    http::{HeaderValue, Request, Response, StatusCode, header},
};

use crate::network::Network;

pub const SCHEME: &str = "media";
const ORIGIN: &str = "https://static1.e621.net/";
/// Upper bound for one range response, so a `bytes=0-` request for a long video is answered in
/// pieces (the player asks for the rest) instead of being buffered whole in memory.
const MAX_RANGE_BYTES: u64 = 4 * 1024 * 1024;

pub fn handle(app: &AppHandle, request: Request<Vec<u8>>, responder: UriSchemeResponder) {
    let network = Arc::clone(app.state::<Arc<Network>>().inner());
    tauri::async_runtime::spawn(async move {
        let response = fetch(&network, &request).await.unwrap_or_else(|status| {
            Response::builder().status(status).body(Vec::new()).expect("static response parts are valid")
        });
        responder.respond(response);
    });
}

async fn fetch(network: &Network, request: &Request<Vec<u8>>) -> Result<Response<Vec<u8>>, StatusCode> {
    let path = request.uri().path().trim_start_matches('/');
    if !is_media_path(path) {
        return Err(StatusCode::NOT_FOUND);
    }

    let mut upstream = network.http().get(format!("{ORIGIN}{path}"));
    if let Some(range) = request.headers().get(header::RANGE).and_then(|value| value.to_str().ok()) {
        upstream = upstream.header(header::RANGE, cap_range(range));
    }
    let response = upstream.send().await.map_err(|error| {
        tracing::warn!("media request for {path} failed: {error}");
        StatusCode::BAD_GATEWAY
    })?;

    let status = response.status();
    tracing::debug!(
        "media {path} range={:?} -> {status} {:?}",
        request.headers().get(header::RANGE),
        response.headers().get(header::CONTENT_RANGE)
    );
    if !status.is_success() {
        tracing::warn!("static1 answered {status} for {path}");
        return Err(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY));
    }
    let mut builder = Response::builder().status(status.as_u16());
    for name in [header::CONTENT_TYPE, header::CONTENT_RANGE, header::ACCEPT_RANGES] {
        if let Some(value) = response.headers().get(name.as_str()) {
            builder = builder.header(name, HeaderValue::from_bytes(value.as_bytes()).map_err(|_| StatusCode::BAD_GATEWAY)?);
        }
    }
    let body = response.bytes().await.map_err(|error| {
        tracing::warn!("media body for {path} failed: {error}");
        StatusCode::BAD_GATEWAY
    })?;
    builder.body(body.to_vec()).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Only file paths under static1's `data/` tree; anything else could turn the scheme into a way
/// to make the app request arbitrary URLs.
fn is_media_path(path: &str) -> bool {
    path.starts_with("data/")
        && !path.contains("..")
        && path.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"/._-".contains(&byte))
}

/// Narrows a single `bytes=start-[end]` range to [`MAX_RANGE_BYTES`]; other forms pass through.
fn cap_range(range: &str) -> String {
    let Some((start, end)) = range.strip_prefix("bytes=").and_then(|spec| spec.split_once('-')) else {
        return range.to_string();
    };
    let Ok(start) = start.parse::<u64>() else {
        return range.to_string();
    };
    let limit = start + MAX_RANGE_BYTES - 1;
    let end = if end.is_empty() {
        limit
    } else {
        match end.parse::<u64>() {
            Ok(end) => end.min(limit),
            Err(_) => return range.to_string(),
        }
    };
    format!("bytes={start}-{end}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_static_data_paths() {
        assert!(is_media_path("data/f0/9b/f09b5a771b5861c9c207db6d0fa7bce4.png"));
        assert!(is_media_path("data/sample/36/a9/36a9487818d5a477adb0beed756b5c10_720p.mp4"));
        assert!(!is_media_path("data/../../etc/passwd"));
        assert!(!is_media_path("posts.json"));
        assert!(!is_media_path("data/a?b=c"));
        assert!(!is_media_path("data/%2e%2e/x"));
    }

    #[test]
    fn caps_open_and_large_ranges() {
        assert_eq!(cap_range("bytes=0-"), format!("bytes=0-{}", MAX_RANGE_BYTES - 1));
        assert_eq!(cap_range("bytes=100-199"), "bytes=100-199");
        assert_eq!(cap_range("bytes=10-999999999"), format!("bytes=10-{}", 10 + MAX_RANGE_BYTES - 1));
    }

    #[test]
    fn leaves_other_range_forms_alone() {
        assert_eq!(cap_range("bytes=-500"), "bytes=-500");
        assert_eq!(cap_range("bytes=0-1,5-9"), "bytes=0-1,5-9");
    }
}
