use super::{disk::DiskCache, unkey, KeyBuild, Tile, TileKey};
use crate::paths::cache_dir;
use moka::{policy::EvictionPolicy, sync::Cache};
use std::{
    collections::{HashMap, HashSet, VecDeque},
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
    // Always lock after requests; channel messages are wake tickets, not FIFO jobs.
    queue: Mutex<VecDeque<TileKey>>,
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
            queue: Mutex::new(VecDeque::new()),
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
        self.queue.lock().unwrap().push_back(key);
        self.inflight.fetch_add(1, Ordering::Relaxed);
        if self.tx.send(key).is_err() {
            self.queue.lock().unwrap().retain(|&queued| queued != key);
            requests.insert(key, RequestState::Failed(Instant::now()));
            self.inflight.fetch_sub(1, Ordering::Relaxed);
            return false;
        }
        true
    }
    /// Put the latest view first; retire old queued work while keeping active
    /// downloads and the small satellite overview queue. Keys arrive coarse-first.
    pub fn prioritize_view(&self, keys: &[TileKey]) {
        let wanted: HashSet<_> = keys.iter().copied().collect();
        {
            let mut requests = self.requests.lock().unwrap();
            let mut queue = self.queue.lock().unwrap();
            queue.retain(|key| {
                let (provider, zoom, _, _, _) = unkey(*key);
                let keep =
                    wanted.contains(key) || (provider == super::Provider::Satellite && zoom <= 4);
                if !keep && matches!(requests.remove(key), Some(RequestState::Loading(_))) {
                    self.inflight.fetch_sub(1, Ordering::Relaxed);
                }
                keep
            });
            let queued: HashSet<_> = queue.iter().copied().collect();
            let mut ordered: VecDeque<_> = keys
                .iter()
                .copied()
                .filter(|key| queued.contains(key))
                .collect();
            ordered.extend(queue.iter().copied().filter(|key| !wanted.contains(key)));
            *queue = ordered;
        }
        for &key in keys {
            if !self.request(key) && self.inflight() >= MAX_INFLIGHT {
                break;
            }
        }
    }
    fn next_job(&self) -> Option<(TileKey, u64)> {
        let requests = self.requests.lock().unwrap();
        let key = self.queue.lock().unwrap().pop_front()?;
        let Some(RequestState::Loading(epoch)) = requests.get(&key) else {
            return None;
        };
        Some((key, *epoch))
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
        self.queue.lock().unwrap().retain(|&queued| queued != key);
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

struct FetchedTile {
    tile: Tile,
    persistence: Option<(std::path::PathBuf, Vec<u8>)>,
}
async fn decode(bytes: Vec<u8>) -> Result<(Tile, Vec<u8>), crate::Error> {
    crate::cpu::run(crate::cpu::Service::Tiles, move || {
        Tile::decode(&bytes).map(|tile| (tile, bytes))
    })
    .await
}
async fn fetch_tile(k: TileKey, disk: &DiskCache) -> Result<FetchedTile, crate::Error> {
    let (provider, zoom, x, y, release) = unkey(k);
    let path = disk.path(k)?;
    if let Some(bytes) = disk.read(&path).await {
        match decode(bytes).await {
            Ok((tile, _)) => {
                return Ok(FetchedTile {
                    tile,
                    persistence: None,
                })
            }
            Err(_) => {
                let _ = disk.invalidate(&path).await;
            }
        }
    }
    let url = provider.url(zoom, x, y, release);
    if url.is_empty() {
        return Err("empty tile URL".into());
    }
    let bytes = crate::http::body(
        crate::http::client(crate::http::Policy::Tile)?.get(url),
        MAX_TILE_RESPONSE_BYTES,
    )
    .await?;
    let (tile, bytes) = decode(bytes).await?;
    Ok(FetchedTile {
        tile,
        persistence: Some((path, bytes)),
    })
}
async fn worker(
    cache: std::sync::Weak<TileCache>,
    rx: Arc<tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<TileKey>>>,
    repaint: Arc<dyn Fn() + Send + Sync>,
) {
    loop {
        let Some(_ticket) = rx.lock().await.recv().await else {
            return;
        };
        let Some(cache) = cache.upgrade() else {
            return;
        };
        let Some((key, epoch)) = cache.next_job() else {
            continue;
        };
        match fetch_tile(key, &cache.disk).await {
            Ok(FetchedTile { tile, persistence }) => {
                // Publish before optional SQLite persistence, so writes/eviction do
                // not delay imagery readiness. Persistence keeps its original epoch.
                cache.complete(key, Ok(tile));
                repaint();
                if let Some((path, bytes)) = persistence {
                    let _ = cache.disk.write_owned(&path, bytes, epoch).await;
                }
            }
            Err(error) => {
                cache.complete(key, Err(error));
                repaint();
            }
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod benchmarks;
