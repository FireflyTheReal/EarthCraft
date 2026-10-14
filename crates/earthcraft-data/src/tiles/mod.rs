use crate::wayback::wayback_tile_url;
use earthcraft_core::language::Lang;
use glam::Vec4;
use std::hash::{BuildHasherDefault, Hasher};
// Tile-key bit offsets/masks are part of the packed-key contract.
const PROVIDER_SHIFT: u32 = 59;
const ZOOM_SHIFT: u32 = 54;
const RELEASE_SHIFT: u32 = 38;
const X_SHIFT: u32 = 19;
const PROVIDER_MASK: u64 = 7;
const ZOOM_MASK: u64 = 31;
const COORDINATE_MASK: u64 = 0x7FFFF;
const RELEASE_MASK: u64 = 0xFFFF;
// Cache/network policy: preserve both the thresholds and the eviction strategy.

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
            Provider::Satellite => lg.text("provider-satellite-label"),
            Provider::Wayback => lg.text("provider-wayback-label"),
            Provider::Streets => lg.text("provider-streets-label"),
            Provider::Topo => lg.text("provider-topography-label"),
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
            Provider::Satellite | Provider::Wayback => lg.text("provider-imagery-attribution"),
            Provider::Streets => lg.text("provider-streets-attribution"),
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

    /// Decoded RGBA layout for backend texture upload; storage remains tile-owned.
    pub fn rgba(&self) -> (usize, usize, &[u8]) {
        (self.w, self.h, &self.px)
    }

    /// Bilinear RGB in SIMD lanes, with the unused fourth lane set to zero.
    pub fn sample(&self, u: f64, v: f64) -> Vec4 {
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
            Vec4::new(rgba[0] as f32, rgba[1] as f32, rgba[2] as f32, 0.0)
        };
        let (a, b, c, d) = (pixel(x0, y0), pixel(x1, y0), pixel(x0, y1), pixel(x1, y1));
        let top = a.lerp_monotonic(b, fx);
        let bottom = c.lerp_monotonic(d, fx);
        top.lerp_monotonic(bottom, fy) / 255.0
    }
}

mod cache;
mod disk;
pub use cache::{DEFAULT_TILE_CACHE_BUDGET_BYTES, Lookup, MAX_INFLIGHT, TileCache};
mod warmup;
pub use warmup::TileWarmup;

#[cfg(test)]
mod sampling_tests {
    use super::*;
    #[test]
    fn simd_sampling_preserves_pixel_centers_bilinear_weights_and_unused_lane() {
        let tile = Tile {
            w: 2,
            h: 2,
            px: vec![
                255, 0, 0, 255, 0, 255, 0, 0, 0, 0, 255, 128, 255, 255, 255, 64,
            ],
        };
        assert_eq!(tile.sample(0.25, 0.25), Vec4::new(1.0, 0.0, 0.0, 0.0));
        assert_eq!(tile.sample(0.75, 0.25), Vec4::new(0.0, 1.0, 0.0, 0.0));
        assert_eq!(tile.sample(0.5, 0.5), Vec4::new(0.5, 0.5, 0.5, 0.0));
        assert_eq!(tile.sample(-1.0, -1.0), tile.sample(0.25, 0.25));
        assert_eq!(tile.sample(2.0, 2.0), Vec4::new(1.0, 1.0, 1.0, 0.0));
    }
}
