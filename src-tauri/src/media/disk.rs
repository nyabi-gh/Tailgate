//! Whole images kept on disk between runs (docs/report.md §7.3). Within a run WKWebView keeps
//! the images it loaded and asks for none twice (docs/research.md §7), so this only spares the
//! tunnel after a restart. static1 paths contain the file's md5, so a kept file never goes stale.

use std::{
    io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::SystemTime,
};

use serde::Serialize;
use tokio::sync::{Mutex, OnceCell};

/// A trim goes this far below the limit, so the next few files do not each start another.
const TRIM_TO_PERCENT: u64 = 90;
/// A file is written under this suffix and renamed when complete.
const PARTIAL: &str = "part";

/// What the settings screen shows, in bytes.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheUsage {
    pub used: u64,
    pub limit: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, thiserror::Error)]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum ClearError {
    #[error("could not remove the kept images")]
    Storage,
}

pub struct DiskCache {
    root: PathBuf,
    limit: AtomicU64,
    /// Bytes kept, once the first scan has counted what earlier runs left.
    used: AtomicU64,
    scanned: OnceCell<()>,
    /// Writes, trims and clears take turns, so a clear never leaves a file being written behind.
    changing: Mutex<()>,
}

impl DiskCache {
    pub fn new(root: PathBuf, limit: u64) -> Self {
        Self {
            root,
            limit: AtomicU64::new(limit),
            used: AtomicU64::new(0),
            scanned: OnceCell::new(),
            changing: Mutex::new(()),
        }
    }

    /// The file for a static1 path, which `media::is_media_path` has already checked.
    fn file(&self, path: &str) -> PathBuf {
        self.root.join(path)
    }

