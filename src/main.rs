// EarthCraft — globo 3D estilo Google Earth (Windows, macOS, Linux)
//
// Renderização: ray-casting em CPU (paralelo com rayon) de uma esfera texturizada
// com tiles Web Mercator baixados sob demanda (Esri / OSM / OpenTopoMap).
// Interface: egui/eframe.
#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

use eframe::egui::{self, Align2, Color32, FontId, Key, PointerButton, Pos2, Rect, Sense, Stroke, Vec2};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;
use std::hash::{BuildHasherDefault, Hash, Hasher};
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const R_KM: f64 = 6371.0088;
const R_M: f64 = R_KM * 1000.0;
const FOV_TAN: f64 = 0.414_213_562_373_095_1; // tan(22.5°) => FOV vertical de 45°
const MIN_ALT: f64 = 0.15; // km
const MAX_ALT: f64 = 40_000.0; // km
const MAX_INFLIGHT: usize = 48;

// ───────────────────────────── Matemática ─────────────────────────────

#[derive(Clone, Copy, Debug)]
struct V3 {
    x: f64,
    y: f64,
    z: f64,
}
impl V3 {
    fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
    fn dot(self, o: V3) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    fn cross(self, o: V3) -> V3 {
        V3::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
    fn norm(self) -> V3 {
        let l = self.dot(self).sqrt();
        self * (1.0 / l)
    }
}
impl std::ops::Add for V3 {
    type Output = V3;
    fn add(self, o: V3) -> V3 {
        V3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl std::ops::Sub for V3 {
    type Output = V3;
    fn sub(self, o: V3) -> V3 {
        V3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl std::ops::Mul<f64> for V3 {
    type Output = V3;
    fn mul(self, k: f64) -> V3 {
        V3::new(self.x * k, self.y * k, self.z * k)
    }
}

fn ll_to_v3(lat: f64, lon: f64) -> V3 {
    let (la, lo) = (lat.to_radians(), lon.to_radians());
    V3::new(la.cos() * lo.cos(), la.cos() * lo.sin(), la.sin())
}
fn v3_to_ll(p: V3) -> (f64, f64) {
    (p.z.clamp(-1.0, 1.0).asin().to_degrees(), p.y.atan2(p.x).to_degrees())
}
fn wrap_lon(l: f64) -> f64 {
    (l + 180.0).rem_euclid(360.0) - 180.0
}
fn haversine_km(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (la1, la2) = (a.0.to_radians(), b.0.to_radians());
    let dlat = la2 - la1;
    let dlon = (b.1 - a.1).to_radians();
    let h = (dlat / 2.0).sin().powi(2) + la1.cos() * la2.cos() * (dlon / 2.0).sin().powi(2);
    2.0 * R_KM * h.sqrt().min(1.0).asin()
}
fn great_circle(a: (f64, f64), b: (f64, f64), n: usize) -> Vec<V3> {
    let (pa, pb) = (ll_to_v3(a.0, a.1), ll_to_v3(b.0, b.1));
    let om = pa.dot(pb).clamp(-1.0, 1.0).acos();
    if om < 1e-9 {
        return vec![pa];
    }
    (0..=n)
        .map(|i| {
            let t = i as f64 / n as f64;
            (pa * ((1.0 - t) * om).sin() + pb * (t * om).sin()) * (1.0 / om.sin())
        })
        .collect()
}
fn fmt_dist(km: f64) -> String {
    if km < 1.0 {
        format!("{:.0} m", km * 1000.0)
    } else {
        format!("{:.2} km", km)
    }
}
fn fmt_alt(km: f64) -> String {
    if km < 1.0 {
        format!("{:.0} m", km * 1000.0)
    } else {
        format!("{:.0} km", km)
    }
}

/// Ponto subsolar (lat, lon) aproximado para o instante Unix dado.
fn subsolar(unix: f64) -> (f64, f64) {
    let n = unix / 86400.0 - 10957.5; // dias desde J2000
    let l = (280.460 + 0.9856474 * n).rem_euclid(360.0);
    let g = (357.528 + 0.9856003 * n).rem_euclid(360.0).to_radians();
    let lam = (l + 1.915 * g.sin() + 0.020 * (2.0 * g).sin()).to_radians();
    let eps = (23.439 - 0.0000004 * n).to_radians();
    let dec = (eps.sin() * lam.sin()).asin();
    let ra = (eps.cos() * lam.sin()).atan2(lam.cos()).to_degrees();
    let gmst = (280.46061837 + 360.98564736629 * n).rem_euclid(360.0);
    (dec.to_degrees(), wrap_lon(ra - gmst))
}
fn now_unix() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

// ───────────────────────────── Câmera ─────────────────────────────

#[derive(Clone, Copy, Debug)]
struct Cam {
    lat: f64,
    lon: f64,
    heading: f64, // graus: direção (azimute) para a qual o "topo" da tela aponta
    alt: f64,     // km acima da superfície
}

#[derive(Clone, Copy)]
struct Basis {
    c: V3, // posição da câmera (raios terrestres)
    f: V3,
    r: V3,
    u: V3,
    aspect: f64,
}

impl Cam {
    fn basis(&self, aspect: f64) -> Basis {
        let n = ll_to_v3(self.lat, self.lon);
        let (la, lo) = (self.lat.to_radians(), self.lon.to_radians());
        let east = V3::new(-lo.sin(), lo.cos(), 0.0);
        let north = V3::new(-la.sin() * lo.cos(), -la.sin() * lo.sin(), la.cos());
        let h = self.heading.to_radians();
        let u = north * h.cos() + east * h.sin();
        let f = n * -1.0;
        let r = f.cross(u);
        Basis { c: n * (1.0 + self.alt / R_KM), f, r, u, aspect }
    }
}

impl Basis {
    fn dir(&self, nx: f64, ny: f64) -> V3 {
        (self.f + self.r * (nx * self.aspect * FOV_TAN) + self.u * (ny * FOV_TAN)).norm()
    }
    fn project(&self, p: V3, rect: Rect) -> Option<Pos2> {
        if p.dot(self.c) < 1.0 {
            return None; // atrás do horizonte
        }
        let v = p - self.c;
        let z = v.dot(self.f);
        if z <= 1e-9 {
            return None;
        }
        let nx = v.dot(self.r) / (z * self.aspect * FOV_TAN);
        let ny = v.dot(self.u) / (z * FOV_TAN);
        Some(Pos2::new(
            rect.center().x + nx as f32 * rect.width() * 0.5,
            rect.center().y - ny as f32 * rect.height() * 0.5,
        ))
    }
    fn pick(&self, rect: Rect, pos: Pos2) -> Option<(f64, f64)> {
        let nx = ((pos.x - rect.min.x) / rect.width()) as f64 * 2.0 - 1.0;
        let ny = 1.0 - ((pos.y - rect.min.y) / rect.height()) as f64 * 2.0;
        let d = self.dir(nx, ny);
        let b = self.c.dot(d);
        let disc = b * b - (self.c.dot(self.c) - 1.0);
        if disc < 0.0 || b > 0.0 {
            return None;
        }
        let t = -b - disc.sqrt();
        Some(v3_to_ll(self.c + d * t))
    }
}

// ───────────────────────────── Tiles ─────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Provider {
    Satellite,
    Streets,
    Topo,
}
impl Provider {
    const ALL: [Provider; 3] = [Provider::Satellite, Provider::Streets, Provider::Topo];
    fn id(self) -> u64 {
        match self {
            Provider::Satellite => 0,
            Provider::Streets => 1,
            Provider::Topo => 2,
        }
    }
    fn from_id(i: u64) -> Provider {
        match i {
            0 => Provider::Satellite,
            1 => Provider::Streets,
            _ => Provider::Topo,
        }
    }
    fn name(self) -> &'static str {
        match self {
            Provider::Satellite => "Satélite (Esri World Imagery)",
            Provider::Streets => "Ruas (OpenStreetMap)",
            Provider::Topo => "Relevo/Topografia (OpenTopoMap)",
        }
    }
    fn dir(self) -> &'static str {
        match self {
            Provider::Satellite => "esri_sat",
            Provider::Streets => "osm",
            Provider::Topo => "opentopo",
        }
    }
    fn max_zoom(self) -> u8 {
        match self {
            Provider::Satellite => 17,
            Provider::Streets => 19,
            Provider::Topo => 17,
        }
    }
    fn url(self, z: u8, x: u32, y: u32) -> String {
        match self {
            Provider::Satellite => format!(
                "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}"
            ),
            Provider::Streets => format!("https://tile.openstreetmap.org/{z}/{x}/{y}.png"),
            Provider::Topo => format!("https://tile.opentopomap.org/{z}/{x}/{y}.png"),
        }
    }
    fn attribution(self) -> &'static str {
        match self {
            Provider::Satellite => "Imagens © Esri, Maxar, Earthstar Geographics",
            Provider::Streets => "© contribuidores do OpenStreetMap",
            Provider::Topo => "© OpenTopoMap (CC-BY-SA), © OpenStreetMap",
        }
    }
}

type TileKey = u64;
fn make_key(p: Provider, z: u8, x: u32, y: u32) -> TileKey {
    (p.id() << 60) | ((z as u64) << 55) | ((x as u64) << 27) | (y as u64)
}
fn unkey(k: TileKey) -> (Provider, u8, u32, u32) {
    (
        Provider::from_id(k >> 60),
        ((k >> 55) & 31) as u8,
        ((k >> 27) & 0xFFF_FFFF) as u32,
        (k & 0x7FF_FFFF) as u32,
    )
}

#[derive(Default)]
struct KeyHasher(u64);
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
type KeyBuild = BuildHasherDefault<KeyHasher>;

struct Tile {
    w: usize,
    h: usize,
    px: Vec<u8>, // RGBA
}
impl Tile {
    fn sample(&self, u: f64, v: f64) -> [f32; 3] {
        let x = (u * self.w as f64 - 0.5).max(0.0);
        let y = (v * self.h as f64 - 0.5).max(0.0);
        let x0 = (x as usize).min(self.w - 1);
        let y0 = (y as usize).min(self.h - 1);
        let x1 = (x0 + 1).min(self.w - 1);
        let y1 = (y0 + 1).min(self.h - 1);
        let fx = ((x - x0 as f64) as f32).min(1.0);
        let fy = ((y - y0 as f64) as f32).min(1.0);
        let p = |xx: usize, yy: usize| {
            let i = (yy * self.w + xx) * 4;
            [self.px[i] as f32, self.px[i + 1] as f32, self.px[i + 2] as f32]
        };
        let (a, b, c, d) = (p(x0, y0), p(x1, y0), p(x0, y1), p(x1, y1));
        let mut out = [0.0; 3];
        for i in 0..3 {
            let top = a[i] + (b[i] - a[i]) * fx;
            let bot = c[i] + (d[i] - c[i]) * fx;
            out[i] = (top + (bot - top) * fy) / 255.0;
        }
        out
    }
}

enum Slot {
    Loading,
    Ready(Arc<Tile>),
    Failed(Instant),
}
enum Lookup {
    Ready(Arc<Tile>),
    Pending,
    Absent,
}

struct TileCache {
    map: Mutex<HashMap<TileKey, Slot, KeyBuild>>,
    tx: Mutex<mpsc::Sender<TileKey>>,
    gen: AtomicU64,
    inflight: AtomicUsize,
}

impl TileCache {
    fn new(ctx: egui::Context) -> Arc<Self> {
        let (tx, rx) = mpsc::channel();
        let rx = Arc::new(Mutex::new(rx));
        let cache = Arc::new(TileCache {
            map: Mutex::new(HashMap::default()),
            tx: Mutex::new(tx),
            gen: AtomicU64::new(0),
            inflight: AtomicUsize::new(0),
        });
        for _ in 0..6 {
            let (c, r, x) = (cache.clone(), rx.clone(), ctx.clone());
            std::thread::spawn(move || worker(c, r, x));
        }
        cache
    }

