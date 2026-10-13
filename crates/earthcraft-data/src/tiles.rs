use crate::{
    paths::cache_dir,
    wayback::{wayback_release, wayback_tile_url},
};
use earthcraft_core::language::Lang;
use std::{
    collections::HashMap,
    hash::{BuildHasherDefault, Hasher},
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
// Tile-key bit offsets/masks are part of the existing packed-key contract.
const PROVIDER_SHIFT: u32 = 59;
const ZOOM_SHIFT: u32 = 54;
const RELEASE_SHIFT: u32 = 38;
const X_SHIFT: u32 = 19;
const PROVIDER_MASK: u64 = 7;
const ZOOM_MASK: u64 = 31;
const COORDINATE_MASK: u64 = 0x7FFFF;
const RELEASE_MASK: u64 = 0xFFFF;
// Cache/network policy: preserve both the thresholds and the eviction strategy.
const TILE_WORKER_COUNT: usize = 6;
const FAILED_TILE_RETRY_DELAY: Duration = Duration::from_secs(15);
const TILE_CACHE_PRUNE_THRESHOLD: usize = 2500;
const RETAINED_OVERVIEW_MAX_ZOOM: u8 = 4;
const PRUNE_KEEP_EVERY_NTH_TILE: usize = 2;
const MAX_TILE_RESPONSE_BYTES: u64 = 8_000_000;

pub const MAX_INFLIGHT: usize = 48;
// ───────────────────────────── Tiles ─────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Provider {
    Satellite,
    Wayback,
    Streets,
    Topo,
}
impl Provider {
    pub const ALL: [Provider; 4] = [
        Provider::Satellite,
        Provider::Wayback,
        Provider::Streets,
        Provider::Topo,
    ];
    pub fn id(self) -> u64 {
        match self {
            Provider::Satellite => 0,
            Provider::Streets => 1,
            Provider::Topo => 2,
            Provider::Wayback => 3,
        }
    }
    fn from_id(i: u64) -> Provider {
        match i {
            0 => Provider::Satellite,
            1 => Provider::Streets,
            3 => Provider::Wayback,
            _ => Provider::Topo,
        }
    }
    /// Imagery providers get smooth blending between zoom levels.
    pub fn is_imagery(self) -> bool {
        matches!(self, Provider::Satellite | Provider::Wayback)
    }
    pub fn name(self, lg: Lang) -> &'static str {
        match self {
            Provider::Satellite => lg.pick(
                "Satellite (Esri World Imagery)",
                "Satélite (Esri World Imagery)",
            ),
            Provider::Wayback => lg.pick(
                "Historical imagery (Esri Wayback)",
                "Imagens históricas (Esri Wayback)",
            ),
            Provider::Streets => lg.pick("Streets (OpenStreetMap)", "Ruas (OpenStreetMap)"),
            Provider::Topo => lg.pick(
                "Terrain/Topography (OpenTopoMap)",
                "Relevo/Topografia (OpenTopoMap)",
            ),
        }
    }
    fn dir(self) -> &'static str {
        match self {
            Provider::Satellite => "esri_sat",
            Provider::Wayback => "wayback",
            Provider::Streets => "osm",
            Provider::Topo => "opentopo",
        }
    }
    pub fn max_zoom(self) -> u8 {
        match self {
            Provider::Satellite | Provider::Wayback => 17,
            Provider::Streets => 19,
            Provider::Topo => 17,
        }
    }
    fn url(self, z: u8, x: u32, y: u32, rel: u16) -> String {
        match self {
            Provider::Wayback => wayback_tile_url(rel, z, x, y),
            Provider::Satellite => format!(
                "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}"
            ),
            Provider::Streets => format!("https://tile.openstreetmap.org/{z}/{x}/{y}.png"),
            Provider::Topo => format!("https://tile.opentopomap.org/{z}/{x}/{y}.png"),
        }
    }
    pub fn attribution(self, lg: Lang) -> &'static str {
        match self {
            Provider::Satellite | Provider::Wayback => lg.pick(
                "Imagery © Esri, Maxar, Earthstar Geographics",
                "Imagens © Esri, Maxar, Earthstar Geographics",
            ),
            Provider::Streets => lg.pick(
                "© OpenStreetMap contributors",
                "© contribuidores do OpenStreetMap",
            ),
            Provider::Topo => "© OpenTopoMap (CC-BY-SA), © OpenStreetMap",
        }
    }
}

pub type TileKey = u64;
// Key layout: y 19 bits | x 19 bits | release index 16 bits | zoom 5 bits | provider 3 bits.
// `rel` is the index of the Wayback release (0 for every other provider).
pub fn make_key(p: Provider, z: u8, x: u32, y: u32, rel: u16) -> TileKey {
    (p.id() << PROVIDER_SHIFT)
        | ((z as u64) << ZOOM_SHIFT)
        | ((rel as u64) << RELEASE_SHIFT)
        | ((x as u64) << X_SHIFT)
        | (y as u64)
}
pub fn unkey(k: TileKey) -> (Provider, u8, u32, u32, u16) {
    (
        Provider::from_id((k >> PROVIDER_SHIFT) & PROVIDER_MASK),
        ((k >> ZOOM_SHIFT) & ZOOM_MASK) as u8,
        ((k >> X_SHIFT) & COORDINATE_MASK) as u32,
        (k & COORDINATE_MASK) as u32,
        ((k >> RELEASE_SHIFT) & RELEASE_MASK) as u16,
    )
}

