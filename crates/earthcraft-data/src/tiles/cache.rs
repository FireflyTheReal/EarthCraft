use super::{disk::DiskCache, unkey, KeyBuild, Tile, TileKey};
use crate::paths::cache_dir;
use moka::{policy::EvictionPolicy, sync::Cache};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
const TILE_WORKER_COUNT: usize = 6;
const FAILED_TILE_RETRY_DELAY: Duration = Duration::from_secs(15);
/// Best-effort decoded-pixel target; Moka may temporarily exceed it during eviction.
pub const DEFAULT_TILE_CACHE_BUDGET_BYTES: usize = 256 * 1024 * 1024;
const MAX_TILE_RESPONSE_BYTES: u64 = 8_000_000;
pub const MAX_INFLIGHT: usize = 48;

enum RequestState {
    Loading(u64),
    Failed(Instant),
}
impl RequestState {
    fn is_pending(&self) -> bool {
        match self {
            Self::Loading(_) => true,
            Self::Failed(time) => time.elapsed() < FAILED_TILE_RETRY_DELAY,
        }
    }
}
pub enum Lookup {
    Ready(Arc<Tile>),
    Pending,
    Absent,
}

pub struct TileCache {
    ready: Cache<TileKey, Arc<Tile>, KeyBuild>,
    // Synchronizes request admission only; decoded-cache hits bypass this lock.
    requests: Mutex<HashMap<TileKey, RequestState, KeyBuild>>,
    budget: usize,
    disk: DiskCache,
    tx: tokio::sync::mpsc::UnboundedSender<TileKey>,
    gen: AtomicU64,
    inflight: AtomicUsize,
}

impl TileCache {
    fn with_budget(tx: tokio::sync::mpsc::UnboundedSender<TileKey>, budget: usize) -> Self {
        Self {
            ready: Cache::builder()
                .name("earthcraft-decoded-tiles")
                .max_capacity(budget as u64)
                .weigher(|_: &TileKey, tile: &Arc<Tile>| {
                    u32::try_from(tile.px.len()).unwrap_or(u32::MAX)
                })
                // Map browsing has a recency bias; retain the existing LRU policy.
                .eviction_policy(EvictionPolicy::lru())
                .build_with_hasher(KeyBuild::default()),
            requests: Mutex::new(HashMap::default()),
            budget,
            disk: DiskCache::new(cache_dir()),
            tx,
            gen: AtomicU64::new(0),
            inflight: AtomicUsize::new(0),
        }
    }
    pub fn new(
        repaint: impl Fn() + Send + Sync + 'static,
        runtime: &tokio::runtime::Handle,
    ) -> Arc<Self> {
        let repaint: Arc<dyn Fn() + Send + Sync> = Arc::new(repaint);
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let rx = Arc::new(tokio::sync::Mutex::new(rx));
        let cache = Arc::new(Self::with_budget(tx, DEFAULT_TILE_CACHE_BUDGET_BYTES));
        for _ in 0..TILE_WORKER_COUNT {
            let (c, r, x) = (cache.clone(), rx.clone(), repaint.clone());
            runtime.spawn(worker(Arc::downgrade(&c), r, x));
        }
        cache
    }