    fn lookup(&self, k: TileKey) -> Lookup {
        match self.map.lock().unwrap().get(&k) {
            Some(Slot::Ready(t)) => Lookup::Ready(t.clone()),
            Some(Slot::Loading) => Lookup::Pending,
            Some(Slot::Failed(t)) if t.elapsed() < Duration::from_secs(15) => Lookup::Pending,
            _ => Lookup::Absent,
        }
    }

    fn request(&self, k: TileKey) -> bool {
        if self.inflight.load(Ordering::Relaxed) >= MAX_INFLIGHT {
            return false;
        }
        let mut m = self.map.lock().unwrap();
        match m.get(&k) {
            Some(Slot::Loading) | Some(Slot::Ready(_)) => return false,
            Some(Slot::Failed(t)) if t.elapsed() < Duration::from_secs(15) => return false,
            _ => {}
        }
        if m.len() > 2500 {
            let mut n = 0;
            m.retain(|k, s| match s {
                Slot::Ready(_) if unkey(*k).1 > 4 => {
                    n += 1;
                    n % 2 == 0
                }
                _ => true,
            });
        }
        m.insert(k, Slot::Loading);
        drop(m);
        self.inflight.fetch_add(1, Ordering::Relaxed);
        let _ = self.tx.lock().unwrap().send(k);
        true
    }
}

fn cache_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("earthcraft_tiles")
}

