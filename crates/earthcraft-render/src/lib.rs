//! Parallel CPU globe rendering. No filesystem or UI lifecycle ownership.
use glam::{Vec2, Vec4};
mod tuning;
use earthcraft_core::METERS_PER_LATITUDE_DEGREE;
use earthcraft_core::{
    camera::Basis,
    math::{v3_to_ll, DVec3},
    EARTH_RADIUS_M, VERTICAL_FOV_HALF_TAN,
};
use earthcraft_data::tiles::{
    make_key, unkey, KeyBuild, Lookup, Provider, Tile, TileCache, TileKey, MAX_INFLIGHT,
};
use rayon::prelude::*;
use std::{collections::HashMap, f64::consts::PI, sync::Arc};
use tuning::{
    ATMOSPHERE_BLEND, ATMOSPHERE_COLOR, ATMOSPHERE_HEIGHT_RATIO, ATMOSPHERE_RADIUS_RATIO,
    COLOR_CHANNEL_MAX, DAY_BRIGHTNESS_RANGE, GRID_BLEND, GRID_COLOR, GRID_LINE_WIDTH_PX,
    LOD_SHARPNESS_BIAS, MIN_LOD_INCIDENCE_COSINE, MIN_LOD_LATITUDE_COSINE, MIN_ZOOM_BLEND_FRACTION,
    MISSING_TILE_COLOR, NIGHT_BLUE_TINT, NIGHT_BRIGHTNESS, OPAQUE_ALPHA, RGBA_CHANNELS, RIM_BLEND,
    RIM_COLOR, RIM_FALLOFF_POWER, ROWS_PER_TILE_BATCH, SPACE_COLOR, STAR_BRIGHTNESS_DIVISOR,
    STAR_BRIGHTNESS_MASK, STAR_BRIGHTNESS_SHIFT, STAR_GRID_RESOLUTION, STAR_MIN_BRIGHTNESS,
    STAR_OCCURRENCE_MODULUS, TILE_SIZE_PX, TWILIGHT_OFFSET, TWILIGHT_WIDTH,
    WEB_MERCATOR_MAX_LATITUDE_DEG,
};
// ───────────────────────────── Rendering ─────────────────────────────

pub struct RenderCfg {
    pub provider: Provider,
    pub rel: u16,
    pub grid: bool,
    pub grid_step: f64,
    pub atmo: bool,
    pub sun: Option<DVec3>,
}

struct Local<'a, F> {
    lookup: &'a F,
    map: HashMap<TileKey, LocalTile, KeyBuild>,
}
enum LocalTile {
    Ready(Arc<Tile>),
    Pending,
    Absent(f32),
}
impl<F: Fn(TileKey) -> Lookup> Local<'_, F> {
    fn tile_color(&mut self, k: TileKey, dist: f32, u: f64, v: f64) -> Option<Vec4> {
        match self.map.entry(k).or_insert_with(|| match (self.lookup)(k) {
            Lookup::Ready(tile) => LocalTile::Ready(tile),
            Lookup::Pending => LocalTile::Pending,
            Lookup::Absent => LocalTile::Absent(dist),
        }) {
            LocalTile::Ready(tile) => Some(tile.sample(u, v)),
            LocalTile::Pending => None,
            LocalTile::Absent(nearest) => {
                *nearest = nearest.min(dist);
                None
            }
        }
    }
    fn misses(self) -> impl Iterator<Item = (TileKey, f32)> {
        self.map.into_iter().filter_map(|(key, tile)| match tile {
            LocalTile::Absent(dist) => Some((key, dist)),
            _ => None,
        })
    }
}

