use super::{unkey, KeyBuild, Provider, Tile, TileKey};
use crate::{paths::cache_dir, wayback::wayback_release};
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
pub const DEFAULT_TILE_CACHE_BUDGET_BYTES: usize = 128 * 1024 * 1024;
const MAX_TILE_RESPONSE_BYTES: u64 = 8_000_000;
pub const MAX_INFLIGHT: usize = 48;

enum RequestState {
    Loading,
    Failed(Instant),
}
impl RequestState {
    fn is_pending(&self) -> bool {
        match self {
            Self::Loading => true,
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
        let mut requests = self.requests.lock().unwrap();
        if self.ready.contains_key(&key)
            || requests.get(&key).is_some_and(RequestState::is_pending)
            || self.inflight.load(Ordering::Relaxed) >= MAX_INFLIGHT
        {
            return false;
        }
        requests.insert(key, RequestState::Loading);
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
        let was_loading = matches!(requests.remove(&key), Some(RequestState::Loading));
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
}

async fn fetch_tile(k: TileKey) -> Result<Tile, crate::Error> {
    let (p, z, x, y, rel) = unkey(k);
    let mut base = cache_dir().join(p.dir());
    if p == Provider::Wayback {
        base = base.join(
            wayback_release(rel)
                .ok_or("wayback: release not loaded")?
                .id
                .to_string(),
        );
    }
    let path = base
        .join(z.to_string())
        .join(x.to_string())
        .join(format!("{y}.tile"));
    let (bytes, from_disk) = match tokio::fs::read(&path).await {
        Ok(bytes) => (bytes, true),
        Err(_) => {
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
        if let Some(dir) = path.parent() {
            let _ = tokio::fs::create_dir_all(dir).await;
        }
        let _ = tokio::fs::write(path, bytes).await;
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
        let result = fetch_tile(key).await;
        cache.complete(key, result);
        repaint();
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod benchmarks;