#[derive(Default)]
pub struct KeyHasher(u64);
impl Hasher for KeyHasher {
    fn finish(&self) -> u64 {
        self.0 ^ (self.0 >> 29)
    }
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 = (self.0 ^ *b as u64).wrapping_mul(0x100000001b3);
        }
    }
    fn write_u64(&mut self, k: u64) {
        self.0 = k.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
}
pub type KeyBuild = BuildHasherDefault<KeyHasher>;

pub struct Tile {
    w: usize,
    h: usize,
    px: Vec<u8>, // RGBA
}
impl Tile {
    /// Decodes the same PNG/JPEG tile bytes used by the disk and network loaders.
    pub fn decode(bytes: &[u8]) -> Result<Self, crate::Error> {
        let img = image::load_from_memory(bytes)
            .map_err(crate::Error::from)?
            .to_rgba8();
        Ok(Self {
            w: img.width() as usize,
            h: img.height() as usize,
            px: img.into_raw(),
        })
    }

    pub fn sample(&self, u: f64, v: f64) -> [f32; 3] {
        let x = (u * self.w as f64 - 0.5).max(0.0);
        let y = (v * self.h as f64 - 0.5).max(0.0);
        let x0 = (x as usize).min(self.w - 1);
        let y0 = (y as usize).min(self.h - 1);
        let x1 = (x0 + 1).min(self.w - 1);
        let y1 = (y0 + 1).min(self.h - 1);
        let fx = ((x - x0 as f64) as f32).min(1.0);
        let fy = ((y - y0 as f64) as f32).min(1.0);
        let pixels: &[[u8; 4]] = bytemuck::cast_slice(&self.px);
        let pixel = |xx: usize, yy: usize| {
            let rgba = pixels[yy * self.w + xx];
            glam::Vec4::new(rgba[0] as f32, rgba[1] as f32, rgba[2] as f32, 0.0)
        };
        let (a, b, c, d) = (pixel(x0, y0), pixel(x1, y0), pixel(x0, y1), pixel(x1, y1));
        let top = a + (b - a) * fx;
        let bottom = c + (d - c) * fx;
        ((top + (bottom - top) * fy) / 255.0).truncate().to_array()
    }
}

enum Slot {
    Loading,
    Ready(Arc<Tile>),
    Failed(Instant),
}
pub enum Lookup {
    Ready(Arc<Tile>),
    Pending,
    Absent,
}

pub struct TileCache {
    map: Mutex<HashMap<TileKey, Slot, KeyBuild>>,
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
            map: Mutex::new(HashMap::default()),
            tx,
            gen: AtomicU64::new(0),
            inflight: AtomicUsize::new(0),
        });
        for _ in 0..TILE_WORKER_COUNT {
            let (c, r, x) = (cache.clone(), rx.clone(), repaint.clone());
            runtime.spawn(worker(c, r, x));
        }
        cache
    }

    pub fn lookup(&self, k: TileKey) -> Lookup {
        match self.map.lock().unwrap().get(&k) {
            Some(Slot::Ready(t)) => Lookup::Ready(t.clone()),
            Some(Slot::Loading) => Lookup::Pending,
            Some(Slot::Failed(t)) if t.elapsed() < FAILED_TILE_RETRY_DELAY => Lookup::Pending,
            _ => Lookup::Absent,
        }
    }

    pub fn request(&self, k: TileKey) -> bool {
        if self.inflight.load(Ordering::Relaxed) >= MAX_INFLIGHT {
            return false;
        }
        let mut m = self.map.lock().unwrap();
        match m.get(&k) {
            Some(Slot::Loading) | Some(Slot::Ready(_)) => return false,
            Some(Slot::Failed(t)) if t.elapsed() < FAILED_TILE_RETRY_DELAY => return false,
            _ => {}
        }
        if m.len() > TILE_CACHE_PRUNE_THRESHOLD {
            let mut n = 0;
            m.retain(|k, s| match s {
                Slot::Ready(_) if unkey(*k).1 > RETAINED_OVERVIEW_MAX_ZOOM => {
                    n += 1;
                    n % PRUNE_KEEP_EVERY_NTH_TILE == 0
                }
                _ => true,
            });
        }
        m.insert(k, Slot::Loading);
        drop(m);
        self.inflight.fetch_add(1, Ordering::Relaxed);
        let _ = self.tx.send(k);
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
    cache: Arc<TileCache>,
    rx: Arc<tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<TileKey>>>,
    repaint: Arc<dyn Fn() + Send + Sync>,
) {
    loop {
        let Some(key) = rx.lock().await.recv().await else {
            return;
        };
        let result = fetch_tile(key).await;
        {
            let mut map = cache.map.lock().unwrap();
            map.insert(
                key,
                match result {
                    Ok(tile) => Slot::Ready(Arc::new(tile)),
                    Err(_) => Slot::Failed(Instant::now()),
                },
            );
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
    use super::*;
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
