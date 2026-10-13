use super::{unkey, KeyBuild, Provider, Tile, TileKey};
use crate::{paths::cache_dir, wayback::wayback_release};
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
pub const DEFAULT_TILE_CACHE_BUDGET_BYTES: usize = 128 * 1024 * 1024;
const MAX_TILE_RESPONSE_BYTES: u64 = 8_000_000;
pub const MAX_INFLIGHT: usize = 48;
enum Slot {
    Loading,
    Ready { tile: Arc<Tile>, touched: u64 },
    Failed(Instant),
}
struct CacheState {
    entries: HashMap<TileKey, Slot, KeyBuild>,
    bytes: usize,
    budget: usize,
    clock: u64,
}
impl CacheState {
    fn new(budget: usize) -> Self {
        Self {
            entries: HashMap::default(),
            bytes: 0,
            budget,
            clock: 0,
        }
    }
    fn lookup(&mut self, key: TileKey) -> Lookup {
        self.clock = self.clock.wrapping_add(1);
        match self.entries.get_mut(&key) {
            Some(Slot::Ready { tile, touched }) => {
                *touched = self.clock;
                Lookup::Ready(tile.clone())
            }
            Some(Slot::Loading) => Lookup::Pending,
            Some(Slot::Failed(time)) if time.elapsed() < FAILED_TILE_RETRY_DELAY => Lookup::Pending,
            _ => Lookup::Absent,
        }
    }
    fn complete(&mut self, key: TileKey, result: Result<Tile, crate::Error>) {
        if let Some(Slot::Ready { tile, .. }) = self.entries.remove(&key) {
            self.bytes -= tile.px.len();
        }
        self.entries.retain(
            |_, slot| !matches!(slot,Slot::Failed(time) if time.elapsed()>=FAILED_TILE_RETRY_DELAY),
        );
        match result {
            Ok(tile) if tile.px.len() <= self.budget => {
                while self.bytes + tile.px.len() > self.budget {
                    let victim = self
                        .entries
                        .iter()
                        .filter_map(|(&key, slot)| match slot {
                            Slot::Ready { touched, .. } => Some((key, *touched)),
                            _ => None,
                        })
                        .min_by_key(|entry| entry.1)
                        .map(|entry| entry.0);
                    let Some(victim) = victim else {
                        break;
                    };
                    if let Some(Slot::Ready { tile, .. }) = self.entries.remove(&victim) {
                        self.bytes -= tile.px.len();
                    }
                }
                self.clock = self.clock.wrapping_add(1);
                self.bytes += tile.px.len();
                self.entries.insert(
                    key,
                    Slot::Ready {
                        tile: Arc::new(tile),
                        touched: self.clock,
                    },
                );
            }
            _ => {
                self.entries.insert(key, Slot::Failed(Instant::now()));
            }
        }
    }
}
pub enum Lookup {
    Ready(Arc<Tile>),
    Pending,
    Absent,
}

pub struct TileCache {
    map: Mutex<CacheState>,
    tx: tokio::sync::mpsc::UnboundedSender<TileKey>,
    gen: AtomicU64,
    inflight: AtomicUsize,
}

impl TileCache {
    pub fn new(
        repaint: impl Fn() + Send + Sync + 'static,
        runtime: &tokio::runtime::Handle,
    ) -> Arc<Self> {
        let repaint: Arc<dyn Fn() + Send + Sync> = Arc::new(repaint);
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let rx = Arc::new(tokio::sync::Mutex::new(rx));
        let cache = Arc::new(TileCache {
            map: Mutex::new(CacheState::new(DEFAULT_TILE_CACHE_BUDGET_BYTES)),
            tx,
            gen: AtomicU64::new(0),
            inflight: AtomicUsize::new(0),
        });
        for _ in 0..TILE_WORKER_COUNT {
            let (c, r, x) = (cache.clone(), rx.clone(), repaint.clone());
            runtime.spawn(worker(Arc::downgrade(&c), r, x));
        }
        cache
    }

    pub fn lookup(&self, key: TileKey) -> Lookup {
        self.map.lock().unwrap().lookup(key)
    }
    /// Bytes retained by the decoded cache; transient renderer references are separate.
    pub fn resident_bytes(&self) -> usize {
        self.map.lock().unwrap().bytes
    }
    pub fn request(&self, key: TileKey) -> bool {
        let mut state = self.map.lock().unwrap();
        match state.entries.get(&key) {
            Some(Slot::Loading) | Some(Slot::Ready { .. }) => return false,
            Some(Slot::Failed(time)) if time.elapsed() < FAILED_TILE_RETRY_DELAY => return false,
            _ => {}
        }
        if self.inflight.load(Ordering::Relaxed) >= MAX_INFLIGHT {
            return false;
        }
        state.entries.insert(key, Slot::Loading);
        self.inflight.fetch_add(1, Ordering::Relaxed);
        if self.tx.send(key).is_err() {
            state.entries.insert(key, Slot::Failed(Instant::now()));
            self.inflight.fetch_sub(1, Ordering::Relaxed);
            return false;
        }
        true
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
        {
            cache.map.lock().unwrap().complete(key, result);
        }
        cache.inflight.fetch_sub(1, Ordering::Relaxed);
        cache.gen.fetch_add(1, Ordering::Relaxed);
        repaint();
    }
}

impl TileCache {
    /// Changes whenever a tile request finishes, including failures.
    pub fn generation(&self) -> u64 {
        self.gen.load(Ordering::Relaxed)
    }
    pub fn inflight(&self) -> usize {
        self.inflight.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::super::make_key;
    use super::*;
    fn tiny_tile(value: u8) -> Tile {
        Tile {
            w: 1,
            h: 1,
            px: vec![value, value, value, 255],
        }
    }
    #[test]
    fn cache_evicts_least_recently_used_with_a_byte_budget() {
        let mut cache = CacheState::new(8);
        cache.complete(1, Ok(tiny_tile(1)));
        cache.complete(2, Ok(tiny_tile(2)));
        assert!(matches!(cache.lookup(1), Lookup::Ready(_)));
        cache.complete(3, Ok(tiny_tile(3)));
        assert!(matches!(cache.lookup(2), Lookup::Absent));
        assert!(matches!(cache.lookup(1), Lookup::Ready(_)));
        assert_eq!(cache.bytes, 8);
        cache.complete(1, Ok(tiny_tile(4)));
        assert_eq!(cache.bytes, 8);
    }
    #[test]
    fn failed_requests_and_oversized_tiles_do_not_consume_cache_bytes() {
        let mut cache = CacheState::new(4);
        cache.complete(1, Err("network failure".into()));
        cache.complete(
            2,
            Ok(Tile {
                w: 2,
                h: 1,
                px: vec![0; 8],
            }),
        );
        assert_eq!(cache.bytes, 0);
        assert!(matches!(cache.lookup(1), Lookup::Pending));
        assert!(matches!(cache.lookup(2), Lookup::Pending));
    }

    #[test]
    fn tile_key_roundtrip() {
        for (p, z, x, y, rel) in [
            (Provider::Wayback, 17u8, 98_765u32, 123_456u32, 4321u16),
            (Provider::Satellite, 0, 0, 0, 0),
            (Provider::Streets, 19, 524_287, 524_287, 0),
            (Provider::Topo, 5, 17, 9, 0),
        ] {
            let k = make_key(p, z, x, y, rel);
            assert_eq!(unkey(k), (p, z, x, y, rel));
        }
        assert_ne!(
            make_key(Provider::Wayback, 10, 1, 1, 1),
            make_key(Provider::Wayback, 10, 1, 1, 2)
        );
    }
}