    pub fn lookup(&self, key: TileKey) -> Lookup {
        if let Some(tile) = self.ready.get(&key) {
            return Lookup::Ready(tile);
        }
        match self.requests.lock().unwrap().get(&key) {
            Some(state) if state.is_pending() => Lookup::Pending,
            _ => Lookup::Absent,
        }
    }
    /// Estimated decoded-cache bytes; Moka maintenance may lag inserts and evictions.
    /// Renderer references and other allocations are not included in this metric.
    pub fn resident_bytes(&self) -> usize {
        usize::try_from(self.ready.weighted_size()).unwrap_or(usize::MAX)
    }
    pub fn request(&self, key: TileKey) -> bool {
        self.request_under_limit(key, MAX_INFLIGHT)
    }
    // Check under the admission lock, so background work cannot race past its limit.
    pub(super) fn request_under_limit(&self, key: TileKey, limit: usize) -> bool {
        let mut requests = self.requests.lock().unwrap();
        if self.ready.contains_key(&key)
            || requests.get(&key).is_some_and(RequestState::is_pending)
            || self.inflight.load(Ordering::Relaxed) >= limit.min(MAX_INFLIGHT)
        {
            return false;
        }
        requests.insert(key, RequestState::Loading(self.disk.epoch()));
        self.inflight.fetch_add(1, Ordering::Relaxed);
        if self.tx.send(key).is_err() {
            requests.insert(key, RequestState::Failed(Instant::now()));
            self.inflight.fetch_sub(1, Ordering::Relaxed);
            return false;
        }
        true
    }
    fn complete(&self, key: TileKey, result: Result<Tile, crate::Error>) {
        let admitted = match result {
            Ok(tile) if tile.px.len() <= self.budget => {
                self.ready.insert(key, Arc::new(tile));
                true
            }
            _ => {
                self.ready.invalidate(&key);
                false
            }
        };
        // Publish the decoded tile before removing Loading to avoid duplicate requests.
        let mut requests = self.requests.lock().unwrap();
        let was_loading = matches!(requests.remove(&key), Some(RequestState::Loading(_)));
        requests.retain(|_, state| state.is_pending());
        if !admitted {
            requests.insert(key, RequestState::Failed(Instant::now()));
        }
        if was_loading {
            self.inflight.fetch_sub(1, Ordering::Relaxed);
        }
        self.gen.fetch_add(1, Ordering::Relaxed);
    }
    /// Changes whenever a tile request finishes, including failures.
    pub fn generation(&self) -> u64 {
        self.gen.load(Ordering::Relaxed)
    }
    pub fn inflight(&self) -> usize {
        self.inflight.load(Ordering::Relaxed)
    }
    /// Remove this instance's downloaded tile files while preserving decoded
    /// tiles and other application data. Old jobs cannot write back afterward.
    pub async fn clear_disk_cache(&self) -> Result<(), crate::Error> {
        self.disk.clear().await?;
        Ok(())
    }
    /// Snapshot of encoded file bytes in the managed provider directories.
    /// Filesystem allocation overhead and other application data are excluded.
    pub async fn disk_usage_bytes(&self) -> Result<u64, crate::Error> {
        self.disk.usage_bytes().await
    }
}

async fn fetch_tile(k: TileKey, disk: &DiskCache, epoch: u64) -> Result<Tile, crate::Error> {
    let (p, z, x, y, rel) = unkey(k);
    let path = disk.path(k)?;
    let (bytes, from_disk) = match disk.read(&path).await {
        Some(bytes) => (bytes, true),
        None => {
            let url = p.url(z, x, y, rel);
            if url.is_empty() {
                return Err("empty url".into());
            }
            (
                crate::http::body(
                    crate::http::client(crate::http::Policy::Tile)?.get(url),
                    MAX_TILE_RESPONSE_BYTES,
                )
                .await?,
                false,
            )
        }
    };
    let (tile, bytes) =
        tokio::task::spawn_blocking(move || Tile::decode(&bytes).map(|tile| (tile, bytes)))
            .await??;
    if !from_disk {
        // Optional persistence stays best-effort; a writable disk cache is not
        // required to display successfully downloaded imagery.
        let _ = disk.write(&path, &bytes, epoch).await;
    }
    Ok(tile)
}
async fn worker(
    cache: std::sync::Weak<TileCache>,
    rx: Arc<tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<TileKey>>>,
    repaint: Arc<dyn Fn() + Send + Sync>,
) {
    loop {
        let Some(key) = rx.lock().await.recv().await else {
            return;
        };
        let Some(cache) = cache.upgrade() else {
            return;
        };
        let epoch = {
            let requests = cache.requests.lock().unwrap();
            let Some(RequestState::Loading(epoch)) = requests.get(&key) else {
                continue;
            };
            *epoch
        };
        let result = fetch_tile(key, &cache.disk, epoch).await;
        cache.complete(key, result);
        repaint();
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod benchmarks;
