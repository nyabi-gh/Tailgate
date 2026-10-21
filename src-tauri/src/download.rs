//! Saves original files to the download folder through the app's connection.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::{fs, io::AsyncWriteExt};

use crate::network::UserAgent;

const ORIGIN: &str = "https://static1.e621.net/data/";
/// Progress goes to the UI at most once per this many bytes, plus once at the end.
const PROGRESS_STEP: u64 = 256 * 1024;
/// Artist tags that mark a post rather than name who drew it.
const NOT_ARTISTS: &[&str] = &[
    "anonymous_artist",
    "avoid_posting",
    "conditional_dnp",
    "epilepsy_warning",
    "jumpscare_warning",
    "sound_warning",
    "third-party_edit",
    "unknown_artist",
    "unknown_artist_signature",
];

/// What the UI sends: the post's own fields, so Rust needs no cache of posts.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadRequest {
    pub id: u64,
    pub url: String,
    pub ext: String,
    pub artists: Vec<String>,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum Saved {
    Saved { path: PathBuf },
    AlreadySaved { path: PathBuf },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, thiserror::Error)]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum DownloadError {
    #[error("not a static1 file URL")]
    NotMedia,
    #[error("could not reach static1")]
    Network,
    #[error("static1 answered {status}")]
    Http { status: u16 },
    #[error("could not write the file")]
    Storage,
}

pub struct Downloads {
    http: reqwest::Client,
    user_agent: UserAgent,
    folder: PathBuf,
    /// Only files under this prefix are fetched: the URL comes from the webview.
    origin: String,
}

impl Downloads {
    /// `http` should be the app's e621 client, so downloads take the same route and kill switch.
    pub fn new(http: reqwest::Client, user_agent: UserAgent, folder: PathBuf) -> Self {
        Self::with_origin(http, user_agent, folder, ORIGIN.to_owned())
    }

    fn with_origin(
        http: reqwest::Client,
        user_agent: UserAgent,
        folder: PathBuf,
        origin: String,
    ) -> Self {
        Self {
            http,
            user_agent,
            folder,
            origin,
        }
    }

    /// Writes to `<name>.part` and renames it once complete, so a failed download never leaves
    /// a file that looks finished. An existing file of the same name is kept as it is.
    pub async fn save(
        &self,
        request: &DownloadRequest,
        progress: impl Fn(u64, u64),
    ) -> Result<Saved, DownloadError> {
        if !request.url.starts_with(&self.origin) || request.url.contains("..") {
            return Err(DownloadError::NotMedia);
        }
        let target = self.folder.join(file_name(request));
        if fs::try_exists(&target).await.unwrap_or(false) {
            return Ok(Saved::AlreadySaved { path: target });
        }
        fs::create_dir_all(&self.folder)
            .await
            .map_err(storage_error)?;
        let part = target.with_file_name(format!(
            "{}.part",
            target
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
        ));
        let result = self.fetch(request, &part, progress).await;
        match result {
            Ok(()) => {
                fs::rename(&part, &target).await.map_err(storage_error)?;
                Ok(Saved::Saved { path: target })
            }
            Err(error) => {
                remove_partial(&part).await;
                Err(error)
            }
        }
    }

    async fn fetch(
        &self,
        request: &DownloadRequest,
        part: &Path,
        progress: impl Fn(u64, u64),
    ) -> Result<(), DownloadError> {
        let mut response = self
            .http
            .get(&request.url)
            .header(reqwest::header::USER_AGENT, self.user_agent.get())
            .send()
            .await
            .map_err(|error| {
                tracing::warn!("download of post {} failed: {error}", request.id);
                DownloadError::Network
            })?;
        let status = response.status();
        if !status.is_success() {
            tracing::warn!("static1 answered {status} for post {}", request.id);
            return Err(DownloadError::Http {
                status: status.as_u16(),
            });
        }
        let total = response.content_length().unwrap_or(0);
        let mut file = fs::File::create(part).await.map_err(storage_error)?;
        let (mut received, mut reported) = (0u64, 0u64);
        while let Some(chunk) = response.chunk().await.map_err(|error| {
            tracing::warn!("download of post {} broke off: {error}", request.id);
            DownloadError::Network
        })? {
            file.write_all(&chunk).await.map_err(storage_error)?;
            received += chunk.len() as u64;
            if received - reported >= PROGRESS_STEP {
                progress(received, total);
                reported = received;
            }
        }
        file.sync_all().await.map_err(storage_error)?;
        progress(received, total);
        Ok(())
    }
}