fn fetch_tile(agent: &ureq::Agent, k: TileKey) -> Result<Tile, String> {
    let (p, z, x, y) = unkey(k);
    let path = cache_dir().join(p.dir()).join(z.to_string()).join(x.to_string()).join(format!("{y}.tile"));
    let (bytes, from_disk) = match std::fs::read(&path) {
        Ok(b) => (b, true),
        Err(_) => {
            let resp = agent.get(&p.url(z, x, y)).call().map_err(|e| e.to_string())?;
            let mut buf = Vec::new();
            resp.into_reader().take(8_000_000).read_to_end(&mut buf).map_err(|e| e.to_string())?;
            (buf, false)
        }
    };
    let img = image::load_from_memory(&bytes).map_err(|e| e.to_string())?.to_rgba8();
    if !from_disk {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, &bytes);
    }
    Ok(Tile { w: img.width() as usize, h: img.height() as usize, px: img.into_raw() })
}

fn worker(cache: Arc<TileCache>, rx: Arc<Mutex<mpsc::Receiver<TileKey>>>, ctx: egui::Context) {
    let agent = ureq::AgentBuilder::new()
        .user_agent("EarthCraft/0.1 (visualizador educacional de globo 3D)")
        .timeout(Duration::from_secs(15))
        .build();
    loop {
        let k = {
            let g = rx.lock().unwrap();
            match g.recv() {
                Ok(k) => k,
                Err(_) => return,
            }
        };
        let res = fetch_tile(&agent, k);
        {
            let mut m = cache.map.lock().unwrap();
            match res {
                Ok(t) => {
                    m.insert(k, Slot::Ready(Arc::new(t)));
                }
                Err(_) => {
                    m.insert(k, Slot::Failed(Instant::now()));
                }
            }
        }
        cache.inflight.fetch_sub(1, Ordering::Relaxed);
        cache.gen.fetch_add(1, Ordering::Relaxed);
        ctx.request_repaint();
    }
}

// ───────────────────────────── Renderização ─────────────────────────────

struct RenderCfg {
    provider: Provider,
    grid: bool,
    grid_step: f64,
    atmo: bool,
    sun: Option<V3>,
}

