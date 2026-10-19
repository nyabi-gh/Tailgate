//! In-memory cache of fixed-size blocks of media files.
//!
//! WKWebView reads an MP4 from a custom scheme in hundreds of 8–24 byte ranges; a round trip to
//! static1 for each never lets playback start. Ranges are answered from 1 MB blocks that are
//! fetched once (docs/report.md §7.2).

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use bytes::Bytes;
use tauri::http::HeaderValue;
use tokio::sync::OnceCell;

pub const BLOCK_SIZE: u64 = 1024 * 1024;

/// One aligned block of a file. Only the last block of a file is shorter than [`BLOCK_SIZE`].
#[derive(Clone, Debug)]
pub struct Block {
    pub data: Bytes,
    pub file_len: u64,
    pub content_type: Option<HeaderValue>,
}

/// static1 paths contain the file's md5, so a cached block never goes stale.
type Key = (String, u64);

struct Entry {
    block: Arc<OnceCell<Block>>,
    last_used: u64,
    /// Counted against the capacity once the block has arrived; blocks still being fetched are
    /// never evicted.
    size: Option<u64>,
}

#[derive(Default)]
struct State {
    entries: HashMap<Key, Entry>,
    cached_bytes: u64,
    clock: u64,
}

pub struct BlockCache {
    capacity: u64,
    state: Mutex<State>,
}

impl BlockCache {
    pub fn new(capacity: u64) -> Self {
        Self {
            capacity,
            state: Mutex::default(),
        }
    }

    /// Returns the block, running `fetch` only when no other request has it or is fetching it.
    /// A failed fetch is not cached, so the next request tries again.
    pub async fn get<E, F>(&self, path: &str, index: u64, fetch: F) -> Result<Block, E>
    where
        F: AsyncFnOnce() -> Result<Block, E>,
    {
        let key = (path.to_owned(), index);
        let cell = self.cell(&key);
        let mut fetched_here = false;
        let result = cell
            .get_or_try_init(|| {
                fetched_here = true;
                fetch()
            })
            .await
            .cloned();
        match &result {
            Ok(block) if fetched_here => self.admit(&key, &cell, block.data.len() as u64),
            Ok(_) => {}
            Err(_) => self.forget_failed(&key, &cell),
        }
        result
    }

    fn cell(&self, key: &Key) -> Arc<OnceCell<Block>> {
        let mut state = self.state.lock().expect("block cache lock poisoned");
        state.clock += 1;
        let now = state.clock;
        let entry = state.entries.entry(key.clone()).or_insert_with(|| Entry {
            block: Arc::default(),
            last_used: now,
            size: None,
        });
        entry.last_used = now;
        Arc::clone(&entry.block)
    }

    fn admit(&self, key: &Key, cell: &Arc<OnceCell<Block>>, size: u64) {
        let mut state = self.state.lock().expect("block cache lock poisoned");
        let Some(entry) = state.entries.get_mut(key) else {
            return;
        };
        if !Arc::ptr_eq(&entry.block, cell) || entry.size.is_some() {
            return;
        }
        entry.size = Some(size);
        state.cached_bytes += size;
        while state.cached_bytes > self.capacity {
            let Some((oldest, size)) = state
                .entries
                .iter()
                .filter_map(|(key, entry)| Some((key, entry.size?, entry.last_used)))
                .min_by_key(|(_, _, last_used)| *last_used)
                .map(|(key, size, _)| (key.clone(), size))
            else {
                break;
            };
            state.entries.remove(&oldest);
            state.cached_bytes -= size;
        }
    }

    fn forget_failed(&self, key: &Key, cell: &Arc<OnceCell<Block>>) {
        let mut state = self.state.lock().expect("block cache lock poisoned");
        if state
            .entries
            .get(key)
            .is_some_and(|entry| Arc::ptr_eq(&entry.block, cell) && !cell.initialized())
        {
            state.entries.remove(key);
        }
    }

    #[cfg(test)]
    fn cached_bytes(&self) -> u64 {
        self.state.lock().unwrap().cached_bytes
    }

    #[cfg(test)]
    fn contains(&self, path: &str, index: u64) -> bool {
        let state = self.state.lock().unwrap();
        state
            .entries
            .get(&(path.to_owned(), index))
            .is_some_and(|entry| entry.size.is_some())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    fn block(len: usize) -> Block {
        Block {
            data: Bytes::from(vec![0; len]),
            file_len: 10 * BLOCK_SIZE,
            content_type: None,
        }
    }

    #[tokio::test]
    async fn fetches_a_block_once_for_concurrent_requests() {
        let cache = BlockCache::new(u64::MAX);
        let fetches = AtomicUsize::new(0);
        let fetch = async || {
            fetches.fetch_add(1, Ordering::SeqCst);
            tokio::task::yield_now().await;
            Ok::<_, ()>(block(4))
        };
        let (a, b) = tokio::join!(cache.get("a", 0, fetch), cache.get("a", 0, fetch));
        assert!(a.is_ok() && b.is_ok());
        assert_eq!(fetches.load(Ordering::SeqCst), 1);
        assert_eq!(cache.cached_bytes(), 4);
    }

    #[tokio::test]
    async fn does_not_cache_failures() {
        let cache = BlockCache::new(u64::MAX);
        assert!(cache.get("a", 0, async || Err(())).await.is_err());
        assert!(!cache.contains("a", 0));
        assert!(
            cache
                .get("a", 0, async || Ok::<_, ()>(block(4)))
                .await
                .is_ok()
        );
        assert!(cache.contains("a", 0));
    }

    #[tokio::test]
    async fn evicts_the_least_recently_used_blocks() {
        let cache = BlockCache::new(8);
        for index in 0..2 {
            cache
                .get("a", index, async || Ok::<_, ()>(block(4)))
                .await
                .unwrap();
        }
        cache
            .get("a", 0, async || -> Result<Block, ()> { unreachable!() })
            .await
            .unwrap();
        cache
            .get("a", 2, async || Ok::<_, ()>(block(4)))
            .await
            .unwrap();
        assert!(cache.contains("a", 0));
        assert!(!cache.contains("a", 1));
        assert!(cache.contains("a", 2));
        assert_eq!(cache.cached_bytes(), 8);
    }
}