/// `{id}_{artist}.{ext}`, with the first artist tag that names a person, or `{id}.{ext}`.
/// Characters a file name may not hold on Windows or macOS become `_`.
fn file_name(request: &DownloadRequest) -> String {
    let artist = request
        .artists
        .iter()
        .find(|artist| !NOT_ARTISTS.contains(&artist.as_str()));
    let stem = match artist {
        Some(artist) => format!("{}_{}", request.id, artist),
        None => request.id.to_string(),
    };
    let clean = |text: &str| -> String {
        text.chars()
            .map(|c| {
                if c.is_alphanumeric() || "-_.()".contains(c) {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    };
    format!("{}.{}", clean(&stem), clean(&request.ext))
}

fn storage_error(error: std::io::Error) -> DownloadError {
    tracing::warn!("download storage failed: {error}");
    DownloadError::Storage
}

/// The download already failed; a leftover `.part` file is only logged.
async fn remove_partial(part: &Path) {
    if let Err(error) = fs::remove_file(part).await
        && error.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!("could not remove {}: {error}", part.display());
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    };

    use tokio::sync::mpsc;

    use super::*;
    use crate::api::testing::{Received, json_response, serve};

    struct Folder(PathBuf);

    impl Folder {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "tailgate-download-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            Self(path)
        }
    }

    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn request(url: &str, artists: &[&str]) -> DownloadRequest {
        DownloadRequest {
            id: 42,
            url: url.to_owned(),
            ext: "png".into(),
            artists: artists.iter().map(|artist| (*artist).to_owned()).collect(),
        }
    }

    /// A local stand-in for static1; the returned origin replaces `https://static1.e621.net/data/`.
    async fn fake_static1(
        folder: &Folder,
        responses: Vec<String>,
    ) -> (Downloads, String, mpsc::UnboundedReceiver<Received>) {
        let (base, received) = serve(responses).await;
        let origin = format!("{base}data/");
        let http = reqwest::Client::builder().no_proxy().build().unwrap();
        (
            Downloads::with_origin(http, UserAgent::new(), folder.0.clone(), origin.clone()),
            origin,
            received,
        )
    }

    #[test]
    fn names_files_after_the_post_and_its_artist() {
        let url = "https://static1.e621.net/data/aa/bb/x.png";
        assert_eq!(
            file_name(&request(url, &["conditional_dnp", "foo_bar"])),
            "42_foo_bar.png"
        );
        assert_eq!(file_name(&request(url, &["sound_warning"])), "42.png");
        assert_eq!(file_name(&request(url, &[])), "42.png");
        assert_eq!(file_name(&request(url, &["a/b:c?"])), "42_a_b_c_.png");
    }

    #[tokio::test]
    async fn refuses_urls_outside_static1() {
        let folder = Folder::new();
        let (downloads, _, _received) = fake_static1(&folder, Vec::new()).await;
        assert_eq!(
            downloads
                .save(&request("https://example.com/x.png", &[]), |_, _| {})
                .await,
            Err(DownloadError::NotMedia)
        );
        let (downloads, origin, _received) = fake_static1(&folder, Vec::new()).await;
        assert_eq!(
            downloads
                .save(&request(&format!("{origin}../x.png"), &[]), |_, _| {})
                .await,
            Err(DownloadError::NotMedia)
        );
    }

    #[tokio::test]
    async fn saves_through_a_part_file_and_reports_progress() {
        let folder = Folder::new();
        let body = "x".repeat(600 * 1024);
        let (downloads, origin, mut received) =
            fake_static1(&folder, vec![json_response("200 OK", &body)]).await;
        let url = format!("{origin}aa/bb/x.png");
        let reports = Mutex::new(Vec::new());
        let saved = downloads
            .save(&request(&url, &["foo"]), |received, total| {
                reports.lock().unwrap().push((received, total));
            })
            .await
            .unwrap();
        let path = folder.0.join("42_foo.png");
        assert_eq!(saved, Saved::Saved { path: path.clone() });
        assert_eq!(std::fs::read(&path).unwrap().len(), body.len());
        assert!(!folder.0.join("42_foo.png.part").exists());
        let reports = reports.into_inner().unwrap();
        assert_eq!(
            reports.last(),
            Some(&(body.len() as u64, body.len() as u64))
        );
        assert!(reports.len() >= 2);
        assert_eq!(received.recv().await.unwrap().target, "/data/aa/bb/x.png");
    }

    #[tokio::test]
    async fn keeps_an_existing_file() {
        let folder = Folder::new();
        std::fs::create_dir_all(&folder.0).unwrap();
        std::fs::write(folder.0.join("42.png"), "old").unwrap();
        let (downloads, origin, _received) = fake_static1(&folder, Vec::new()).await;
        let saved = downloads
            .save(&request(&format!("{origin}aa/bb/x.png"), &[]), |_, _| {})
            .await
            .unwrap();
        assert_eq!(
            saved,
            Saved::AlreadySaved {
                path: folder.0.join("42.png")
            }
        );
        assert_eq!(
            std::fs::read_to_string(folder.0.join("42.png")).unwrap(),
            "old"
        );
    }

    #[tokio::test]
    async fn leaves_nothing_behind_when_the_server_refuses() {
        let folder = Folder::new();
        let (downloads, origin, _received) =
            fake_static1(&folder, vec![json_response("403 Forbidden", "{}")]).await;
        let url = format!("{origin}aa/bb/x.png");
        assert_eq!(
            downloads.save(&request(&url, &[]), |_, _| {}).await,
            Err(DownloadError::Http { status: 403 })
        );
        assert!(!folder.0.join("42.png").exists());
        assert!(!folder.0.join("42.png.part").exists());
    }
}