    pub async fn get(&self, path: &str) -> Option<Vec<u8>> {
        let file = self.file(path);
        match tokio::fs::read(&file).await {
            Ok(data) => {
                // The modification time stands for the last use, which trims go by.
                let touched = tokio::task::spawn_blocking(move || touch(&file)).await;
                if let Ok(Err(error)) = touched {
                    tracing::debug!("could not mark {path} as used: {error}");
                }
                Some(data)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => {
                tracing::warn!("could not read {path} from the media cache: {error}");
                None
            }
        }
    }

    /// Failures only cost the next run a download, so they are logged and not returned.
    pub async fn put(&self, path: &str, data: &[u8]) {
        self.scan().await;
        let _changing = self.changing.lock().await;
        let file = self.file(path);
        if tokio::fs::try_exists(&file).await.unwrap_or(false) {
            return;
        }
        if let Err(error) = write(&file, data).await {
            tracing::warn!("could not keep {path} in the media cache: {error}");
            return;
        }
        let used = self.used.fetch_add(data.len() as u64, Ordering::Relaxed) + data.len() as u64;
        if used > self.limit.load(Ordering::Relaxed) {
            self.trim().await;
        }
    }

    pub async fn usage(&self) -> u64 {
        self.scan().await;
        self.used.load(Ordering::Relaxed)
    }

    pub fn limit(&self) -> u64 {
        self.limit.load(Ordering::Relaxed)
    }

    pub async fn status(&self) -> CacheUsage {
        CacheUsage {
            used: self.usage().await,
            limit: self.limit(),
        }
    }

    pub async fn set_limit(&self, limit: u64) {
        self.scan().await;
        let _changing = self.changing.lock().await;
        self.limit.store(limit, Ordering::Relaxed);
        if self.used.load(Ordering::Relaxed) > limit {
            self.trim().await;
        }
    }

    pub async fn clear(&self) -> Result<(), ClearError> {
        self.scan().await;
        let _changing = self.changing.lock().await;
        let removed = match tokio::fs::remove_dir_all(&self.root).await {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            other => other,
        };
        // Whatever stayed is counted again rather than guessed.
        let left = list(&self.root).await;
        self.used
            .store(left.iter().map(|kept| kept.size).sum(), Ordering::Relaxed);
        removed.map_err(|error| {
            tracing::warn!("could not clear the media cache: {error}");
            ClearError::Storage
        })
    }

    /// Counts what earlier runs kept and removes files a crash left half written. Runs once.
    pub async fn scan(&self) {
        self.scanned
            .get_or_init(async || {
                let kept = list(&self.root).await;
                self.used
                    .fetch_add(kept.iter().map(|kept| kept.size).sum(), Ordering::Relaxed);
            })
            .await;
    }

    /// Removes the least recently used files until the cache is under the trim target. Called
    /// with `changing` held.
    async fn trim(&self) {
        let target = self.limit.load(Ordering::Relaxed) / 100 * TRIM_TO_PERCENT;
        let mut kept = list(&self.root).await;
        kept.sort_by_key(|kept| kept.used);
        let mut used: u64 = kept.iter().map(|kept| kept.size).sum();
        for oldest in kept {
            if used <= target {
                break;
            }
            match tokio::fs::remove_file(&oldest.path).await {
                Ok(()) => used -= oldest.size,
                Err(error) if error.kind() == io::ErrorKind::NotFound => used -= oldest.size,
                Err(error) => {
                    tracing::warn!("could not remove {}: {error}", oldest.path.display())
                }
            }
        }
        self.used.store(used, Ordering::Relaxed);
    }
}

struct Kept {
    path: PathBuf,
    size: u64,
    used: SystemTime,
}

/// Every complete file under `root`; half-written ones are removed on the way.
async fn list(root: &Path) -> Vec<Kept> {
    let root = root.to_owned();
    tokio::task::spawn_blocking(move || {
        let mut kept = Vec::new();
        let mut folders = vec![root];
        while let Some(folder) = folders.pop() {
            let entries = match std::fs::read_dir(&folder) {
                Ok(entries) => entries,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => {
                    tracing::warn!("could not list {}: {error}", folder.display());
                    continue;
                }
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let Ok(metadata) = entry.metadata() else {
                    continue;
                };
                if metadata.is_dir() {
                    folders.push(path);
                } else if path.extension().is_some_and(|ext| ext == PARTIAL) {
                    if let Err(error) = std::fs::remove_file(&path) {
                        tracing::warn!("could not remove {}: {error}", path.display());
                    }
                } else {
                    kept.push(Kept {
                        size: metadata.len(),
                        used: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                        path,
                    });
                }
            }
        }
        kept
    })
    .await
    .expect("listing the media cache panicked")
}

async fn write(file: &Path, data: &[u8]) -> io::Result<()> {
    if let Some(folder) = file.parent() {
        tokio::fs::create_dir_all(folder).await?;
    }
    let mut partial = file.as_os_str().to_owned();
    partial.push(".");
    partial.push(PARTIAL);
    tokio::fs::write(&partial, data).await?;
    tokio::fs::rename(&partial, file).await
}

fn touch(file: &Path) -> io::Result<()> {
    std::fs::File::options()
        .write(true)
        .open(file)?
        .set_modified(SystemTime::now())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::settings::testing::TempDir;

    const A: &str = "data/preview/aa/aa/a.jpg";
    const B: &str = "data/preview/bb/bb/b.jpg";
    const C: &str = "data/preview/cc/cc/c.jpg";

    fn last_used(cache: &DiskCache, path: &str, ago: Duration) {
        std::fs::File::options()
            .write(true)
            .open(cache.file(path))
            .unwrap()
            .set_modified(SystemTime::now() - ago)
            .unwrap();
    }

    #[tokio::test]
    async fn keeps_files_for_the_next_run() {
        let dir = TempDir::new();
        let cache = DiskCache::new(dir.join("media"), 1000);
        cache.put(A, b"first").await;
        assert_eq!(cache.get(A).await.as_deref(), Some(&b"first"[..]));
        assert_eq!(cache.get(B).await, None);

        let next_run = DiskCache::new(dir.join("media"), 1000);
        assert_eq!(next_run.usage().await, 5);
        assert_eq!(next_run.get(A).await.as_deref(), Some(&b"first"[..]));
    }

    #[tokio::test]
    async fn removes_the_least_recently_used_files_past_the_limit() {
        let dir = TempDir::new();
        let cache = DiskCache::new(dir.join("media"), 1000);
        cache.put(A, &[1; 400]).await;
        cache.put(B, &[2; 400]).await;
        last_used(&cache, A, Duration::from_secs(120));
        last_used(&cache, B, Duration::from_secs(60));
        // Reading A makes B the least recently used.
        assert!(cache.get(A).await.is_some());

        cache.put(C, &[3; 400]).await;
        assert_eq!(cache.usage().await, 800);
        assert!(cache.get(A).await.is_some());
        assert_eq!(cache.get(B).await, None);
        assert!(cache.get(C).await.is_some());
    }

    #[tokio::test]
    async fn a_lower_limit_trims_at_once() {
        let dir = TempDir::new();
        let cache = DiskCache::new(dir.join("media"), 1000);
        cache.put(A, &[1; 400]).await;
        cache.put(B, &[2; 400]).await;
        last_used(&cache, A, Duration::from_secs(60));
        cache.set_limit(500).await;
        assert_eq!(cache.usage().await, 400);
        assert_eq!(cache.get(A).await, None);
        assert_eq!(cache.limit(), 500);
    }

    #[tokio::test]
    async fn clearing_removes_everything() {
        let dir = TempDir::new();
        let cache = DiskCache::new(dir.join("media"), 1000);
        cache.put(A, b"first").await;
        cache.clear().await.unwrap();
        assert_eq!(cache.usage().await, 0);
        assert_eq!(cache.get(A).await, None);
        cache.put(B, b"again").await;
        assert_eq!(cache.usage().await, 5);
    }

    #[tokio::test]
    async fn removes_files_a_crash_left_half_written() {
        let dir = TempDir::new();
        let partial = dir.join("media/data/preview/aa/aa/a.jpg.part");
        std::fs::create_dir_all(partial.parent().unwrap()).unwrap();
        std::fs::write(&partial, b"half").unwrap();
        let cache = DiskCache::new(dir.join("media"), 1000);
        assert_eq!(cache.usage().await, 0);
        assert!(!partial.exists());
    }
}