struct Local<'a> {
    cache: &'a TileCache,
    map: HashMap<TileKey, Option<Arc<Tile>>, KeyBuild>,
    misses: Vec<(TileKey, f32)>,
}
impl<'a> Local<'a> {
    fn tile_color(&mut self, k: TileKey, dist: f32, u: f64, v: f64) -> Option<[f32; 3]> {
        if !self.map.contains_key(&k) {
            let val = match self.cache.lookup(k) {
                Lookup::Ready(t) => Some(t),
                Lookup::Pending => None,
                Lookup::Absent => {
                    self.misses.push((k, dist));
                    None
                }
            };
            self.map.insert(k, val);
        }
        self.map.get(&k).and_then(|o| o.as_ref()).map(|t| t.sample(u, v))
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
fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Amostra o tile do nível `z`; se ainda não carregou, usa os níveis mais grosseiros (pai).
fn sample_chain(loc: &mut Local, prov: Provider, z: i32, u: f64, v: f64, dist: f32) -> [f32; 3] {
    let mut zz = z;
    while zz >= 0 {
        let n = (1u64 << zz) as f64;
        let tx = (u * n).floor().min(n - 1.0);
        let ty = (v * n).floor().min(n - 1.0);
        let k = make_key(prov, zz as u8, tx as u32, ty as u32);
        if let Some(c) = loc.tile_color(k, dist, u * n - tx, v * n - ty) {
            return c;
        }
        zz -= 1;
    }
    [0.04, 0.09, 0.18]
}

fn render_row(
    y: usize,
    w: usize,
    h: usize,
    b: &Basis,
    cfg: &RenderCfg,
    cache: &TileCache,
    row: &mut [u8],
) -> Vec<(TileKey, f32)> {
    let mut loc = Local { cache, map: HashMap::default(), misses: Vec::new() };
    let ny = 1.0 - (y as f64 + 0.5) / h as f64 * 2.0;
    let pa = 2.0 * FOV_TAN / h as f64; // ângulo por pixel
    let cc = b.c.dot(b.c);
    let maxz = cfg.provider.max_zoom() as f64;

    for x in 0..w {
        let nx = (x as f64 + 0.5) / w as f64 * 2.0 - 1.0;
        let d = b.dir(nx, ny);
        let bb = b.c.dot(d);
        let disc = bb * bb - (cc - 1.0);
        let mut col: [f32; 3];

        if disc >= 0.0 && bb < 0.0 {
            let t = -bb - disc.sqrt();
            let p = b.c + d * t;
            let cosinc = (-d.dot(p)).max(0.0);
            let (lat, lon) = v3_to_ll(p);
            let coslat = lat.to_radians().cos().max(0.02);
            // Nível de detalhe (LOD): limite do grazing mais brando e viés para mais nitidez,
            // o que amplia bastante a área em alta resolução.
            let mpp = t * R_M * pa / cosinc.max(0.2).sqrt();
            let zf = ((2.0 * PI * R_M * coslat) / (256.0 * mpp)).log2();
            let lod = zf + 0.5;
            // Satélite: mistura suave entre dois níveis (sem círculos).
            // Mapas com texto: nível único, um pouco mais nítido, para não duplicar rótulos.
            let (zl, frac) = if cfg.provider == Provider::Satellite {
                let f = lod.floor();
                (f, lod - f)
            } else {
                (lod.ceil(), 0.0)
            };
            let z0 = zl.clamp(0.0, maxz) as i32;
            let frac = if (z0 as f64) >= maxz { 0.0 } else { frac as f32 };

            let latc = lat.clamp(-85.051_128_78, 85.051_128_78);
            let u = ((lon + 180.0) / 360.0).rem_euclid(1.0);
            let s = latc.to_radians().sin();
            let v = 0.5 - ((1.0 + s) / (1.0 - s)).ln() / (4.0 * PI);
            let dist = (x as f32 - w as f32 / 2.0).powi(2) + (y as f32 - h as f32 / 2.0).powi(2);

            col = sample_chain(&mut loc, cfg.provider, z0, u, v, dist);
            if frac > 0.01 {
                let c2 = sample_chain(&mut loc, cfg.provider, z0 + 1, u, v, dist);
                col = mix(col, c2, frac);
            }

            if cfg.grid {
                let st = cfg.grid_step;
                let px_deg = mpp / 111_195.0;
                let thr = px_deg * 0.8;
                let dlat = (lat / st - (lat / st).round()).abs() * st;
                let dlon = (lon / st - (lon / st).round()).abs() * st * lat.to_radians().cos().abs();
                if dlat < thr || dlon < thr {
                    col = mix(col, [1.0, 1.0, 0.6], 0.55);
                }
            }
            if let Some(sun) = cfg.sun {
                let l = p.dot(sun);
                let k = ((l + 0.12) / 0.30).clamp(0.0, 1.0) as f32;
                let k = k * k * (3.0 - 2.0 * k);
                let shade = 0.10 + 0.90 * k;
                col = [col[0] * shade, col[1] * shade, col[2] * shade + 0.015 * (1.0 - k)];
            }
            if cfg.atmo {
                let rim = (1.0 - cosinc as f32).powi(4) * 0.75;
                col = mix(col, [0.35, 0.60, 1.0], rim);
            }
        } else {
            col = [0.0, 0.0, 0.015];
            let cell = ((d.x * 900.0).floor() as i64, (d.y * 900.0).floor() as i64, (d.z * 900.0).floor() as i64);
            let hs = hash3(cell.0, cell.1, cell.2);
            if hs % 1400 == 0 {
                let br = 0.45 + ((hs >> 20) & 255) as f32 / 480.0;
                col = [br, br, br];
            }
            if cfg.atmo && bb < 0.0 {
                let rho = (cc - bb * bb).max(0.0).sqrt();
                if (1.0..1.07).contains(&rho) {
                    let g = (1.0 - (rho - 1.0) / 0.07).powi(2) as f32;
                    col = mix(col, [0.30, 0.55, 1.0], g * 0.9);
                }
            }
        }
        let o = x * 4;
        row[o] = (col[0].clamp(0.0, 1.0) * 255.0) as u8;
        row[o + 1] = (col[1].clamp(0.0, 1.0) * 255.0) as u8;
        row[o + 2] = (col[2].clamp(0.0, 1.0) * 255.0) as u8;
        row[o + 3] = 255;
    }
    loc.misses
}

// ───────────────────────────── Dados do usuário ─────────────────────────────

#[derive(Clone)]
struct Placemark {
    name: String,
    desc: String,
    lat: f64,
    lon: f64,
}
#[derive(Clone)]
struct GeoResult {
    name: String,
    lat: f64,
    lon: f64,
    alt: f64,
}

fn parse_latlon(s: &str) -> Option<(f64, f64)> {
    let cleaned = s.replace([',', ';'], " ");
    let parts: Vec<&str> = cleaned.split_whitespace().collect();
    if parts.len() != 2 {
        return None;
    }
    let (a, b) = (parts[0].parse::<f64>().ok()?, parts[1].parse::<f64>().ok()?);
    if a.abs() <= 90.0 && b.abs() <= 180.0 {
        Some((a, b))
    } else {
        None
    }
}

fn geocode(q: &str) -> Result<Vec<GeoResult>, String> {
    let resp = ureq::get("https://nominatim.openstreetmap.org/search")
        .set("User-Agent", "EarthCraft/0.1 (visualizador educacional de globo 3D)")
        .query("format", "json")
        .query("limit", "6")
        .query("q", q)
        .call()
        .map_err(|e| e.to_string())?;
    let v: serde_json::Value = resp.into_json().map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for it in v.as_array().ok_or("resposta inválida")? {
        let f = |k: &str| it[k].as_str().and_then(|s| s.parse::<f64>().ok());
        let (Some(lat), Some(lon)) = (f("lat"), f("lon")) else { continue };
        let bb: Vec<f64> = it["boundingbox"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x.as_str()?.parse().ok()).collect())
            .unwrap_or_default();
        let alt = if bb.len() == 4 {
            let span = (bb[1] - bb[0]).max((bb[3] - bb[2]) * lat.to_radians().cos().abs());
            (span * 111.2 * 1.6).clamp(0.5, 12_000.0)
        } else {
            5.0
        };
        out.push(GeoResult { name: it["display_name"].as_str().unwrap_or("?").to_string(), lat, lon, alt });
    }
    Ok(out)
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}
fn xml_unescape(s: &str) -> String {
    let s = s.trim();
    let s = s.strip_prefix("<![CDATA[").and_then(|x| x.strip_suffix("]]>")).unwrap_or(s);
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&amp;", "&")
}
fn between<'a>(s: &'a str, a: &str, b: &str) -> Option<&'a str> {
    let i = s.find(a)? + a.len();
    let j = s[i..].find(b)? + i;
    Some(&s[i..j])
}
fn save_kml(path: &str, pms: &[Placemark]) -> std::io::Result<()> {
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<kml xmlns=\"http://www.opengis.net/kml/2.2\">\n<Document>\n<name>EarthCraft</name>\n",
    );
    for p in pms {
        s += &format!(
            "<Placemark><name>{}</name><description>{}</description><Point><coordinates>{},{},0</coordinates></Point></Placemark>\n",
            xml_escape(&p.name),
            xml_escape(&p.desc),
            p.lon,
            p.lat
        );
    }
    s += "</Document>\n</kml>\n";
    std::fs::write(path, s)
}
fn load_kml(path: &str) -> Result<Vec<Placemark>, String> {
    let txt = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    let mut rest = txt.as_str();
    while let Some(i) = rest.find("<Placemark") {
        let after = &rest[i..];
        let end = after.find("</Placemark>").map(|e| e + 12).unwrap_or(after.len());
        let block = &after[..end];
        if let Some(c) = between(block, "<coordinates>", "</coordinates>") {
            let nums: Vec<f64> = c.trim().split(',').filter_map(|x| x.trim().parse().ok()).collect();
            if nums.len() >= 2 {
                out.push(Placemark {
                    name: between(block, "<name>", "</name>").map(xml_unescape).unwrap_or_else(|| "Sem nome".into()),
                    desc: between(block, "<description>", "</description>").map(xml_unescape).unwrap_or_default(),
                    lat: nums[1],
                    lon: nums[0],
                });
            }
        }
        rest = &after[end..];
    }
    Ok(out)
}

// ───────────────────────────── Aplicativo ─────────────────────────────

struct Fly {
    a: Cam,
    b: Cam,
    t: f64,
    dur: f64,
    bump: f64,
}