fn hash3(a: i64, b: i64, c: i64) -> u64 {
    let mut h = (a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (b as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ (c as u64).wrapping_mul(0x1656_67B1_9E37_79F9);
    h ^= h >> 33;
    h = h.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    h ^ (h >> 33)
}
/// Samples the tile at level `z`; if it has not loaded yet, falls back to coarser (parent) levels.
fn sample_chain(
    loc: &mut Local<impl Fn(TileKey) -> Lookup>,
    prov: Provider,
    rel: u16,
    z: i32,
    u: f64,
    v: f64,
    dist: f32,
) -> Vec4 {
    let mut zz = z;
    while zz >= 0 {
        let n = (1u64 << zz) as f64;
        let tx = (u * n).floor().min(n - 1.0);
        let ty = (v * n).floor().min(n - 1.0);
        let k = make_key(prov, zz as u8, tx as u32, ty as u32, rel);
        if let Some(c) = loc.tile_color(k, dist, u * n - tx, v * n - ty) {
            return c;
        }
        zz -= 1;
    }
    MISSING_TILE_COLOR
}

fn render_row(
    y: usize,
    w: usize,
    h: usize,
    b: &Basis,
    cfg: &RenderCfg,
    loc: &mut Local<impl Fn(TileKey) -> Lookup>,
    row: &mut [u8],
) {
    let ny = 1.0 - (y as f64 + 0.5) / h as f64 * 2.0;
    let pa = 2.0 * VERTICAL_FOV_HALF_TAN / h as f64; // angle per pixel
    let cc = b.c.length_squared();
    let maxz = cfg.provider.max_zoom() as f64;

    let pixels: &mut [[u8; RGBA_CHANNELS]] = bytemuck::cast_slice_mut(row);
    for (x, pixel) in pixels.iter_mut().enumerate() {
        let nx = (x as f64 + 0.5) / w as f64 * 2.0 - 1.0;
        let d = b.dir(nx, ny);
        let bb = b.c.dot(d);
        let disc = bb * bb - (cc - 1.0);
        let mut col: Vec4;

        if disc >= 0.0 && bb < 0.0 {
            let t = -bb - disc.sqrt();
            let p = b.c + d * t;
            let cosinc = (-d.dot(p)).max(0.0);
            let (lat, lon) = v3_to_ll(p);
            let coslat = lat.to_radians().cos().max(MIN_LOD_LATITUDE_COSINE);
            // Level of detail (LOD): softer grazing-angle limit and a sharpness bias,
            // which greatly enlarges the high-resolution area.
            let mpp = t * EARTH_RADIUS_M * pa / cosinc.max(MIN_LOD_INCIDENCE_COSINE).sqrt();
            let zf = ((2.0 * PI * EARTH_RADIUS_M * coslat) / (TILE_SIZE_PX * mpp)).log2();
            let lod = zf + LOD_SHARPNESS_BIAS;
            // Satellite: smooth blend between two levels (no visible circles).
            // Text maps: single, slightly sharper level so labels are not duplicated.
            let (zl, frac) = if cfg.provider.is_imagery() {
                let f = lod.floor();
                (f, lod - f)
            } else {
                (lod.ceil(), 0.0)
            };
            let z0 = zl.clamp(0.0, maxz) as i32;
            let frac = if (z0 as f64) >= maxz {
                0.0
            } else {
                frac as f32
            };

            let latc = lat.clamp(
                -WEB_MERCATOR_MAX_LATITUDE_DEG,
                WEB_MERCATOR_MAX_LATITUDE_DEG,
            );
            let u = ((lon + 180.0) / 360.0).rem_euclid(1.0);
            let s = latc.to_radians().sin();
            let v = 0.5 - ((1.0 + s) / (1.0 - s)).ln() / (4.0 * PI);
            let offset = Vec2::new(x as f32 - w as f32 / 2.0, y as f32 - h as f32 / 2.0);
            let dist = offset.length_squared();

            col = sample_chain(loc, cfg.provider, cfg.rel, z0, u, v, dist);
            if frac > MIN_ZOOM_BLEND_FRACTION {
                let c2 = sample_chain(loc, cfg.provider, cfg.rel, z0 + 1, u, v, dist);
                col = col.lerp_monotonic(c2, frac);
            }

            if cfg.grid {
                let st = cfg.grid_step;
                let px_deg = mpp / METERS_PER_LATITUDE_DEGREE;
                let thr = px_deg * GRID_LINE_WIDTH_PX;
                let dlat = (lat / st - (lat / st).round()).abs() * st;
                let dlon =
                    (lon / st - (lon / st).round()).abs() * st * lat.to_radians().cos().abs();
                if dlat < thr || dlon < thr {
                    col = col.lerp_monotonic(GRID_COLOR, GRID_BLEND);
                }
            }
            if let Some(sun) = cfg.sun {
                let l = p.dot(sun);
                let k = ((l + TWILIGHT_OFFSET) / TWILIGHT_WIDTH).clamp(0.0, 1.0) as f32;
                let k = k * k * (3.0 - 2.0 * k);
                let shade = NIGHT_BRIGHTNESS + DAY_BRIGHTNESS_RANGE * k;
                col = col * shade + Vec4::new(0.0, 0.0, NIGHT_BLUE_TINT * (1.0 - k), 0.0);
            }
            if cfg.atmo {
                let rim = (1.0 - cosinc as f32).powi(RIM_FALLOFF_POWER) * RIM_BLEND;
                col = col.lerp_monotonic(RIM_COLOR, rim);
            }
        } else {
            col = SPACE_COLOR;
            let cell = (
                (d.x * STAR_GRID_RESOLUTION).floor() as i64,
                (d.y * STAR_GRID_RESOLUTION).floor() as i64,
                (d.z * STAR_GRID_RESOLUTION).floor() as i64,
            );
            let hs = hash3(cell.0, cell.1, cell.2);
            if hs.is_multiple_of(STAR_OCCURRENCE_MODULUS) {
                let br = STAR_MIN_BRIGHTNESS
                    + ((hs >> STAR_BRIGHTNESS_SHIFT) & STAR_BRIGHTNESS_MASK) as f32
                        / STAR_BRIGHTNESS_DIVISOR;
                col = Vec4::new(br, br, br, 0.0);
            }
            if cfg.atmo && bb < 0.0 {
                let rho = (cc - bb * bb).max(0.0).sqrt();
                if (1.0..ATMOSPHERE_RADIUS_RATIO).contains(&rho) {
                    let g = (1.0 - (rho - 1.0) / ATMOSPHERE_HEIGHT_RATIO).powi(2) as f32;
                    col = col.lerp_monotonic(ATMOSPHERE_COLOR, g * ATMOSPHERE_BLEND);
                }
            }
        }
        let channels = col.clamp(Vec4::ZERO, Vec4::ONE) * COLOR_CHANNEL_MAX;
        pixel[0] = channels.x as u8;
        pixel[1] = channels.y as u8;
        pixel[2] = channels.z as u8;
        pixel[3] = OPAQUE_ALPHA;
    }
}

/// Renders an RGBA frame and queues missing tiles, coarse zoom levels first.
pub fn render_frame(
    w: usize,
    h: usize,
    basis: &Basis,
    cfg: &RenderCfg,
    cache: &TileCache,
) -> Vec<u8> {
    let (buf, mut list) = render_pixels(w, h, basis, cfg, &|key| cache.lookup(key));

    // Request missing tiles: lowest zoom first, then closest to the center.
    list.sort_by(|a, b| {
        unkey(a.0)
            .1
            .cmp(&unkey(b.0).1)
            .then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    });
    for (k, _) in list {
        if !cache.request(k) && cache.inflight() >= MAX_INFLIGHT {
            break;
        }
    }
    buf
}

fn render_pixels(
    w: usize,
    h: usize,
    basis: &Basis,
    cfg: &RenderCfg,
    lookup: &(impl Fn(TileKey) -> Lookup + Sync),
) -> (Vec<u8>, Vec<(TileKey, f32)>) {
    let mut buf = vec![0u8; w * h * RGBA_CHANNELS];
    let row_bytes = w * RGBA_CHANNELS;
    let per_batch: Vec<Vec<(TileKey, f32)>> = buf
        .par_chunks_mut(row_bytes * ROWS_PER_TILE_BATCH)
        .enumerate()
        .map(|(batch, rows)| {
            // Neighboring rows share tiles. Keep handles only for this small batch,
            // avoiding repeated Moka reads without retaining tiles between frames.
            let mut loc = Local {
                lookup,
                map: HashMap::default(),
            };
            for (offset, row) in rows.chunks_mut(row_bytes).enumerate() {
                render_row(
                    batch * ROWS_PER_TILE_BATCH + offset,
                    w,
                    h,
                    basis,
                    cfg,
                    &mut loc,
                    row,
                );
            }
            loc.misses().collect()
        })
        .collect();

    let mut misses: HashMap<TileKey, f32> = HashMap::new();
    for (k, d) in per_batch.into_iter().flatten() {
        let e = misses.entry(k).or_insert(f32::MAX);
        *e = e.min(d);
    }
    (buf, misses.into_iter().collect())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod benchmarks;