struct App {
    cam: Cam,
    fly: Option<Fly>,
    cache: Arc<TileCache>,
    provider: Provider,
    show_grid: bool,
    show_atmo: bool,
    show_sun: bool,
    auto_rotate: bool,

    texture: Option<egui::TextureHandle>,
    image: Option<(usize, usize, Vec<u8>)>,
    last_state: u64,
    last_gen: u64,
    last_render: Instant,
    last_interact: Instant,

    query: String,
    results: Vec<GeoResult>,
    search_rx: Option<mpsc::Receiver<Result<Vec<GeoResult>, String>>>,
    status: String,

    placemarks: Vec<Placemark>,
    new_name: String,
    kml_path: String,
    menu_pick: Option<(f64, f64)>,

    ruler: bool,
    ruler_pts: Vec<(f64, f64)>,
    cursor: Option<(f64, f64)>,
    show_help: bool,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut vis = egui::Visuals::dark();
        vis.window_rounding = 6.0.into();
        cc.egui_ctx.set_visuals(vis);
        App {
            cam: Cam { lat: -15.0, lon: -50.0, heading: 0.0, alt: 14_000.0 },
            fly: None,
            cache: TileCache::new(cc.egui_ctx.clone()),
            provider: Provider::Satellite,
            show_grid: false,
            show_atmo: true,
            show_sun: false,
            auto_rotate: false,
            texture: None,
            image: None,
            last_state: 0,
            last_gen: u64::MAX,
            last_render: Instant::now(),
            last_interact: Instant::now(),
            query: String::new(),
            results: vec![],
            search_rx: None,
            status: "Pronto. Arraste para girar, role para aproximar.".into(),
            placemarks: vec![],
            new_name: String::new(),
            kml_path: "placemarks.kml".into(),
            menu_pick: None,
            ruler: false,
            ruler_pts: vec![],
            cursor: None,
            show_help: false,
        }
    }

    fn touch(&mut self) {
        self.last_interact = Instant::now();
    }

    fn fly_to(&mut self, lat: f64, lon: f64, alt: f64) {
        let a = self.cam;
        let b = Cam { lat: lat.clamp(-89.9, 89.9), lon: wrap_lon(lon), heading: 0.0, alt: alt.clamp(MIN_ALT, MAX_ALT) };
        let dist = haversine_km((a.lat, a.lon), (b.lat, b.lon));
        let bump = if dist * 0.7 > a.alt.max(b.alt) {
            (dist * 0.7).min(MAX_ALT).ln() - (a.alt.ln() + b.alt.ln()) / 2.0
        } else {
            0.0
        };
        let dur = (1.0 + dist / 8000.0 * 2.0).clamp(1.0, 3.5);
        self.fly = Some(Fly { a, b, t: 0.0, dur, bump });
    }

    fn step_fly(&mut self, dt: f64) {
        if let Some(mut f) = self.fly.take() {
            f.t += dt / f.dur;
            let t = f.t.min(1.0);
            let s = t * t * (3.0 - 2.0 * t);
            let (a, b) = (f.a, f.b);
            let (la, lb) = (a.alt.ln(), b.alt.ln());
            self.cam = Cam {
                lat: a.lat + (b.lat - a.lat) * s,
                lon: wrap_lon(a.lon + wrap_lon(b.lon - a.lon) * s),
                heading: a.heading + wrap_lon(b.heading - a.heading) * s,
                alt: (la + (lb - la) * s + (PI * t).sin() * f.bump).exp(),
            };
            self.touch();
            if f.t >= 1.0 {
                self.cam = b;
            } else {
                self.fly = Some(f);
            }
        }
    }

    fn deg_per_pt(&self, height_pts: f32) -> f64 {
        let mpp = self.cam.alt * 1000.0 * 2.0 * FOV_TAN / height_pts.max(1.0) as f64;
        mpp / 111_195.0
    }

    fn pan(&mut self, dx: f64, dy: f64, height_pts: f32) {
        let k = self.deg_per_pt(height_pts);
        let h = self.cam.heading.to_radians();
        let de = -dx * k * h.cos() + dy * k * h.sin();
        let dn = dx * k * h.sin() + dy * k * h.cos();
        self.cam.lat = (self.cam.lat + dn).clamp(-89.9, 89.9);
        self.cam.lon = wrap_lon(self.cam.lon + de / self.cam.lat.to_radians().cos().max(0.05));
    }

    fn zoom(&mut self, factor: f64) {
        self.cam.alt = (self.cam.alt * factor).clamp(MIN_ALT, MAX_ALT);
    }

    fn poll_search(&mut self) {
        let Some(rx) = &self.search_rx else { return };
        if let Ok(res) = rx.try_recv() {
            self.search_rx = None;
            match res {
                Ok(v) if v.is_empty() => self.status = "Nenhum resultado encontrado.".into(),
                Ok(v) => {
                    self.status = format!("{} resultado(s).", v.len());
                    let first = v[0].clone();
                    self.results = v;
                    self.fly_to(first.lat, first.lon, first.alt);
                }
                Err(e) => self.status = format!("Erro na busca: {e}"),
            }
        }
    }

    fn start_search(&mut self, ctx: &egui::Context) {
        let q = self.query.trim().to_string();
        if q.is_empty() {
            return;
        }
        if let Some((lat, lon)) = parse_latlon(&q) {
            self.fly_to(lat, lon, 3.0);
            self.status = format!("Indo para {lat:.5}, {lon:.5}");
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.search_rx = Some(rx);
        self.status = "Buscando…".into();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(geocode(&q));
            ctx.request_repaint();
        });
    }

    // ───────── Painéis ─────────

    fn top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.strong("🌍 EarthCraft");
                ui.separator();
                let r = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .hint_text("Buscar lugar, endereço ou “lat, lon”")
                        .desired_width(340.0),
                );
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                if ui.button("🔍 Buscar").clicked() || enter {
                    self.start_search(ctx);
                }
                ui.separator();
                ui.label(&self.status);
            });
        });
    }

    fn side_panel(&mut self, ctx: &egui::Context) {
        let mut goto: Option<(f64, f64, f64)> = None;
        let mut del: Option<usize> = None;
        egui::SidePanel::left("side").default_width(270.0).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::CollapsingHeader::new("Camadas").default_open(true).show(ui, |ui| {
                    for p in Provider::ALL {
                        ui.radio_value(&mut self.provider, p, p.name());
                    }
                    ui.separator();
                    ui.checkbox(&mut self.show_grid, "Grade de latitude/longitude");
                    ui.checkbox(&mut self.show_atmo, "Atmosfera e estrelas");
                    ui.checkbox(&mut self.show_sun, "Luz do sol em tempo real (dia/noite)");
                    ui.checkbox(&mut self.auto_rotate, "Girar automaticamente");
                });

                egui::CollapsingHeader::new("Navegação").default_open(true).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("🧭 Norte para cima").clicked() {
                            self.cam.heading = 0.0;
                        }
                        if ui.button("🌐 Visão global").clicked() {
                            self.fly_to(self.cam.lat, self.cam.lon, 14_000.0);
                        }
                    });
                    if !self.results.is_empty() {
                        ui.separator();
                        ui.label("Resultados da busca:");
                        for r in &self.results {
                            let short: String = r.name.chars().take(48).collect();
                            if ui.button(short).on_hover_text(&r.name).clicked() {
                                goto = Some((r.lat, r.lon, r.alt));
                            }
                        }
                    }
                });

                egui::CollapsingHeader::new("Locais (Placemarks)").default_open(true).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.new_name).hint_text("Nome").desired_width(120.0));
                        if ui.button("➕ No centro").clicked() {
                            let name =
                                if self.new_name.trim().is_empty() { format!("Local {}", self.placemarks.len() + 1) } else { self.new_name.trim().to_string() };
                            self.placemarks.push(Placemark { name, desc: String::new(), lat: self.cam.lat, lon: self.cam.lon });
                            self.new_name.clear();
                        }
                    });
                    for (i, p) in self.placemarks.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut p.name).desired_width(110.0));
                            if ui.small_button("Ir").on_hover_text(format!("{:.5}, {:.5}", p.lat, p.lon)).clicked() {
                                goto = Some((p.lat, p.lon, 3.0));
                            }
                            if ui.small_button("🗑").clicked() {
                                del = Some(i);
                            }
                        });
                    }
                    ui.separator();
                    ui.add(egui::TextEdit::singleline(&mut self.kml_path).desired_width(220.0));
                    ui.horizontal(|ui| {
                        if ui.button("💾 Salvar KML").clicked() {
                            self.status = match save_kml(&self.kml_path, &self.placemarks) {
                                Ok(_) => format!("Salvo em {}", self.kml_path),
                                Err(e) => format!("Erro ao salvar: {e}"),
                            };
                        }
                        if ui.button("📂 Abrir KML").clicked() {
                            match load_kml(&self.kml_path) {
                                Ok(v) => {
                                    self.status = format!("{} local(is) importado(s).", v.len());
                                    self.placemarks.extend(v);
                                }
                                Err(e) => self.status = format!("Erro ao abrir: {e}"),
                            }
                        }
                    });
                });

                egui::CollapsingHeader::new("Régua").default_open(true).show(ui, |ui| {
                    ui.checkbox(&mut self.ruler, "Medir distâncias (clique no globo)");
                    let total: f64 = self.ruler_pts.windows(2).map(|w| haversine_km(w[0], w[1])).sum();
                    ui.label(format!("Pontos: {}   Total: {}", self.ruler_pts.len(), fmt_dist(total)));
                    if ui.button("Limpar régua").clicked() {
                        self.ruler_pts.clear();
                    }
                });

                egui::CollapsingHeader::new("Exportar").default_open(false).show(ui, |ui| {
                    if ui.button("📷 Salvar captura (PNG)").clicked() {
                        self.status = match &self.image {
                            Some((w, h, buf)) => {
                                let name = format!("earthcraft_{}.png", now_unix() as u64);
                                match image::save_buffer(&name, buf, *w as u32, *h as u32, image::ColorType::Rgba8) {
                                    Ok(_) => format!("Captura salva: {name}"),
                                    Err(e) => format!("Erro: {e}"),
                                }
                            }
                            None => "Nada para salvar ainda.".into(),
                        };
                    }
                });

                ui.checkbox(&mut self.show_help, "Mostrar ajuda de controles");
                if self.show_help {
                    ui.label(
                        "• Arrastar (esq.): girar o globo\n\
                         • Botão direito/Shift+arrastar: girar a vista\n\
                         • Roda/pinça ou +/-: zoom\n\
                         • Duplo clique: voar até o ponto\n\
                         • Setas: mover   • N: norte para cima\n\
                         • Botão direito (clique): menu de contexto",
                    );
                }
            });
        });
        if let Some(i) = del {
            self.placemarks.remove(i);
        }
        if let Some((la, lo, al)) = goto {
            self.fly_to(la, lo, al);
        }
    }

    fn bottom_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("bottom").show(ctx, |ui| {
            ui.horizontal(|ui| {
                match self.cursor {
                    Some((la, lo)) => ui.monospace(format!("Cursor: {la:>10.5}°, {lo:>11.5}°")),
                    None => ui.monospace("Cursor: —"),
                };
                ui.separator();
                ui.monospace(format!("Centro: {:.4}°, {:.4}°", self.cam.lat, self.cam.lon));
                ui.separator();
                ui.monospace(format!("Altitude da câmera: {}", fmt_alt(self.cam.alt)));
                ui.separator();
                let n = self.cache.inflight.load(Ordering::Relaxed);
                if n > 0 {
                    ui.monospace(format!("⏳ carregando {n} tiles"));
                } else {
                    ui.monospace("✔ tiles ok");
                }
            });
        });
    }

    // ───────── Globo ─────────

    fn globe_view(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let rect = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(rect, Sense::click_and_drag());
        let ppp = ctx.pixels_per_point();
        let aspect = (rect.width() / rect.height()) as f64;
        let shift = ctx.input(|i| i.modifiers.shift);

        // ── entrada
        let mut interacting = false;
        if resp.dragged_by(PointerButton::Primary) && !shift {
            let d = resp.drag_delta();
            self.fly = None;
            self.pan(d.x as f64, d.y as f64, rect.height());
            interacting = true;
        } else if resp.dragged_by(PointerButton::Secondary) || (resp.dragged_by(PointerButton::Primary) && shift) {
            self.cam.heading = wrap_lon(self.cam.heading - resp.drag_delta().x as f64 * 0.3);
            interacting = true;
        }
        if resp.hovered() {
            let sc = ctx.input(|i| i.smooth_scroll_delta.y) as f64;
            let zd = ctx.input(|i| i.zoom_delta()) as f64;
            if sc != 0.0 {
                self.fly = None;
                self.zoom((-sc * 0.0025).exp());
                interacting = true;
            }
            if (zd - 1.0).abs() > 1e-4 {
                self.zoom(1.0 / zd);
                interacting = true;
            }
        }
        if !ctx.wants_keyboard_input() {
            let dt = ctx.input(|i| i.unstable_dt).min(0.1) as f64;
            let sp = 350.0 * dt;
            let (mut dx, mut dy) = (0.0, 0.0);
            ctx.input(|i| {
                if i.key_down(Key::ArrowLeft) {
                    dx += sp;
                }
                if i.key_down(Key::ArrowRight) {
                    dx -= sp;
                }
                if i.key_down(Key::ArrowUp) {
                    dy += sp;
                }
                if i.key_down(Key::ArrowDown) {
                    dy -= sp;
                }
            });
            if dx != 0.0 || dy != 0.0 {
                self.fly = None;
                self.pan(dx, dy, rect.height());
                interacting = true;
            }
            let (zin, zout, north) = ctx.input(|i| {
                (
                    i.key_down(Key::Plus) || i.key_down(Key::Equals) || i.key_down(Key::PageUp),
                    i.key_down(Key::Minus) || i.key_down(Key::PageDown),
                    i.key_pressed(Key::N),
                )
            });
            if zin {
                self.zoom((-1.2 * dt).exp());
                interacting = true;
            }
            if zout {
                self.zoom((1.2 * dt).exp());
                interacting = true;
            }
            if north {
                self.cam.heading = 0.0;
            }
        }
        if interacting || resp.dragged() {
            self.touch();
        }

        let basis = self.cam.basis(aspect);
        self.cursor = resp.hover_pos().and_then(|p| basis.pick(rect, p));

        if resp.double_clicked() {
            if let Some((la, lo)) = resp.interact_pointer_pos().and_then(|p| basis.pick(rect, p)) {
                self.fly_to(la, lo, (self.cam.alt * 0.35).max(0.3));
            }
        } else if resp.clicked() && self.ruler {
            if let Some(pt) = resp.interact_pointer_pos().and_then(|p| basis.pick(rect, p)) {
                self.ruler_pts.push(pt);
            }
        }
        if resp.secondary_clicked() {
            self.menu_pick = resp.interact_pointer_pos().and_then(|p| basis.pick(rect, p));
        }

        // ── renderização
        let idle = self.last_interact.elapsed() > Duration::from_millis(250) && !resp.dragged();
        let scale = if idle { 1.0 } else { 0.4 };
        let w = ((rect.width() * ppp * scale) as usize).clamp(64, 2000);
        let h = ((w as f32 * rect.height() / rect.width()) as usize).max(32);
        let sun_key = if self.show_sun { (now_unix() / 60.0) as u64 } else { 0 };

        let mut hs = std::collections::hash_map::DefaultHasher::new();
        (
            self.cam.lat.to_bits(),
            self.cam.lon.to_bits(),
            self.cam.heading.to_bits(),
            self.cam.alt.to_bits(),
            w,
            h,
            self.provider.id(),
            self.show_grid,
            self.show_atmo,
            sun_key,
        )
            .hash(&mut hs);
        let state = hs.finish();
        let gen = self.cache.gen.load(Ordering::Relaxed);

        let mut need = state != self.last_state || gen != self.last_gen || self.texture.is_none();
        if need && state == self.last_state && self.texture.is_some() && self.last_render.elapsed() < Duration::from_millis(60) {
            ctx.request_repaint_after(Duration::from_millis(60));
            need = false;
        }
        if need {
            self.render(w, h, aspect, ctx);
            self.last_state = state;
            self.last_gen = gen;
            self.last_render = Instant::now();
        }
        if !idle {
            ctx.request_repaint_after(Duration::from_millis(260));
        }

        // ── desenho
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::BLACK);
        if let Some(tex) = &self.texture {
            painter.image(tex.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        }
        self.draw_overlays(&painter, &basis, rect);

        // bússola
        let ccenter = Pos2::new(rect.right() - 48.0, rect.top() + 56.0);
        painter.circle_filled(ccenter, 24.0, Color32::from_black_alpha(150));
        painter.circle_stroke(ccenter, 24.0, Stroke::new(1.0, Color32::from_gray(160)));
        let hr = self.cam.heading.to_radians() as f32;
        let nd = Vec2::new(-hr.sin(), -hr.cos());
        painter.line_segment([ccenter, ccenter + nd * 16.0], Stroke::new(3.0, Color32::from_rgb(235, 60, 60)));
        painter.line_segment([ccenter, ccenter - nd * 16.0], Stroke::new(3.0, Color32::from_gray(220)));
        painter.text(ccenter + nd * 31.0, Align2::CENTER_CENTER, "N", FontId::proportional(13.0), Color32::WHITE);
        let cresp = ui.interact(Rect::from_center_size(ccenter, Vec2::splat(50.0)), ui.id().with("compass"), Sense::click());
        if cresp.on_hover_text("Clique para alinhar ao norte").clicked() {
            self.cam.heading = 0.0;
        }

        // barra de escala
        if self.cam.alt < 3000.0 {
            let mpp = self.cam.alt * 1000.0 * 2.0 * FOV_TAN / rect.height() as f64;
            let target = mpp * 120.0;
            let pow = 10f64.powf(target.log10().floor());
            let nice = [5.0, 2.0, 1.0].iter().map(|m| m * pow).find(|v| *v <= target).unwrap_or(pow);
            let len = (nice / mpp) as f32;
            let y = rect.bottom() - 28.0;
            let x0 = rect.left() + 18.0;
            painter.line_segment([Pos2::new(x0, y), Pos2::new(x0 + len, y)], Stroke::new(3.0, Color32::BLACK));
            painter.line_segment([Pos2::new(x0, y), Pos2::new(x0 + len, y)], Stroke::new(1.5, Color32::WHITE));
            let label = if nice >= 1000.0 { format!("{} km", nice / 1000.0) } else { format!("{} m", nice) };
            painter.text(Pos2::new(x0, y - 6.0), Align2::LEFT_BOTTOM, label, FontId::proportional(12.0), Color32::WHITE);
        }
        painter.text(
            rect.right_bottom() - Vec2::new(8.0, 6.0),
            Align2::RIGHT_BOTTOM,
            self.provider.attribution(),
            FontId::proportional(11.0),
            Color32::from_white_alpha(200),
        );

        // menu de contexto (botão direito)
        let pick = self.menu_pick;
        let mut act: Option<u8> = None;
        resp.context_menu(|ui| match pick {
            Some((la, lo)) => {
                ui.label(format!("{la:.5}, {lo:.5}"));
                ui.separator();
                if ui.button("📍 Adicionar local aqui").clicked() {
                    act = Some(0);
                    ui.close_menu();
                }
                if ui.button("✈ Voar até aqui").clicked() {
                    act = Some(1);
                    ui.close_menu();
                }
                if ui.button("📏 Adicionar ponto na régua").clicked() {
                    act = Some(2);
                    ui.close_menu();
                }
                if ui.button("📋 Copiar coordenadas").clicked() {
                    act = Some(3);
                    ui.close_menu();
                }
            }
            None => {
                ui.label("Clique sobre o globo.");
            }
        });
        if let (Some(a), Some((la, lo))) = (act, pick) {
            match a {
                0 => self.placemarks.push(Placemark {
                    name: format!("Local {}", self.placemarks.len() + 1),
                    desc: String::new(),
                    lat: la,
                    lon: lo,
                }),
                1 => self.fly_to(la, lo, (self.cam.alt * 0.35).max(0.3)),
                2 => {
                    self.ruler = true;
                    self.ruler_pts.push((la, lo));
                }
                _ => ctx.copy_text(format!("{la:.6}, {lo:.6}")),
            }
        }

        // rotação automática (mantém repaint)
        if self.auto_rotate && self.fly.is_none() {
            let dt = ctx.input(|i| i.unstable_dt).min(0.1) as f64;
            self.cam.lon = wrap_lon(self.cam.lon + 6.0 * dt);
            self.touch();
            ctx.request_repaint();
        }
    }

    fn render(&mut self, w: usize, h: usize, aspect: f64, ctx: &egui::Context) {
        let basis = self.cam.basis(aspect);
        let mpp = self.cam.alt * 1000.0 * 2.0 * FOV_TAN / h as f64;
        let span = (mpp * w as f64 / 111_195.0).min(180.0);
        let steps = [30.0, 10.0, 5.0, 2.0, 1.0, 0.5, 0.2, 0.1, 0.05, 0.01, 0.005, 0.001];
        let grid_step = steps.iter().copied().find(|s| *s <= span / 5.0).unwrap_or(0.001);
        let sun = if self.show_sun {
            let (la, lo) = subsolar(now_unix());
            Some(ll_to_v3(la, lo))
        } else {
            None
        };
        let cfg = RenderCfg { provider: self.provider, grid: self.show_grid, grid_step, atmo: self.show_atmo, sun };

        let mut buf = vec![0u8; w * h * 4];
        let cache = &*self.cache;
        let per_row: Vec<Vec<(TileKey, f32)>> = buf
            .par_chunks_mut(w * 4)
            .enumerate()
            .map(|(y, row)| render_row(y, w, h, &basis, &cfg, cache, row))
            .collect();

        // pede os tiles faltantes: zoom menor primeiro, depois mais perto do centro
        let mut misses: HashMap<TileKey, f32> = HashMap::new();
        for (k, d) in per_row.into_iter().flatten() {
            let e = misses.entry(k).or_insert(f32::MAX);
            *e = e.min(d);
        }
        let mut list: Vec<(TileKey, f32)> = misses.into_iter().collect();
        list.sort_by(|a, b| unkey(a.0).1.cmp(&unkey(b.0).1).then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)));
        let mut seen = HashSet::new();
        for (k, _) in list {
            if seen.insert(k) && !self.cache.request(k) && self.cache.inflight.load(Ordering::Relaxed) >= MAX_INFLIGHT {
                break;
            }
        }

        let img = egui::ColorImage::from_rgba_unmultiplied([w, h], &buf);
        match &mut self.texture {
            Some(t) => t.set(img, egui::TextureOptions::LINEAR),
            None => self.texture = Some(ctx.load_texture("globe", img, egui::TextureOptions::LINEAR)),
        }
        self.image = Some((w, h, buf));
    }

    fn draw_overlays(&self, painter: &egui::Painter, basis: &Basis, rect: Rect) {
        // régua
        let stroke = Stroke::new(2.5, Color32::from_rgb(255, 80, 80));
        for pair in self.ruler_pts.windows(2) {
            let pts: Vec<Option<Pos2>> = great_circle(pair[0], pair[1], 48).into_iter().map(|p| basis.project(p, rect)).collect();
            for s in pts.windows(2) {
                if let (Some(a), Some(b)) = (s[0], s[1]) {
                    painter.line_segment([a, b], stroke);
                }
            }
            if let (Some(a), Some(b)) = (
                basis.project(ll_to_v3(pair[0].0, pair[0].1), rect),
                basis.project(ll_to_v3(pair[1].0, pair[1].1), rect),
            ) {
                let mid = Pos2::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
                let txt = fmt_dist(haversine_km(pair[0], pair[1]));
                painter.text(mid + Vec2::new(1.0, 1.0), Align2::CENTER_BOTTOM, &txt, FontId::proportional(13.0), Color32::BLACK);
                painter.text(mid, Align2::CENTER_BOTTOM, &txt, FontId::proportional(13.0), Color32::WHITE);
            }
        }
        for p in &self.ruler_pts {
            if let Some(s) = basis.project(ll_to_v3(p.0, p.1), rect) {
                painter.circle_filled(s, 4.0, Color32::WHITE);
                painter.circle_stroke(s, 4.0, Stroke::new(1.5, Color32::from_rgb(255, 80, 80)));
            }
        }
        // placemarks
        for pm in &self.placemarks {
            if let Some(s) = basis.project(ll_to_v3(pm.lat, pm.lon), rect) {
                if rect.contains(s) {
                    painter.circle_filled(s, 6.0, Color32::from_rgb(255, 200, 0));
                    painter.circle_stroke(s, 6.0, Stroke::new(1.5, Color32::BLACK));
                    let tp = s + Vec2::new(9.0, -9.0);
                    painter.text(tp + Vec2::new(1.0, 1.0), Align2::LEFT_BOTTOM, &pm.name, FontId::proportional(13.0), Color32::BLACK);
                    painter.text(tp, Align2::LEFT_BOTTOM, &pm.name, FontId::proportional(13.0), Color32::WHITE);
                }
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let dt = ctx.input(|i| i.unstable_dt).min(0.1) as f64;
        self.poll_search();
        self.step_fly(dt);
        if self.fly.is_some() {
            ctx.request_repaint();
        }
        self.top_bar(ctx);
        self.bottom_bar(ctx);
        self.side_panel(ctx);
        egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| self.globe_view(ui, ctx));
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 800.0]).with_title("EarthCraft"),
        ..Default::default()
    };
    eframe::run_native("EarthCraft", options, Box::new(|cc| Box::new(App::new(cc))))
}
