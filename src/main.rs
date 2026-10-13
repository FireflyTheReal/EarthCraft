// EarthCraft — globo 3D estilo Google Earth (Windows, macOS, Linux)
//
// Rendering: CPU ray-casting (parallel via rayon) of a sphere textured with
// Web Mercator tiles downloaded on demand (Esri / OSM / OpenTopoMap).
// UI: egui/eframe.
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
const FOV_TAN: f64 = 0.414_213_562_373_095_1; // tan(22.5°) => 45° vertical FOV
const MIN_ALT: f64 = 0.15; // km
const MAX_ALT: f64 = 40_000.0; // km
const MAX_INFLIGHT: usize = 48;

// ───────────────────────────── Languages ─────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Lang {
    En,
    PtBr,
}
impl Lang {
    const ALL: [Lang; 2] = [Lang::En, Lang::PtBr];
    fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::PtBr => "pt-BR",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::PtBr => "Português (Brasil)",
        }
    }
    /// Picks the text for the current language.
    fn pick(self, en: &'static str, pt: &'static str) -> &'static str {
        match self {
            Lang::En => en,
            Lang::PtBr => pt,
        }
    }
}
fn lang_file() -> PathBuf {
    cache_dir().join("lang.txt")
}
fn load_lang() -> Lang {
    match std::fs::read_to_string(lang_file()).ok().as_deref().map(str::trim) {
        Some("pt-BR") => Lang::PtBr,
        _ => Lang::En, // English is the default
    }
}
fn save_lang(l: Lang) {
    let _ = std::fs::create_dir_all(cache_dir());
    let _ = std::fs::write(lang_file(), l.code());
}

// ───────────────────────────── Math ─────────────────────────────

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

/// Approximate subsolar point (lat, lon) for the given Unix time.
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

// ───────────────────────────── Camera ─────────────────────────────

#[derive(Clone, Copy, Debug)]
struct Cam {
    lat: f64,
    lon: f64,
    heading: f64, // degrees: bearing the top of the screen points to
    alt: f64,     // km above the surface
}

#[derive(Clone, Copy)]
struct Basis {
    c: V3, // camera position (Earth radii)
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
            return None; // behind the horizon
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
    Wayback,
    Streets,
    Topo,
}
impl Provider {
    const ALL: [Provider; 4] = [Provider::Satellite, Provider::Wayback, Provider::Streets, Provider::Topo];
    fn id(self) -> u64 {
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
    fn is_imagery(self) -> bool {
        matches!(self, Provider::Satellite | Provider::Wayback)
    }
    fn name(self, lg: Lang) -> &'static str {
        match self {
            Provider::Satellite => lg.pick("Satellite (Esri World Imagery)", "Satélite (Esri World Imagery)"),
            Provider::Wayback => lg.pick("Historical imagery (Esri Wayback)", "Imagens históricas (Esri Wayback)"),
            Provider::Streets => lg.pick("Streets (OpenStreetMap)", "Ruas (OpenStreetMap)"),
            Provider::Topo => lg.pick("Terrain/Topography (OpenTopoMap)", "Relevo/Topografia (OpenTopoMap)"),
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
    fn max_zoom(self) -> u8 {
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
    fn attribution(self, lg: Lang) -> &'static str {
        match self {
            Provider::Satellite | Provider::Wayback => lg.pick("Imagery © Esri, Maxar, Earthstar Geographics", "Imagens © Esri, Maxar, Earthstar Geographics"),
            Provider::Streets => lg.pick("© OpenStreetMap contributors", "© contribuidores do OpenStreetMap"),
            Provider::Topo => "© OpenTopoMap (CC-BY-SA), © OpenStreetMap",
        }
    }
}

type TileKey = u64;
// Key layout: y 19 bits | x 19 bits | release index 16 bits | zoom 5 bits | provider 3 bits.
// `rel` is the index of the Wayback release (0 for every other provider).
fn make_key(p: Provider, z: u8, x: u32, y: u32, rel: u16) -> TileKey {
    (p.id() << 59) | ((z as u64) << 54) | ((rel as u64) << 38) | ((x as u64) << 19) | (y as u64)
}
fn unkey(k: TileKey) -> (Provider, u8, u32, u32, u16) {
    (
        Provider::from_id((k >> 59) & 7),
        ((k >> 54) & 31) as u8,
        ((k >> 19) & 0x7FFFF) as u32,
        (k & 0x7FFFF) as u32,
        ((k >> 38) & 0xFFFF) as u16,
    )
}

// ───────────────────────────── Wayback (historical imagery) ─────────────────────────────

#[derive(Clone, Debug)]
struct WaybackRelease {
    id: u32,
    date: String, // YYYY-MM-DD
    url: String,  // template with {level}/{row}/{col}
}
/// Sorted newest first. A tile key stores the index into this list.
static WAYBACK: Mutex<Vec<WaybackRelease>> = Mutex::new(Vec::new());
const WAYBACK_CONFIG: &str = "https://s3-us-west-2.amazonaws.com/config.maptiles.arcgis.com/waybackconfig.json";

fn wayback_release(rel: u16) -> Option<WaybackRelease> {
    WAYBACK.lock().ok().and_then(|v| v.get(rel as usize).cloned())
}
fn wayback_tile_url(rel: u16, z: u8, x: u32, y: u32) -> String {
    match wayback_release(rel) {
        Some(r) => r
            .url
            .replace("{level}", &z.to_string())
            .replace("{row}", &y.to_string())
            .replace("{col}", &x.to_string())
            .replace("{M}", &r.id.to_string()),
        None => String::new(),
    }
}
fn parse_wayback_config(v: &serde_json::Value) -> Vec<WaybackRelease> {
    let mut out = Vec::new();
    let Some(obj) = v.as_object() else { return out };
    for (k, item) in obj {
        let Ok(id) = k.trim().parse::<u32>() else { continue };
        let title = item.get("itemTitle").and_then(|t| t.as_str()).unwrap_or("");
        // "World Imagery (Wayback 2014-02-20)"
        let date = title
            .split("Wayback")
            .nth(1)
            .map(|t| t.trim().trim_end_matches(')').trim().to_string())
            .filter(|d| d.len() >= 10)
            .unwrap_or_else(|| title.to_string());
        if date.is_empty() {
            continue;
        }
        let url = item
            .get("itemURL")
            .and_then(|u| u.as_str())
            .map(|u| u.to_string())
            .unwrap_or_else(|| {
                format!("https://wayback.maptiles.arcgis.com/arcgis/rest/services/World_Imagery/WMTS/1.0.0/default028mm/MapServer/tile/{id}/{{level}}/{{row}}/{{col}}")
            });
        out.push(WaybackRelease { id, date, url });
    }
    out.sort_by(|a, b| b.date.cmp(&a.date).then(b.id.cmp(&a.id)));
    out.truncate(60_000);
    out
}
/// Fetches the release list (network first so that new releases show up; disk cache as fallback).
fn load_wayback_releases() -> Result<Vec<WaybackRelease>, String> {
    let path = geo_cache_path(WAYBACK_CONFIG);
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(60))
        .user_agent("EarthCraft/0.2 (wayback release list)")
        .build();
    let net: Result<serde_json::Value, String> = agent
        .get(WAYBACK_CONFIG)
        .call()
        .map_err(|e| e.to_string())
        .and_then(|r| {
            let mut buf = Vec::new();
            r.into_reader().take(64 << 20).read_to_end(&mut buf).map_err(|e| e.to_string())?;
            serde_json::from_slice::<serde_json::Value>(&buf).map(|v| (v, buf)).map_err(|e| e.to_string())
        })
        .map(|(v, buf)| {
            if let Some(d) = path.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            let _ = std::fs::write(&path, buf);
            v
        });
    let json = match net {
        Ok(v) => v,
        Err(e) => match std::fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()) {
            Some(v) => v,
            None => return Err(e),
        },
    };
    let list = parse_wayback_config(&json);
    if list.is_empty() {
        Err("lista de versões vazia".into())
    } else {
        Ok(list)
    }
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

// ───────────────────────────── Settings ─────────────────────────────

fn user_home() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}
/// Default output folder: `~/<parent>/EarthCraft` (e.g. Pictures/EarthCraft).
fn default_out_dir(parent: &str) -> String {
    let home = user_home();
    let base = if home.join(parent).is_dir() { home.join(parent) } else { home };
    base.join("EarthCraft").to_string_lossy().into_owned()
}

#[derive(Clone, PartialEq)]
struct Settings {
    screenshot_dir: String,
    kml_dir: String,
    ui_scale: f32,  // text size of buttons/panels
    map_scale: f32, // text size of map labels
}
impl Default for Settings {
    fn default() -> Self {
        Settings {
            screenshot_dir: default_out_dir("Pictures"),
            kml_dir: default_out_dir("Documents"),
            ui_scale: 1.0,
            map_scale: 1.0,
        }
    }
}
fn settings_file() -> PathBuf {
    cache_dir().join("settings.json")
}
impl Settings {
    fn load() -> Self {
        let mut st = Settings::default();
        if let Some(v) = std::fs::read(settings_file()).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok()) {
            if let Some(x) = v["screenshot_dir"].as_str().filter(|x| !x.is_empty()) {
                st.screenshot_dir = x.into();
            }
            if let Some(x) = v["kml_dir"].as_str().filter(|x| !x.is_empty()) {
                st.kml_dir = x.into();
            }
            if let Some(x) = v["ui_scale"].as_f64() {
                st.ui_scale = (x as f32).clamp(0.7, 2.5);
            }
            if let Some(x) = v["map_scale"].as_f64() {
                st.map_scale = (x as f32).clamp(0.6, 3.0);
            }
        }
        st
    }
    fn save(&self) {
        let _ = std::fs::create_dir_all(cache_dir());
        let v = serde_json::json!({
            "screenshot_dir": self.screenshot_dir,
            "kml_dir": self.kml_dir,
            "ui_scale": self.ui_scale,
            "map_scale": self.map_scale,
        });
        let _ = std::fs::write(settings_file(), v.to_string());
    }
}

/// Applies the UI text scale to every egui text style (buttons, labels, inputs, headings).
fn apply_ui_scale(ctx: &egui::Context, scale: f32) {
    use egui::{FontFamily::{Monospace, Proportional}, TextStyle};
    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (TextStyle::Small, FontId::new(10.0 * scale, Proportional)),
        (TextStyle::Body, FontId::new(14.0 * scale, Proportional)),
        (TextStyle::Button, FontId::new(14.0 * scale, Proportional)),
        (TextStyle::Heading, FontId::new(19.0 * scale, Proportional)),
        (TextStyle::Monospace, FontId::new(13.0 * scale, Monospace)),
    ]
    .into();
    style.spacing.interact_size.y = (20.0 * scale).max(18.0);
    ctx.set_style(style);
}

fn fetch_tile(agent: &ureq::Agent, k: TileKey) -> Result<Tile, String> {
    let (p, z, x, y, rel) = unkey(k);
    let mut base = cache_dir().join(p.dir());
    if p == Provider::Wayback {
        let r = wayback_release(rel).ok_or("wayback: release not loaded")?;
        base = base.join(r.id.to_string());
    }
    let path = base.join(z.to_string()).join(x.to_string()).join(format!("{y}.tile"));
    let (bytes, from_disk) = match std::fs::read(&path) {
        Ok(b) => (b, true),
        Err(_) => {
            let url = p.url(z, x, y, rel);
            if url.is_empty() {
                return Err("empty url".into());
            }
            let resp = agent.get(&url).call().map_err(|e| e.to_string())?;
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

// ───────────────────────────── Rendering ─────────────────────────────

struct RenderCfg {
    provider: Provider,
    rel: u16,
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

/// Samples the tile at level `z`; if it has not loaded yet, falls back to coarser (parent) levels.
fn sample_chain(loc: &mut Local, prov: Provider, rel: u16, z: i32, u: f64, v: f64, dist: f32) -> [f32; 3] {
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
    let pa = 2.0 * FOV_TAN / h as f64; // angle per pixel
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
            // Level of detail (LOD): softer grazing-angle limit and a sharpness bias,
            // which greatly enlarges the high-resolution area.
            let mpp = t * R_M * pa / cosinc.max(0.2).sqrt();
            let zf = ((2.0 * PI * R_M * coslat) / (256.0 * mpp)).log2();
            let lod = zf + 0.5;
            // Satellite: smooth blend between two levels (no visible circles).
            // Text maps: single, slightly sharper level so labels are not duplicated.
            let (zl, frac) = if cfg.provider.is_imagery() {
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

            col = sample_chain(&mut loc, cfg.provider, cfg.rel, z0, u, v, dist);
            if frac > 0.01 {
                let c2 = sample_chain(&mut loc, cfg.provider, cfg.rel, z0 + 1, u, v, dist);
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

// ───────────────────────────── User data ─────────────────────────────

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

fn geocode(q: &str, lang: &str) -> Result<Vec<GeoResult>, String> {
    let resp = ureq::get("https://nominatim.openstreetmap.org/search")
        .set("User-Agent", "EarthCraft/0.1 (visualizador educacional de globo 3D)")
        .query("format", "json")
        .query("limit", "6")
        .query("accept-language", lang)
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

#[derive(Clone)]
struct Suggestion {
    label: String,
    lat: f64,
    lon: f64,
    alt: f64,
}

/// Search-as-you-type suggestions. Nominatim forbids autocomplete, so Photon (Komoot, built on
/// OpenStreetMap data and meant for this use) is used instead.
fn photon_suggest(q: &str, lang: &str) -> Result<Vec<Suggestion>, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(8))
        .user_agent("EarthCraft/0.2 (search suggestions)")
        .build();
    let mut req = agent.get("https://photon.komoot.io/api/").query("q", q).query("limit", "8");
    if lang == "en" {
        req = req.query("lang", "en"); // Photon only supports en/de/fr/it; others use local names
    }
    let v: serde_json::Value = req.call().map_err(|e| e.to_string())?.into_json().map_err(|e| e.to_string())?;
    let mut out: Vec<Suggestion> = Vec::new();
    for f in v["features"].as_array().into_iter().flatten() {
        let (Some(lon), Some(lat)) = (f.pointer("/geometry/coordinates/0").and_then(|x| x.as_f64()), f.pointer("/geometry/coordinates/1").and_then(|x| x.as_f64())) else { continue };
        let pr = &f["properties"];
        let g = |k: &str| pr[k].as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
        let mut parts: Vec<String> = Vec::new();
        let first = g("name").or_else(|| {
            g("street").map(|st| match g("housenumber") {
                Some(n) => format!("{st}, {n}"),
                None => st,
            })
        });
        parts.extend(first);
        for k in ["district", "city", "state", "country"] {
            if let Some(x) = g(k) {
                if !parts.contains(&x) {
                    parts.push(x);
                }
            }
        }
        if parts.is_empty() {
            continue;
        }
        let ex: Vec<f64> = pr["extent"].as_array().map(|a| a.iter().filter_map(|x| x.as_f64()).collect()).unwrap_or_default();
        let alt = if ex.len() == 4 {
            let span = (ex[1] - ex[3]).abs().max((ex[2] - ex[0]).abs() * lat.to_radians().cos().abs());
            (span * 111.2 * 1.6).clamp(0.5, 12_000.0)
        } else {
            match pr["type"].as_str().unwrap_or("") {
                "country" => 6000.0,
                "state" => 1500.0,
                "county" => 300.0,
                "city" => 40.0,
                "district" | "locality" => 12.0,
                "street" => 2.0,
                "house" => 0.6,
                _ => 5.0,
            }
        };
        let label = parts.join(", ");
        if !out.iter().any(|s| s.label == label) {
            out.push(Suggestion { label, lat, lon, alt });
        }
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
fn load_kml(path: &str, lg: Lang) -> Result<Vec<Placemark>, String> {
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
                    name: between(block, "<name>", "</name>").map(xml_unescape).unwrap_or_else(|| lg.pick("Unnamed", "Sem nome").into()),
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

// ───────────────────────────── Application ─────────────────────────────

struct Fly {
    a: Cam,
    b: Cam,
    t: f64,
    dur: f64,
    bump: f64,
}

type Line = Vec<(f64, f64)>;

#[derive(Default)]
struct BoundaryData {
    countries: Vec<Line>,
    states: Vec<Line>,     // admin-1 of "ordinary" countries (shown at medium zoom)
    states_big: Vec<Line>, // admin-1 of the largest countries (shown even with the whole Earth in view)
    cities: Vec<Line>,
    metro: Vec<Line>,
    labels: Vec<MapLabel>,
}

/// States/provinces of these countries (the 10 largest by area) are visible when zoomed far out.
const BIG_COUNTRIES_A3: [&str; 10] = ["RUS", "CAN", "USA", "CHN", "BRA", "AUS", "IND", "ARG", "KAZ", "DZA"];
const BIG_COUNTRIES_NAME: [&str; 10] = [
    "Russia", "Canada", "United States of America", "China", "Brazil", "Australia", "India", "Argentina", "Kazakhstan", "Algeria",
];
/// Below this camera altitude (km) the states of every other country appear too.
const OTHER_STATES_ALT: f64 = 3000.0;
const BIG_STATE_LABELS_ALT: f64 = 22_000.0;

fn is_big_country(feature: &serde_json::Value) -> bool {
    let Some(props) = feature.get("properties") else { return false };
    for k in ["adm0_a3", "ADM0_A3", "sov_a3", "SOV_A3"] {
        if let Some(a3) = props.get(k).and_then(|v| v.as_str()) {
            return BIG_COUNTRIES_A3.contains(&a3);
        }
    }
    for k in ["admin", "ADMIN", "geonunit"] {
        if let Some(n) = props.get(k).and_then(|v| v.as_str()) {
            return BIG_COUNTRIES_NAME.contains(&n);
        }
    }
    false
}

struct MapLabel { lat: f64, lon: f64, en: String, pt: String, kind: LabelKind, rank: u8, big: bool }
impl MapLabel {
    fn text(&self, lg: Lang) -> &str {
        match lg { Lang::En => &self.en, Lang::PtBr => &self.pt }
    }
    /// Whether the label should be shown at the given camera altitude (km).
    /// Bigger places (lower rank) appear first, smaller ones only when zoomed in.
    fn visible_at(&self, alt: f64) -> bool {
        match self.kind {
            LabelKind::Country => true,
            LabelKind::State => alt < if self.big { BIG_STATE_LABELS_ALT } else { OTHER_STATES_ALT },
            LabelKind::Metro => alt < 5000.0,
            LabelKind::City => alt < match self.rank {
                0..=1 => 9000.0,
                2..=3 => 4000.0,
                4..=5 => 2000.0,
                6..=7 => 900.0,
                _ => 300.0,
            },
        }
    }
}
#[derive(Clone, Copy, PartialEq)]
enum LabelKind { Country, State, City, Metro }

#[derive(Clone, Copy, PartialEq)]
enum Layer { Countries, States, Cities, Metro }

/// One finished piece of overlay data, sent from a loader thread to the UI thread.
struct Patch { layer: Layer, lines: Vec<Line>, lines_big: Vec<Line>, labels: Vec<MapLabel> }

/// Human-readable load log shown in the Layers panel, so failures are never silent.
type StatusLog = Arc<Mutex<Vec<String>>>;

fn log_status(log: &StatusLog, msg: String) {
    if let Ok(mut v) = log.lock() { v.push(msg); }
}

const NE: &str = "https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson";
const IBGE: &str = "https://servicodados.ibge.gov.br/api";
const BR_UFS: [u32; 27] = [11, 12, 13, 14, 15, 16, 17, 21, 22, 23, 24, 25, 26, 27, 28, 29, 31, 32, 33, 35, 41, 42, 43, 50, 51, 52, 53];

// GeoJSON layers are fetched in background threads, one per layer, and each layer is shown as soon
// as it is ready. Every layer has fallback sources, results are cached on disk, and every failure is
// reported in the Layers panel (the old code silently dropped any failure, including downloads
// larger than ureq's default 10 MB `into_string` limit, which is what broke the world layers).
fn load_boundaries_async(log: StatusLog) -> mpsc::Receiver<Patch> {
    let (tx, rx) = mpsc::channel();

    let (t, l) = (tx.clone(), log.clone());
    std::thread::spawn(move || {
        let urls = [
            format!("{NE}/ne_50m_admin_0_countries.geojson"),
            "https://raw.githubusercontent.com/datasets/geo-countries/main/data/countries.geojson".to_string(),
            format!("{NE}/ne_110m_admin_0_countries.geojson"),
        ];
        if let Some(p) = load_polygon_layer(&l, "Países/Countries", &urls, Layer::Countries, LabelKind::Country,
            &["NAME", "ADMIN", "name"], &["NAME_PT", "name_pt", "NAME", "ADMIN"], 0) { let _ = t.send(p); }
    });

    let (t, l) = (tx.clone(), log.clone());
    std::thread::spawn(move || {
        let urls = [
            format!("{NE}/ne_10m_admin_1_states_provinces.geojson"),
            format!("{NE}/ne_50m_admin_1_states_provinces.geojson"),
        ];
        if let Some(p) = load_polygon_layer(&l, "Estados/States", &urls, Layer::States, LabelKind::State,
            &["name_en", "name", "NAME_1", "gn_name"], &["name_pt", "name", "name_en", "NAME_1"], 0) { let _ = t.send(p); }
    });

    let (t, l) = (tx.clone(), log.clone());
    std::thread::spawn(move || {
        let urls = [format!("{NE}/ne_10m_populated_places.geojson"), format!("{NE}/ne_50m_populated_places.geojson")];
        if let Some(p) = load_places(&l, &urls) { let _ = t.send(p); }
    });

    let t = tx.clone();
    std::thread::spawn(move || load_brazil(&log, &t));

    rx
}

fn first_json(log: &StatusLog, what: &str, urls: &[String]) -> Option<serde_json::Value> {
    let mut last = String::new();
    for url in urls {
        match fetch_json(url) {
            Ok(v) => return Some(v),
            Err(e) => last = format!("{url}: {e}"),
        }
    }
    log_status(log, format!("✖ {what}: {last}"));
    None
}

fn load_polygon_layer(log: &StatusLog, what: &str, urls: &[String], layer: Layer, kind: LabelKind,
                      en_keys: &[&str], pt_keys: &[&str], rank: u8) -> Option<Patch> {
    let json = first_json(log, what, urls)?;
    let features = json.get("features").and_then(|v| v.as_array());
    let Some(features) = features else {
        log_status(log, format!("✖ {what}: arquivo sem 'features'"));
        return None;
    };
    let (mut lines, mut lines_big, mut labels) = (Vec::new(), Vec::new(), Vec::new());
    for f in features {
        let Some(geometry) = f.get("geometry") else { continue; };
        let big = layer == Layer::States && is_big_country(f);
        if let Some(c) = geometry.get("coordinates") {
            collect_geojson_lines(c, if big { &mut lines_big } else { &mut lines });
        }
        if let (Some(en), Some((lat, lon))) = (feature_name(f, en_keys), label_center(geometry)) {
            let pt = feature_name(f, pt_keys).unwrap_or_else(|| en.clone());
            labels.push(MapLabel { lat, lon, en, pt, kind, rank, big });
        }
    }
    log_status(log, format!("✔ {what}: {} linhas, {} nomes", lines.len() + lines_big.len(), labels.len()));
    Some(Patch { layer, lines, lines_big, labels })
}

fn load_places(log: &StatusLog, urls: &[String]) -> Option<Patch> {
    let json = first_json(log, "Cidades/Cities", urls)?;
    let mut labels = Vec::new();
    for f in json.get("features").and_then(|v| v.as_array()).into_iter().flatten() {
        let c = f.pointer("/geometry/coordinates").and_then(|v| v.as_array());
        let (Some(c), Some(en)) = (c, feature_name(f, &["NAME", "name", "NAMEASCII"])) else { continue; };
        let (Some(lon), Some(lat)) = (c.first().and_then(|v| v.as_f64()), c.get(1).and_then(|v| v.as_f64())) else { continue; };
        let pt = feature_name(f, &["NAME_PT", "name_pt"]).unwrap_or_else(|| en.clone());
        let props = f.get("properties");
        let rank = props.and_then(|p| p.get("SCALERANK").or_else(|| p.get("scalerank"))).and_then(|v| v.as_f64()).unwrap_or(5.0).clamp(0.0, 8.0) as u8;
        labels.push(MapLabel { lat, lon, en, pt, kind: LabelKind::City, rank, big: false });
    }
    log_status(log, format!("✔ Cidades/Cities: {} nomes", labels.len()));
    Some(Patch { layer: Layer::Cities, lines: vec![], lines_big: vec![], labels })
}

struct Muni { code: u32, name: String, lines: Vec<Line>, center: Option<(f64, f64)> }

fn json_code(v: Option<&serde_json::Value>) -> Option<u32> {
    let v = v?;
    v.as_u64().map(|n| n as u32).or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
}

fn load_brazil_state(uf: u32) -> Result<Vec<Muni>, String> {
    let mut names: HashMap<u32, String> = HashMap::new();
    if let Ok(v) = fetch_json(&format!("{IBGE}/v1/localidades/estados/{uf}/municipios")) {
        for m in v.as_array().into_iter().flatten() {
            if let (Some(id), Some(n)) = (json_code(m.get("id")), m.get("nome").and_then(|n| n.as_str())) {
                names.insert(id, n.to_owned());
            }
        }
    }
    let urls = [
        format!("{IBGE}/v3/malhas/estados/{uf}?intrarregiao=municipio&qualidade=intermediaria&formato=application/vnd.geo+json"),
        format!("{IBGE}/v3/malhas/estados/{uf}?intrarregiao=municipio&qualidade=minima&formato=application/vnd.geo+json"),
        format!("https://raw.githubusercontent.com/tbrugz/geodata-br/master/geojson/geojs-{uf}-mun.json"),
    ];
    let mut last = String::new();
    for url in &urls {
        let json = match fetch_json(url) { Ok(j) => j, Err(e) => { last = e; continue; } };
        let mut out = Vec::new();
        for f in json.get("features").and_then(|v| v.as_array()).into_iter().flatten() {
            let props = f.get("properties");
            let code = json_code(props.and_then(|p| p.get("codarea").or_else(|| p.get("id")).or_else(|| p.get("CD_MUN"))));
            let Some(code) = code else { continue; };
            let Some(geometry) = f.get("geometry") else { continue; };
            let mut lines = Vec::new();
            if let Some(c) = geometry.get("coordinates") { collect_geojson_lines(c, &mut lines); }
            let name = names.get(&code).cloned()
                .or_else(|| feature_name(f, &["name", "NM_MUN", "nome"]))
                .unwrap_or_default();
            out.push(Muni { code, name, lines, center: label_center(geometry) });
        }
        if !out.is_empty() { return Ok(out); }
        last = "sem municípios no arquivo".into();
    }
    Err(format!("UF {uf}: {last}"))
}

// Brazil: municipality outlines/names from IBGE (official API, GitHub mirror as fallback), and
// metropolitan regions built from IBGE's list of member municipalities (outer outline only).
fn load_brazil(log: &StatusLog, tx: &mpsc::Sender<Patch>) {
    let results: Vec<Result<Vec<Muni>, String>> = BR_UFS.par_iter().map(|&uf| load_brazil_state(uf)).collect();
    let mut munis: Vec<Muni> = Vec::new();
    let mut errs = Vec::new();
    for r in results { match r { Ok(m) => munis.extend(m), Err(e) => errs.push(e) } }
    if !errs.is_empty() { log_status(log, format!("✖ Municípios: {}", errs.join("; "))); }
    if munis.is_empty() { return; }

    let mut lines = Vec::new();
    let mut labels = Vec::new();
    for m in &munis {
        lines.extend(m.lines.iter().cloned());
        if let (false, Some((lat, lon))) = (m.name.is_empty(), m.center) {
            labels.push(MapLabel { lat, lon, en: m.name.clone(), pt: m.name.clone(), kind: LabelKind::City, rank: 9, big: false });
        }
    }
    log_status(log, format!("✔ Municípios: {} municípios", munis.len()));
    let _ = tx.send(Patch { layer: Layer::Cities, lines, lines_big: vec![], labels });

    let url = format!("{IBGE}/v1/localidades/regioes-metropolitanas");
    let json = match fetch_json(&url) {
        Ok(j) => j,
        Err(e) => { log_status(log, format!("✖ Regiões metropolitanas: {url}: {e}")); return; }
    };
    let by_code: HashMap<u32, &Muni> = munis.iter().map(|m| (m.code, m)).collect();
    let (mut mlines, mut mlabels) = (Vec::new(), Vec::new());
    for rm in json.as_array().into_iter().flatten() {
        let Some(name) = rm.get("nome").and_then(|n| n.as_str()) else { continue; };
        let members: Vec<&Muni> = rm.get("municipios").and_then(|v| v.as_array()).into_iter().flatten()
            .filter_map(|m| json_code(m.get("id")).and_then(|c| by_code.get(&c).copied())).collect();
        if members.is_empty() { continue; }
        mlines.extend(outer_outline(members.iter().flat_map(|m| m.lines.iter())));
        let centers: Vec<(f64, f64)> = members.iter().filter_map(|m| m.center).collect();
        if !centers.is_empty() {
            let n = centers.len() as f64;
            let (lat, lon) = (centers.iter().map(|c| c.0).sum::<f64>() / n, centers.iter().map(|c| c.1).sum::<f64>() / n);
            mlabels.push(MapLabel { lat, lon, en: name.to_owned(), pt: name.to_owned(), kind: LabelKind::Metro, rank: 0, big: false });
        }
    }
    if mlines.is_empty() {
        log_status(log, "✖ Regiões metropolitanas: resposta do IBGE sem municípios".into());
    } else {
        log_status(log, format!("✔ Regiões metropolitanas: {}", mlabels.len()));
        let _ = tx.send(Patch { layer: Layer::Metro, lines: mlines, lines_big: vec![], labels: mlabels });
    }
}

/// Keeps only the segments that belong to exactly one polygon, i.e. the outer boundary of a group
/// of neighbouring municipalities (shared inner borders appear twice and are dropped).
fn outer_outline<'a>(lines: impl Iterator<Item = &'a Line>) -> Vec<Line> {
    let q = |p: (f64, f64)| ((p.0 * 1e5).round() as i64, (p.1 * 1e5).round() as i64);
    let mut count: HashMap<((i64, i64), (i64, i64)), (u32, (f64, f64), (f64, f64))> = HashMap::new();
    for line in lines {
        for w in line.windows(2) {
            let (a, b) = (q(w[0]), q(w[1]));
            if a == b { continue; }
            let key = if a <= b { (a, b) } else { (b, a) };
            count.entry(key).and_modify(|e| e.0 += 1).or_insert((1, w[0], w[1]));
        }
    }
    count.into_values().filter(|e| e.0 == 1).map(|e| vec![e.1, e.2]).collect()
}

fn geo_cache_path(url: &str) -> PathBuf {
    let clean: String = url.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    let start = clean.len().saturating_sub(120);
    cache_dir().join("geo").join(&clean[start..])
}

/// Downloads (or reads from the disk cache) a JSON document. There is no size limit (the world
/// admin-1 file is tens of MB), but there are connect/read timeouts and one retry.
fn fetch_json(url: &str) -> Result<serde_json::Value, String> {
    let path = geo_cache_path(url);
    if let Ok(bytes) = std::fs::read(&path) {
        if let Ok(v) = serde_json::from_slice(&bytes) { return Ok(v); }
    }
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(90))
        .user_agent("EarthCraft/0.1 (map boundary overlay)")
        .build();
    let mut last = String::new();
    for _ in 0..2 {
        let resp = match agent.get(url).call() { Ok(r) => r, Err(e) => { last = e.to_string(); continue; } };
        let mut buf = Vec::new();
        if let Err(e) = resp.into_reader().take(1 << 30).read_to_end(&mut buf) { last = e.to_string(); continue; }
        match serde_json::from_slice::<serde_json::Value>(&buf) {
            Ok(v) => {
                if let Some(dir) = path.parent() { let _ = std::fs::create_dir_all(dir); }
                let _ = std::fs::write(&path, &buf);
                return Ok(v);
            }
            Err(e) => { last = format!("JSON inválido: {e}"); }
        }
    }
    Err(last)
}

fn feature_name(feature: &serde_json::Value, keys: &[&str]) -> Option<String> {
    let props = feature.get("properties")?;
    keys.iter().find_map(|key| props.get(*key).and_then(|v| v.as_str()).filter(|s| !s.trim().is_empty()).map(|s| s.to_owned()))
}

/// Label anchor: bounding-box centre of the geometry's largest outer ring (so countries with
/// far-away islands are labelled on the mainland). Returns (lat, lon).
fn label_center(geometry: &serde_json::Value) -> Option<(f64, f64)> {
    let coords = geometry.get("coordinates")?;
    let rings: Vec<&serde_json::Value> = match geometry.get("type").and_then(|t| t.as_str())? {
        "Polygon" => coords.as_array()?.first().into_iter().collect(),
        "MultiPolygon" => coords.as_array()?.iter().filter_map(|p| p.as_array()?.first()).collect(),
        _ => return None,
    };
    let ring = rings.into_iter().max_by_key(|r| r.as_array().map_or(0, |a| a.len()))?;
    let (mut min_lat, mut max_lat, mut min_lon, mut max_lon) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for p in ring.as_array()? {
        let (Some(lon), Some(lat)) = (p.get(0).and_then(|v| v.as_f64()), p.get(1).and_then(|v| v.as_f64())) else { continue; };
        if lat.abs() > 90.0 || lon.abs() > 180.0 { continue; }
        min_lat = min_lat.min(lat); max_lat = max_lat.max(lat);
        min_lon = min_lon.min(lon); max_lon = max_lon.max(lon);
    }
    (min_lat <= max_lat).then_some(((min_lat + max_lat) / 2.0, (min_lon + max_lon) / 2.0))
}

/// Lower-cases and strips common Portuguese/Latin accents, for name de-duplication.
fn fold_name(s: &str) -> String {
    s.chars().map(|c| c.to_lowercase().next().unwrap_or(c)).map(|c| match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' => 'a', 'é' | 'è' | 'ê' | 'ë' => 'e', 'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o', 'ú' | 'ù' | 'û' | 'ü' => 'u', 'ç' => 'c',
        c => c,
    }).filter(|c| c.is_alphanumeric()).collect()
}

/// Brazilian cities exist both in the world populated-places set and in the IBGE municipality set;
/// keep only the most prominent entry (lowest rank) per name and ~0.5° cell.
fn dedupe_city_labels(labels: &mut Vec<MapLabel>) {
    let mut best: HashMap<(String, i32, i32), usize> = HashMap::new();
    let mut drop = vec![false; labels.len()];
    for (i, l) in labels.iter().enumerate() {
        if l.kind != LabelKind::City { continue; }
        for (dla, dlo) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
            let key = (fold_name(&l.pt), (l.lat * 2.0).floor() as i32 + dla, (l.lon * 2.0).floor() as i32 + dlo);
            if let Some(&j) = best.get(&key) {
                if labels[j].rank <= l.rank { drop[i] = true; } else { drop[j] = true; }
                break;
            }
        }
        if !drop[i] { best.insert((fold_name(&l.pt), (l.lat * 2.0).floor() as i32, (l.lon * 2.0).floor() as i32), i); }
    }
    let mut it = drop.into_iter();
    labels.retain(|_| !it.next().unwrap_or(false));
}

// Recursively extracts GeoJSON coordinate sequences. GeoJSON order is [longitude, latitude].
fn collect_geojson_lines(value: &serde_json::Value, out: &mut Vec<Vec<(f64, f64)>>) {
    let Some(items) = value.as_array() else { return; };
    let is_line = items.first().and_then(|v| v.as_array())
        .map_or(false, |pair| pair.len() >= 2 && pair[0].is_number() && pair[1].is_number());
    if is_line {
        let line: Vec<(f64, f64)> = items.iter().filter_map(|pair| {
            let p = pair.as_array()?;
            let lon = p.get(0)?.as_f64()?; let lat = p.get(1)?.as_f64()?;
            (lat.is_finite() && lon.is_finite() && lat.abs() <= 90.0 && lon.abs() <= 180.0).then_some((lat, lon))
        }).collect();
        if line.len() >= 2 { out.push(line); }
    } else { for child in items { collect_geojson_lines(child, out); } }
}

struct App {
    lang: Lang,
    cam: Cam,
    fly: Option<Fly>,
    cache: Arc<TileCache>,
    provider: Provider,
    show_grid: bool,
    show_country_boundaries: bool,
    show_state_boundaries: bool,
    show_city_boundaries: bool,
    show_metro_boundaries: bool,
    show_place_labels: bool,
    boundaries: BoundaryData,
    boundaries_rx: Option<mpsc::Receiver<Patch>>,
    boundary_log: StatusLog,
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

    settings: Settings,
    saved_settings: Settings,
    applied_ui_scale: f32,
    ui_scale_edit: f32, // slider value while dragging (committed on release)
    show_settings: bool,

    wayback_list: Vec<WaybackRelease>,
    wayback_idx: usize,
    wayback_rx: Option<mpsc::Receiver<Result<Vec<WaybackRelease>, String>>>,
    wayback_started: bool,
    wayback_err: Option<String>,

    suggestions: Vec<Suggestion>,
    remote_sugg: Vec<Suggestion>,
    sugg_open: bool,
    sugg_sel: Option<usize>,
    sugg_last_edit: Instant,
    sugg_sent: String,
    sugg_rx: Option<mpsc::Receiver<(String, Result<Vec<Suggestion>, String>)>>,

    kml_drop_rect: Rect,
    ring_drag: bool,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut vis = egui::Visuals::dark();
        vis.window_rounding = 6.0.into();
        cc.egui_ctx.set_visuals(vis);
        let lang = load_lang();
        let settings = Settings::load();
        let boundary_log: StatusLog = Arc::new(Mutex::new(Vec::new()));
        App {
            lang,
            cam: Cam { lat: -15.0, lon: -50.0, heading: 0.0, alt: 14_000.0 },
            fly: None,
            cache: TileCache::new(cc.egui_ctx.clone()),
            provider: Provider::Satellite,
            show_grid: false,
            show_country_boundaries: true,
            show_state_boundaries: true,
            show_city_boundaries: false,
            show_metro_boundaries: false,
            show_place_labels: true,
            boundaries: BoundaryData::default(),
            boundaries_rx: Some(load_boundaries_async(boundary_log.clone())),
            boundary_log,
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
            status: lang.pick("Ready. Drag to rotate, scroll to zoom.", "Pronto. Arraste para girar, role para aproximar.").into(),
            placemarks: vec![],
            new_name: String::new(),
            kml_path: "placemarks.kml".into(),
            menu_pick: None,
            ruler: false,
            ruler_pts: vec![],
            cursor: None,
            show_help: false,
            ui_scale_edit: settings.ui_scale,
            settings: settings.clone(),
            saved_settings: settings,
            applied_ui_scale: 0.0,
            show_settings: false,
            wayback_list: vec![],
            wayback_idx: 0,
            wayback_rx: None,
            wayback_started: false,
            wayback_err: None,
            suggestions: vec![],
            remote_sugg: vec![],
            sugg_open: false,
            sugg_sel: None,
            sugg_last_edit: Instant::now(),
            sugg_sent: String::new(),
            sugg_rx: None,
            kml_drop_rect: Rect::NOTHING,
            ring_drag: false,
        }
    }

    fn set_lang(&mut self, l: Lang) {
        self.lang = l;
        save_lang(l);
        self.status = l.pick("Ready. Drag to rotate, scroll to zoom.", "Pronto. Arraste para girar, role para aproximar.").into();
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

    /// Rigidly rotates the camera (centre and "up" direction) by the rotation that takes `from` to `to`.
    fn rotate_cam(&mut self, from: V3, to: V3) {
        let axis = from.cross(to);
        let sn = axis.dot(axis).sqrt();
        if sn < 1e-13 {
            return;
        }
        let ang = sn.atan2(from.dot(to));
        let k = axis * (1.0 / sn);
        let rot = |v: V3| v * ang.cos() + k.cross(v) * ang.sin() + k * (k.dot(v) * (1.0 - ang.cos()));
        let n = ll_to_v3(self.cam.lat, self.cam.lon);
        let (la, lo) = (self.cam.lat.to_radians(), self.cam.lon.to_radians());
        let east = V3::new(-lo.sin(), lo.cos(), 0.0);
        let north = V3::new(-la.sin() * lo.cos(), -la.sin() * lo.sin(), la.cos());
        let h = self.cam.heading.to_radians();
        let up = north * h.cos() + east * h.sin();
        let (n2, u2) = (rot(n).norm(), rot(up));
        let (lat2, lon2) = v3_to_ll(n2);
        let (la2, lo2) = (lat2.to_radians(), lon2.to_radians());
        let east2 = V3::new(-lo2.sin(), lo2.cos(), 0.0);
        let north2 = V3::new(-la2.sin() * lo2.cos(), -la2.sin() * lo2.sin(), la2.cos());
        self.cam.lat = lat2.clamp(-89.9, 89.9);
        self.cam.lon = wrap_lon(lon2);
        self.cam.heading = wrap_lon(u2.dot(east2).atan2(u2.dot(north2)).to_degrees());
    }

    /// Zooms keeping the geographic point under `cursor` fixed on screen (like Google Earth).
    fn zoom_at(&mut self, factor: f64, cursor: Option<Pos2>, rect: Rect, aspect: f64) {
        let before = cursor.and_then(|c| self.cam.basis(aspect).pick(rect, c));
        self.zoom(factor);
        if let (Some(p0), Some(c)) = (before, cursor) {
            if let Some(p1) = self.cam.basis(aspect).pick(rect, c) {
                self.rotate_cam(ll_to_v3(p1.0, p1.1), ll_to_v3(p0.0, p0.1));
            }
        }
    }

    // ───────── Historical imagery (Wayback) ─────────

    fn ensure_wayback(&mut self, ctx: &egui::Context) {
        if self.provider != Provider::Wayback || self.wayback_started {
            return;
        }
        self.wayback_started = true;
        self.wayback_err = None;
        let (tx, rx) = mpsc::channel();
        self.wayback_rx = Some(rx);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(load_wayback_releases());
            ctx.request_repaint();
        });
    }

    fn poll_wayback(&mut self) {
        let Some(rx) = &self.wayback_rx else { return };
        match rx.try_recv() {
            Ok(Ok(list)) => {
                self.wayback_rx = None;
                if let Ok(mut g) = WAYBACK.lock() {
                    *g = list.clone();
                }
                // default: the latest release that is at least ~5 years old, so the difference is visible
                let year = 1970 + (now_unix() / 31_556_952.0) as i32;
                let limit = format!("{}-12-31", year - 5);
                self.wayback_idx = list.iter().position(|r| r.date <= limit).unwrap_or(0);
                self.wayback_list = list;
                self.last_state = 0;
                self.last_gen = u64::MAX;
            }
            Ok(Err(e)) => {
                self.wayback_rx = None;
                self.wayback_started = false;
                self.wayback_err = Some(e);
                self.provider = Provider::Satellite;
            }
            Err(mpsc::TryRecvError::Disconnected) => self.wayback_rx = None,
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }

    fn set_wayback(&mut self, idx: usize) {
        if idx < self.wayback_list.len() && idx != self.wayback_idx {
            self.wayback_idx = idx;
            self.last_state = 0;
        }
    }

    fn effective_provider(&self) -> Provider {
        if self.provider == Provider::Wayback && self.wayback_list.is_empty() {
            Provider::Satellite // until the list of dates is loaded
        } else {
            self.provider
        }
    }

    // ───────── Search suggestions ─────────

    fn on_query_changed(&mut self) {
        let q = self.query.trim().to_string();
        self.sugg_sel = None;
        self.sugg_last_edit = Instant::now();
        if q.is_empty() || parse_latlon(&q).is_some() {
            self.sugg_open = false;
            self.suggestions.clear();
            self.remote_sugg.clear();
            return;
        }
        self.sugg_open = true;
        let fq = fold_name(&q);
        self.remote_sugg.retain(|s| fold_name(&s.label).contains(&fq));
        self.rebuild_suggestions();
    }

    /// Instant local matches (countries, states, cities already loaded) + remote results.
    fn rebuild_suggestions(&mut self) {
        let fq = fold_name(self.query.trim());
        let mut out: Vec<Suggestion> = Vec::new();
        if fq.chars().count() >= 2 {
            let mut hits: Vec<(u8, u8, usize, &MapLabel)> = Vec::new();
            for l in &self.boundaries.labels {
                let name = fold_name(l.text(self.lang));
                let kind_rank = match l.kind { LabelKind::Country => 0, LabelKind::State => 1, LabelKind::City => 2, LabelKind::Metro => 3 };
                if name.starts_with(&fq) {
                    hits.push((kind_rank, l.rank, name.len(), l));
                }
            }
            hits.sort_by(|a, b| (a.1.min(3), a.0, a.2).cmp(&(b.1.min(3), b.0, b.2)));
            for (_, _, _, l) in hits {
                if out.len() >= 4 {
                    break;
                }
                let label = l.text(self.lang).to_string();
                if out.iter().any(|o| fold_name(&o.label) == fold_name(&label)) {
                    continue;
                }
                let alt = match l.kind {
                    LabelKind::Country => 6000.0,
                    LabelKind::State => if l.big { 3000.0 } else { 1200.0 },
                    LabelKind::Metro => 150.0,
                    LabelKind::City => 40.0,
                };
                out.push(Suggestion { label, lat: l.lat, lon: l.lon, alt });
            }
        }
        for r in &self.remote_sugg {
            if out.len() >= 8 {
                break;
            }
            if !out.iter().any(|o| fold_name(&o.label) == fold_name(&r.label)) {
                out.push(r.clone());
            }
        }
        self.suggestions = out;
        if self.sugg_sel.map_or(false, |i| i >= self.suggestions.len()) {
            self.sugg_sel = None;
        }
    }

    fn poll_suggestions(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.sugg_rx {
            if let Ok((q, res)) = rx.try_recv() {
                self.sugg_rx = None;
                if let Ok(v) = res {
                    if q == self.query.trim() {
                        self.remote_sugg = v;
                        self.rebuild_suggestions();
                    }
                }
                ctx.request_repaint();
            }
        }
        let q = self.query.trim().to_string();
        if self.sugg_open && q.chars().count() >= 2 && q != self.sugg_sent && self.sugg_rx.is_none() {
            if self.sugg_last_edit.elapsed() >= Duration::from_millis(280) {
                self.sugg_sent = q.clone();
                let (tx, rx) = mpsc::channel();
                self.sugg_rx = Some(rx);
                let (ctx2, lang) = (ctx.clone(), self.lang.code());
                std::thread::spawn(move || {
                    let r = photon_suggest(&q, lang);
                    let _ = tx.send((q, r));
                    ctx2.request_repaint();
                });
            } else {
                ctx.request_repaint_after(Duration::from_millis(90));
            }
        }
    }

    fn choose_suggestion(&mut self, i: usize) {
        if let Some(s) = self.suggestions.get(i).cloned() {
            self.query = s.label.clone();
            self.sugg_sent = s.label.clone();
            self.sugg_open = false;
            self.sugg_sel = None;
            self.results.clear();
            self.fly_to(s.lat, s.lon, s.alt);
            self.status = format!("{} {}", self.lang.pick("Going to", "Indo para"), s.label);
        }
    }

    // ───────── Files: KML, screenshots, folders ─────────

    fn resolve_kml_path(&self, p: &str) -> PathBuf {
        let path = PathBuf::from(p.trim());
        if path.is_absolute() { path } else { PathBuf::from(&self.settings.kml_dir).join(path) }
    }

    fn save_kml_to(&mut self, path: PathBuf) {
        let lg = self.lang;
        if let Some(d) = path.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        self.status = match save_kml(&path.to_string_lossy(), &self.placemarks) {
            Ok(_) => {
                self.kml_path = path.to_string_lossy().into_owned();
                format!("{} {}", lg.pick("Saved to", "Salvo em"), path.display())
            }
            Err(e) => format!("{} {e}", lg.pick("Error saving:", "Erro ao salvar:")),
        };
    }

    fn load_kml_from(&mut self, path: &std::path::Path) {
        let lg = self.lang;
        match load_kml(&path.to_string_lossy(), lg) {
            Ok(v) => {
                self.status = format!("{} {} — {}", v.len(), lg.pick("placemark(s) imported.", "local(is) importado(s)."), path.display());
                self.kml_path = path.to_string_lossy().into_owned();
                if let Some(first) = v.first() {
                    let (la, lo) = (first.lat, first.lon);
                    self.fly_to(la, lo, if v.len() == 1 { 3.0 } else { 2000.0 });
                }
                self.placemarks.extend(v);
            }
            Err(e) => self.status = format!("{} {e}", lg.pick("Error opening:", "Erro ao abrir:")),
        }
    }

    fn pick_kml_open(&mut self) {
        let dir = PathBuf::from(&self.settings.kml_dir);
        let mut dlg = rfd::FileDialog::new().add_filter("KML", &["kml", "KML"]).set_title(self.lang.pick("Open KML", "Abrir KML"));
        if dir.is_dir() {
            dlg = dlg.set_directory(&dir);
        }
        if let Some(path) = dlg.pick_file() {
            self.load_kml_from(&path);
        }
    }

    fn pick_kml_save_as(&mut self) {
        let dir = PathBuf::from(&self.settings.kml_dir);
        let name = std::path::Path::new(&self.kml_path).file_name().and_then(|n| n.to_str()).unwrap_or("placemarks.kml").to_string();
        let mut dlg = rfd::FileDialog::new().add_filter("KML", &["kml"]).set_file_name(name).set_title(self.lang.pick("Save KML as", "Salvar KML como"));
        if dir.is_dir() {
            dlg = dlg.set_directory(&dir);
        }
        if let Some(mut path) = dlg.save_file() {
            if path.extension().is_none() {
                path.set_extension("kml");
            }
            self.save_kml_to(path);
        }
    }

    fn save_screenshot(&mut self) {
        let lg = self.lang;
        self.status = match &self.image {
            Some((w, h, buf)) => {
                let dir = PathBuf::from(&self.settings.screenshot_dir);
                let _ = std::fs::create_dir_all(&dir);
                let path = dir.join(format!("earthcraft_{}.png", now_unix() as u64));
                match image::save_buffer(&path, buf, *w as u32, *h as u32, image::ColorType::Rgba8) {
                    Ok(_) => format!("{} {}", lg.pick("Screenshot saved:", "Captura salva:"), path.display()),
                    Err(e) => format!("{} {e}", lg.pick("Error:", "Erro:")),
                }
            }
            None => lg.pick("Nothing to save yet.", "Nada para salvar ainda.").into(),
        };
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped: Vec<egui::DroppedFile> = ctx.input(|i| i.raw.dropped_files.clone());
        for f in dropped {
            let Some(path) = f.path else { continue };
            let is_kml = path.extension().and_then(|e| e.to_str()).map_or(false, |e| e.eq_ignore_ascii_case("kml"));
            if is_kml {
                self.load_kml_from(&path);
            } else {
                self.status = self.lang.pick("Only .kml files can be dropped here.", "Só é possível soltar arquivos .kml aqui.").into();
            }
        }
    }

    fn poll_search(&mut self) {
        let lg = self.lang;
        let Some(rx) = &self.search_rx else { return };
        if let Ok(res) = rx.try_recv() {
            self.search_rx = None;
            match res {
                Ok(v) if v.is_empty() => self.status = lg.pick("No results found.", "Nenhum resultado encontrado.").into(),
                Ok(v) => {
                    self.status = format!("{} {}", v.len(), lg.pick("result(s).", "resultado(s)."));
                    let first = v[0].clone();
                    self.results = v;
                    self.fly_to(first.lat, first.lon, first.alt);
                }
                Err(e) => self.status = format!("{} {e}", lg.pick("Search error:", "Erro na busca:")),
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
            self.status = format!("{} {lat:.5}, {lon:.5}", self.lang.pick("Going to", "Indo para"));
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.search_rx = Some(rx);
        self.status = self.lang.pick("Searching…", "Buscando…").into();
        let ctx = ctx.clone();
        let lang_code = self.lang.code();
        std::thread::spawn(move || {
            let _ = tx.send(geocode(&q, lang_code));
            ctx.request_repaint();
        });
    }

    // ───────── Panels ─────────

    fn top_bar(&mut self, ctx: &egui::Context) {
        let lg = self.lang;
        let mut field_rect = Rect::NOTHING;
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.strong("🌍 EarthCraft");
                ui.separator();

                // Keys for the suggestion list must be consumed before the TextEdit sees them.
                let popup = self.sugg_open && !self.suggestions.is_empty();
                let (mut down, mut up, mut esc) = (false, false, false);
                if popup {
                    ctx.input_mut(|i| {
                        down = i.consume_key(egui::Modifiers::NONE, Key::ArrowDown);
                        up = i.consume_key(egui::Modifiers::NONE, Key::ArrowUp);
                        esc = i.consume_key(egui::Modifiers::NONE, Key::Escape);
                    });
                    let n = self.suggestions.len();
                    if down {
                        self.sugg_sel = Some(self.sugg_sel.map_or(0, |i| (i + 1) % n));
                    }
                    if up {
                        self.sugg_sel = Some(self.sugg_sel.map_or(n - 1, |i| (i + n - 1) % n));
                    }
                    if esc {
                        self.sugg_open = false;
                    }
                }

                let r = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .hint_text(lg.pick("Search a place, address or “lat, lon”", "Buscar lugar, endereço ou “lat, lon”"))
                        .desired_width(340.0 * self.settings.ui_scale.max(1.0)),
                );
                field_rect = r.rect;
                if r.changed() {
                    self.on_query_changed();
                }
                if r.gained_focus() && !self.query.trim().is_empty() && !self.suggestions.is_empty() {
                    self.sugg_open = true;
                }
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                if enter {
                    match (self.sugg_open, self.sugg_sel) {
                        (true, Some(i)) => self.choose_suggestion(i),
                        _ => {
                            self.sugg_open = false;
                            self.start_search(ctx);
                        }
                    }
                }
                if ui.button(lg.pick("🔍 Search", "🔍 Buscar")).clicked() {
                    self.sugg_open = false;
                    self.start_search(ctx);
                }
                ui.separator();
                ui.label(&self.status);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("⚙").on_hover_text(lg.pick("Settings", "Configurações")).clicked() {
                        self.show_settings = !self.show_settings;
                    }
                    let mut chosen = self.lang;
                    egui::ComboBox::from_id_source("lang_select")
                        .selected_text(format!("🌐 {}", chosen.label()))
                        .show_ui(ui, |ui| {
                            for l in Lang::ALL {
                                ui.selectable_value(&mut chosen, l, l.label());
                            }
                        });
                    if chosen != self.lang {
                        self.set_lang(chosen);
                    }
                });
            });
        });
        self.suggestion_popup(ctx, field_rect);
    }

    /// Google-style dropdown with suggestions under the search field.
    fn suggestion_popup(&mut self, ctx: &egui::Context, field: Rect) {
        if !self.sugg_open || self.suggestions.is_empty() || field == Rect::NOTHING {
            return;
        }
        let mut picked: Option<usize> = None;
        let q = self.query.trim().to_lowercase();
        let ui_scale = self.settings.ui_scale;
        let width = field.width().max(320.0);
        let area = egui::Area::new(egui::Id::new("search_suggestions"))
            .order(egui::Order::Foreground)
            .fixed_pos(field.left_bottom() + Vec2::new(0.0, 3.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).rounding(14.0).inner_margin(egui::Margin::symmetric(4.0, 6.0)).show(ui, |ui| {
                    ui.set_width(width);
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for (i, sg) in self.suggestions.iter().enumerate() {
                        let row_h = 28.0 * ui_scale;
                        let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, row_h), Sense::click());
                        if resp.hovered() {
                            self.sugg_sel = Some(i);
                        }
                        if self.sugg_sel == Some(i) {
                            ui.painter().rect_filled(rect, 8.0, ui.visuals().widgets.hovered.bg_fill);
                        }
                        ui.painter().text(
                            Pos2::new(rect.left() + 14.0, rect.center().y),
                            Align2::LEFT_CENTER,
                            "🔍",
                            FontId::proportional(13.0 * ui_scale),
                            Color32::from_gray(150),
                        );
                        // Typed part in normal weight, completion in bold (like Google).
                        let font = FontId::proportional(14.0 * ui_scale);
                        let (normal, strong) = (ui.visuals().text_color(), ui.visuals().strong_text_color());
                        let mut job = egui::text::LayoutJob::default();
                        job.wrap.max_width = width - 48.0;
                        job.wrap.max_rows = 1;
                        job.wrap.break_anywhere = true;
                        let lower = sg.label.to_lowercase();
                        let split = if !q.is_empty() && lower.starts_with(&q) && sg.label.is_char_boundary(q.len()) { q.len() } else { 0 };
                        if split > 0 {
                            job.append(&sg.label[..split], 0.0, egui::TextFormat::simple(font.clone(), normal));
                        }
                        job.append(&sg.label[split..], 0.0, egui::TextFormat::simple(font, if split > 0 { strong } else { normal }));
                        let galley = ui.fonts(|f| f.layout_job(job));
                        let pos = Pos2::new(rect.left() + 40.0, rect.center().y - galley.size().y / 2.0);
                        ui.painter().galley(pos, galley, normal);
                        if resp.clicked() {
                            picked = Some(i);
                        }
                    }
                });
            });
        let popup_rect = area.response.rect;
        if let Some(i) = picked {
            self.choose_suggestion(i);
            return;
        }
        // click anywhere else closes the list
        let clicked_away = ctx.input(|i| {
            i.pointer.any_pressed() && i.pointer.interact_pos().map_or(false, |p| !field.contains(p) && !popup_rect.contains(p))
        });
        if clicked_away {
            self.sugg_open = false;
        }
    }

    fn settings_window(&mut self, ctx: &egui::Context) {
        let lg = self.lang;
        let mut open = self.show_settings;
        egui::Window::new(lg.pick("⚙ Settings", "⚙ Configurações")).open(&mut open).resizable(false).default_width(420.0).show(ctx, |ui| {
            ui.strong(lg.pick("Text size", "Tamanho do texto"));
            egui::Grid::new("settings_text").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label(lg.pick("Buttons and panels", "Botões e painéis"));
                // The window auto-sizes to its (scaled) content, so changing the scale while dragging
                // moves the slider under the cursor and feeds back into itself (shaking).
                // Fix: only commit the new scale when the drag ends.
                let r = ui.add(egui::Slider::new(&mut self.ui_scale_edit, 0.7..=2.0).step_by(0.05).custom_formatter(|v, _| format!("{:.0}%", v * 100.0)));
                if r.drag_stopped() || (r.changed() && !r.dragged()) {
                    self.settings.ui_scale = self.ui_scale_edit;
                }
                ui.end_row();
                ui.label(lg.pick("Map text (names, ruler, scale)", "Texto do mapa (nomes, régua, escala)"));
                ui.add(egui::Slider::new(&mut self.settings.map_scale, 0.6..=3.0).step_by(0.05).custom_formatter(|v, _| format!("{:.0}%", v * 100.0)));
                ui.end_row();
            });
            ui.separator();
            ui.strong(lg.pick("Where to save files", "Onde salvar os arquivos"));
            let folder_row = |ui: &mut egui::Ui, label: &str, value: &mut String, title: &str| {
                ui.label(label);
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(value).desired_width(300.0));
                    if ui.button(lg.pick("📁 Browse…", "📁 Procurar…")).clicked() {
                        let mut dlg = rfd::FileDialog::new().set_title(title);
                        if std::path::Path::new(value.as_str()).is_dir() {
                            dlg = dlg.set_directory(value.as_str());
                        }
                        if let Some(dir) = dlg.pick_folder() {
                            *value = dir.to_string_lossy().into_owned();
                        }
                    }
                });
            };
            folder_row(ui, lg.pick("Screenshots (PNG):", "Capturas de tela (PNG):"), &mut self.settings.screenshot_dir, lg.pick("Screenshots folder", "Pasta das capturas"));
            folder_row(ui, lg.pick("KML files:", "Arquivos KML:"), &mut self.settings.kml_dir, lg.pick("KML folder", "Pasta dos KML"));
            ui.weak(lg.pick("Folders are created automatically when saving.", "As pastas são criadas automaticamente ao salvar."));
            ui.separator();
            if ui.button(lg.pick("Restore defaults", "Restaurar padrões")).clicked() {
                self.settings = Settings::default();
            }
        });
        self.show_settings = open;
    }

    fn side_panel(&mut self, ctx: &egui::Context) {
        let lg = self.lang;
        let mut goto: Option<(f64, f64, f64)> = None;
        let mut del: Option<usize> = None;
        let (mut save_now, mut save_as, mut open_dialog, mut open_typed, mut shot) = (false, false, false, false, false);
        egui::SidePanel::left("side").default_width(270.0).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::CollapsingHeader::new(lg.pick("Layers", "Camadas")).default_open(true).show(ui, |ui| {
                    for p in Provider::ALL {
                        ui.radio_value(&mut self.provider, p, p.name(lg));
                    }
                    if self.provider == Provider::Wayback {
                        ui.indent("wayback_ui", |ui| {
                            if self.wayback_list.is_empty() {
                                ui.weak(lg.pick("⏳ Loading available dates…", "⏳ Carregando datas disponíveis…"));
                            } else {
                                let n = self.wayback_list.len();
                                let mut idx = self.wayback_idx;
                                ui.horizontal(|ui| {
                                    if ui.small_button("◀").on_hover_text(lg.pick("Older", "Mais antiga")).clicked() && idx + 1 < n {
                                        idx += 1;
                                    }
                                    egui::ComboBox::from_id_source("wayback_date")
                                        .selected_text(self.wayback_list[idx].date.clone())
                                        .height(320.0)
                                        .show_ui(ui, |ui| {
                                            for (i, r) in self.wayback_list.iter().enumerate() {
                                                ui.selectable_value(&mut idx, i, &r.date);
                                            }
                                        });
                                    if ui.small_button("▶").on_hover_text(lg.pick("Newer", "Mais recente")).clicked() && idx > 0 {
                                        idx -= 1;
                                    }
                                });
                                ui.weak(format!("{} {}", n, lg.pick("snapshots available", "versões disponíveis")));
                                ui.weak(lg.pick(
                                    "Not every place has a snapshot on every date; missing areas fall back to the nearest available imagery.",
                                    "Nem todo lugar tem imagem em todas as datas; áreas sem imagem mostram a versão mais próxima disponível.",
                                ));
                                self.set_wayback(idx);
                            }
                            if let Some(e) = &self.wayback_err {
                                ui.colored_label(Color32::from_rgb(255, 120, 120), format!("✖ {e}"));
                            }
                        });
                    }
                    ui.separator();
                    ui.checkbox(&mut self.show_grid, lg.pick("Latitude/longitude grid", "Grade de latitude/longitude"));
                    ui.checkbox(&mut self.show_country_boundaries, lg.pick("Country borders (worldwide)", "Limites dos países (mundo todo)"));
                    ui.checkbox(&mut self.show_state_boundaries, lg.pick("States / provinces (worldwide)", "Estados / províncias (mundo todo)"));
                    ui.checkbox(&mut self.show_place_labels, lg.pick("Country, state and city names", "Nomes de países, estados e cidades"));
                    ui.checkbox(&mut self.show_city_boundaries, lg.pick("City / municipality borders (Brazil dataset)", "Limites de cidades / municípios (base do Brasil)"));
                    ui.checkbox(&mut self.show_metro_boundaries, lg.pick("Metropolitan regions (Brazil only)", "Regiões metropolitanas (disponível apenas para o Brasil)"));
                    if self.boundaries_rx.is_some() {
                        ui.weak(lg.pick("⏳ Loading borders and names…", "⏳ Carregando limites e nomes…"));
                    }
                    if let Ok(log) = self.boundary_log.lock() {
                        for line in log.iter() { ui.weak(egui::RichText::new(line).small()); }
                    }
                    ui.checkbox(&mut self.show_atmo, lg.pick("Atmosphere and stars", "Atmosfera e estrelas"));
                    ui.checkbox(&mut self.show_sun, lg.pick("Real-time sunlight (day/night)", "Luz do sol em tempo real (dia/noite)"));
                    ui.checkbox(&mut self.auto_rotate, lg.pick("Auto-rotate", "Girar automaticamente"));
                });

                egui::CollapsingHeader::new(lg.pick("Navigation", "Navegação")).default_open(true).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button(lg.pick("🧭 North up", "🧭 Norte para cima")).clicked() {
                            self.cam.heading = 0.0;
                        }
                        if ui.button(lg.pick("🌐 Global view", "🌐 Visão global")).clicked() {
                            self.fly_to(self.cam.lat, self.cam.lon, 14_000.0);
                        }
                    });
                    if !self.results.is_empty() {
                        ui.separator();
                        ui.label(lg.pick("Search results:", "Resultados da busca:"));
                        for r in &self.results {
                            let short: String = r.name.chars().take(48).collect();
                            if ui.button(short).on_hover_text(&r.name).clicked() {
                                goto = Some((r.lat, r.lon, r.alt));
                            }
                        }
                    }
                });

                egui::CollapsingHeader::new(lg.pick("Placemarks", "Locais (Placemarks)")).default_open(true).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.new_name).hint_text(lg.pick("Name", "Nome")).desired_width(120.0));
                        if ui.button(lg.pick("➕ At center", "➕ No centro")).clicked() {
                            let name =
                                if self.new_name.trim().is_empty() { format!("{} {}", lg.pick("Place", "Local"), self.placemarks.len() + 1) } else { self.new_name.trim().to_string() };
                            self.placemarks.push(Placemark { name, desc: String::new(), lat: self.cam.lat, lon: self.cam.lon });
                            self.new_name.clear();
                        }
                    });
                    for (i, p) in self.placemarks.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut p.name).desired_width(110.0));
                            if ui.small_button(lg.pick("Go", "Ir")).on_hover_text(format!("{:.5}, {:.5}", p.lat, p.lon)).clicked() {
                                goto = Some((p.lat, p.lon, 3.0));
                            }
                            if ui.small_button("🗑").clicked() {
                                del = Some(i);
                            }
                        });
                    }
                    ui.separator();
                    // Drop zone: drag a .kml file from the file manager onto this box.
                    let hovering_file = ctx.input(|i| !i.raw.hovered_files.is_empty());
                    let (stroke_col, fill) = if hovering_file {
                        (Color32::from_rgb(90, 170, 255), Color32::from_rgba_unmultiplied(60, 120, 220, 45))
                    } else {
                        (Color32::from_gray(90), Color32::from_black_alpha(40))
                    };
                    let zone = egui::Frame::none()
                        .fill(fill)
                        .stroke(Stroke::new(if hovering_file { 2.0 } else { 1.0 }, stroke_col))
                        .rounding(6.0)
                        .inner_margin(egui::Margin::same(8.0))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.vertical_centered(|ui| {
                                ui.label(lg.pick("📥 Drag a .kml file here", "📥 Arraste um arquivo .kml aqui"));
                            });
                            let r = ui.add(egui::TextEdit::singleline(&mut self.kml_path).desired_width(f32::INFINITY));
                            if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                                open_typed = true;
                            }
                        });
                    self.kml_drop_rect = zone.response.rect;
                    ui.horizontal_wrapped(|ui| {
                        if ui.button(lg.pick("💾 Save", "💾 Salvar")).on_hover_text(lg.pick("Saves into the KML folder (see Settings)", "Salva na pasta de KML (veja Configurações)")).clicked() {
                            save_now = true;
                        }
                        if ui.button(lg.pick("💾 Save as…", "💾 Salvar como…")).clicked() {
                            save_as = true;
                        }
                        if ui.button(lg.pick("📂 Open KML…", "📂 Abrir KML…")).clicked() {
                            open_dialog = true;
                        }
                    });
                    ui.weak(format!("{} {}", lg.pick("Folder:", "Pasta:"), self.settings.kml_dir));
                });

                egui::CollapsingHeader::new(lg.pick("Ruler", "Régua")).default_open(true).show(ui, |ui| {
                    ui.checkbox(&mut self.ruler, lg.pick("Measure distances (click on the globe)", "Medir distâncias (clique no globo)"));
                    let total: f64 = self.ruler_pts.windows(2).map(|w| haversine_km(w[0], w[1])).sum();
                    ui.label(format!("{}: {}   Total: {}", lg.pick("Points", "Pontos"), self.ruler_pts.len(), fmt_dist(total)));
                    if ui.button(lg.pick("Clear ruler", "Limpar régua")).clicked() {
                        self.ruler_pts.clear();
                    }
                });

                egui::CollapsingHeader::new(lg.pick("Export", "Exportar")).default_open(false).show(ui, |ui| {
                    if ui.button(lg.pick("📷 Save screenshot (PNG)", "📷 Salvar captura (PNG)")).clicked() {
                        shot = true;
                    }
                    ui.weak(format!("{} {}", lg.pick("Folder:", "Pasta:"), self.settings.screenshot_dir));
                    if ui.button(lg.pick("⚙ Change folders…", "⚙ Mudar pastas…")).clicked() {
                        self.show_settings = true;
                    }
                });

                ui.checkbox(&mut self.show_help, lg.pick("Show controls help", "Mostrar ajuda de controles"));
                if self.show_help {
                    ui.label(lg.pick(
                        "• Drag (left): rotate the globe\n\
                         • Right-drag / Shift+drag: rotate the view\n\
                         • Wheel / pinch or +/-: zoom\n\
                         • Double-click: fly to the point\n\
                         • Arrow keys: move   • N: north up\n\
                         • Right-click: context menu",
                        "• Arrastar (esq.): girar o globo\n\
                         • Botão direito/Shift+arrastar: girar a vista\n\
                         • Roda/pinça ou +/-: zoom\n\
                         • Duplo clique: voar até o ponto\n\
                         • Setas: mover   • N: norte para cima\n\
                         • Botão direito (clique): menu de contexto",
                    ));
                }
            });
        });
        if let Some(i) = del {
            self.placemarks.remove(i);
        }
        if save_now {
            let path = self.resolve_kml_path(&self.kml_path.clone());
            self.save_kml_to(path);
        }
        if save_as {
            self.pick_kml_save_as();
        }
        if open_dialog {
            self.pick_kml_open();
        }
        if open_typed {
            let path = self.resolve_kml_path(&self.kml_path.clone());
            if path.is_file() {
                self.load_kml_from(&path);
            }
        }
        if shot {
            self.save_screenshot();
        }
        if let Some((la, lo, al)) = goto {
            self.fly_to(la, lo, al);
        }
    }

    fn bottom_bar(&mut self, ctx: &egui::Context) {
        let lg = self.lang;
        egui::TopBottomPanel::bottom("bottom").show(ctx, |ui| {
            ui.horizontal(|ui| {
                match self.cursor {
                    Some((la, lo)) => ui.monospace(format!("Cursor: {la:>10.5}°, {lo:>11.5}°")),
                    None => ui.monospace("Cursor: —"),
                };
                ui.separator();
                ui.monospace(format!("{}: {:.4}°, {:.4}°", lg.pick("Center", "Centro"), self.cam.lat, self.cam.lon));
                ui.separator();
                ui.monospace(format!("{}: {}", lg.pick("Camera altitude", "Altitude da câmera"), fmt_alt(self.cam.alt)));
                ui.separator();
                let n = self.cache.inflight.load(Ordering::Relaxed);
                if n > 0 {
                    ui.monospace(format!("⏳ {} {n} tiles", lg.pick("loading", "carregando")));
                } else {
                    ui.monospace("✔ tiles ok");
                }
            });
        });
    }

    // ───────── Globo ─────────

    fn globe_view(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let lg = self.lang;
        let rect = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(rect, Sense::click_and_drag());
        let ppp = ctx.pixels_per_point();
        let aspect = (rect.width() / rect.height()) as f64;
        let shift = ctx.input(|i| i.modifiers.shift);

        // ── input
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
            let cur = resp.hover_pos();
            if sc != 0.0 {
                self.fly = None;
                self.zoom_at((-sc * 0.0025).exp(), cur, rect, aspect);
                interacting = true;
            }
            if (zd - 1.0).abs() > 1e-4 {
                self.fly = None;
                self.zoom_at(1.0 / zd, cur, rect, aspect);
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

        // ── rendering
        let idle = self.last_interact.elapsed() > Duration::from_millis(250) && !resp.dragged();
        let scale = if idle { 1.0 } else { 0.4 };
        let w = ((rect.width() * ppp * scale) as usize).clamp(64, 2000);
        let h = ((w as f32 * rect.height() / rect.width()) as usize).max(32);
        let sun_key = if self.show_sun { (now_unix() / 60.0) as u64 } else { 0 };

        let mut hs = std::collections::hash_map::DefaultHasher::new();
        (
            (
                self.cam.lat.to_bits(),
                self.cam.lon.to_bits(),
                self.cam.heading.to_bits(),
                self.cam.alt.to_bits(),
                w,
                h,
                self.effective_provider().id(),
                self.show_grid,
            ),
            (
                self.show_country_boundaries,
                self.show_state_boundaries,
                self.show_city_boundaries,
                self.show_metro_boundaries,
                self.show_place_labels,
                self.show_atmo,
                sun_key,
                self.wayback_idx,
            ),
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

        // ── drawing
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::BLACK);
        if let Some(tex) = &self.texture {
            painter.image(tex.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        }
        self.draw_overlays(&painter, &basis, rect);

        // Google Earth style navigation controls (rotate / move / zoom)
        let dt_nav = ctx.input(|i| i.stable_dt).min(0.1) as f64;
        if self.nav_controls(ui, rect, dt_nav) {
            self.touch();
            ctx.request_repaint();
        }

        // scale bar
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
            painter.text(Pos2::new(x0, y - 6.0), Align2::LEFT_BOTTOM, label, FontId::proportional(12.0 * self.settings.map_scale), Color32::WHITE);
        }
        let mut attribution = self.provider.attribution(lg).to_string();
        if self.provider == Provider::Wayback {
            if let Some(r) = self.wayback_list.get(self.wayback_idx) {
                attribution = format!("{} — {} {}", attribution, lg.pick("snapshot", "imagem de"), r.date);
            }
        }
        painter.text(
            rect.right_bottom() - Vec2::new(8.0, 6.0),
            Align2::RIGHT_BOTTOM,
            attribution,
            FontId::proportional(11.0 * self.settings.map_scale),
            Color32::from_white_alpha(200),
        );

        // context menu (right-click)
        let pick = self.menu_pick;
        let mut act: Option<u8> = None;
        resp.context_menu(|ui| match pick {
            Some((la, lo)) => {
                ui.label(format!("{la:.5}, {lo:.5}"));
                ui.separator();
                if ui.button(lg.pick("📍 Add placemark here", "📍 Adicionar local aqui")).clicked() {
                    act = Some(0);
                    ui.close_menu();
                }
                if ui.button(lg.pick("✈ Fly here", "✈ Voar até aqui")).clicked() {
                    act = Some(1);
                    ui.close_menu();
                }
                if ui.button(lg.pick("📏 Add ruler point", "📏 Adicionar ponto na régua")).clicked() {
                    act = Some(2);
                    ui.close_menu();
                }
                if ui.button(lg.pick("📋 Copy coordinates", "📋 Copiar coordenadas")).clicked() {
                    act = Some(3);
                    ui.close_menu();
                }
            }
            None => {
                ui.label(lg.pick("Click on the globe.", "Clique sobre o globo."));
            }
        });
        if let (Some(a), Some((la, lo))) = (act, pick) {
            match a {
                0 => self.placemarks.push(Placemark {
                    name: format!("{} {}", lg.pick("Place", "Local"), self.placemarks.len() + 1),
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

        // auto-rotate (keeps repainting)
        if self.auto_rotate && self.fly.is_none() {
            let dt = ctx.input(|i| i.unstable_dt).min(0.1) as f64;
            self.cam.lon = wrap_lon(self.cam.lon + 6.0 * dt);
            self.touch();
            ctx.request_repaint();
        }
    }

    /// Google Earth style control cluster: rotation ring, move pad and zoom slider.
    /// Returns true while the user is operating a control.
    fn nav_controls(&mut self, ui: &mut egui::Ui, rect: Rect, dt: f64) -> bool {
        let lg = self.lang;
        let k = self.settings.ui_scale.clamp(0.8, 1.8);
        let painter = ui.painter_at(rect);
        let base = ui.id().with("nav");
        let mut active = false;

        let fill = Color32::from_rgba_unmultiplied(40, 70, 140, 215);
        let fill_hot = Color32::from_rgba_unmultiplied(75, 115, 205, 235);
        let edge = Color32::from_white_alpha(200);
        let arrow_col = Color32::from_rgb(205, 222, 255);

        let cx = rect.right() - 56.0 * k;
        let c1 = Pos2::new(cx, rect.top() + 52.0 * k);
        let (r_ring, r_disc) = (40.0 * k, 27.0 * k);
        let c2 = Pos2::new(cx, c1.y + r_ring + 12.0 * k + r_disc);
        let dirs = [Vec2::new(0.0, -1.0), Vec2::new(0.0, 1.0), Vec2::new(-1.0, 0.0), Vec2::new(1.0, 0.0)]; // up, down, left, right

        let tri = |center: Pos2, d: Vec2, size: f32, col: Color32| {
            let n = Vec2::new(-d.y, d.x);
            painter.add(egui::Shape::convex_polygon(
                vec![center + d * size, center - d * size * 0.6 + n * size * 0.9, center - d * size * 0.6 - n * size * 0.9],
                col,
                Stroke::NONE,
            ));
        };

        // ── rotation ring + "look" disc ──
        painter.circle_filled(c1, r_ring, Color32::from_black_alpha(110));
        painter.circle_stroke(c1, r_ring, Stroke::new(1.5 * k, Color32::from_white_alpha(170)));
        painter.circle_filled(c1, r_disc, fill);
        painter.circle_stroke(c1, r_disc, Stroke::new(1.0, edge));
        let hr = self.cam.heading.to_radians() as f32;
        let nd = Vec2::new(-hr.sin(), -hr.cos()); // screen direction of north
        let n_pos = c1 + nd * (r_ring - 6.5 * k);
        painter.circle_filled(n_pos, 8.0 * k, Color32::from_rgb(220, 50, 50));
        painter.circle_stroke(n_pos, 8.0 * k, Stroke::new(1.0, Color32::WHITE));
        painter.text(n_pos, Align2::CENTER_CENTER, "N", FontId::proportional(10.5 * k), Color32::WHITE);
        // eye icon
        let eye: Vec<Pos2> = (0..=24)
            .map(|i| {
                let t = i as f32 / 24.0 * std::f32::consts::TAU;
                c1 + Vec2::new(t.cos() * 9.5 * k, t.sin() * 5.5 * k)
            })
            .collect();
        painter.add(egui::Shape::line(eye, Stroke::new(1.6, Color32::WHITE)));
        painter.circle_filled(c1, 3.2 * k, Color32::WHITE);

        let ring = ui.interact(Rect::from_center_size(c1, Vec2::splat(2.0 * r_ring)), base.with("ring"), Sense::drag());
        if ring.drag_started() {
            self.ring_drag = ring.interact_pointer_pos().map_or(false, |p| (p - c1).length() >= r_disc - 1.0);
        }
        if ring.dragged() && self.ring_drag {
            if let Some(cur) = ring.interact_pointer_pos() {
                let prev = cur - ring.drag_delta();
                let ang = |v: Vec2| v.y.atan2(v.x);
                let mut d = ang(cur - c1) - ang(prev - c1);
                if d > std::f32::consts::PI {
                    d -= std::f32::consts::TAU;
                }
                if d < -std::f32::consts::PI {
                    d += std::f32::consts::TAU;
                }
                self.cam.heading = wrap_lon(self.cam.heading - d.to_degrees() as f64);
                self.fly = None;
                active = true;
            }
        }
        if ring.drag_stopped() {
            self.ring_drag = false;
        }
        let nb = ui.interact(Rect::from_center_size(n_pos, Vec2::splat(16.0 * k)), base.with("north"), Sense::click());
        if nb.on_hover_text(lg.pick("Click to align north up", "Clique para alinhar ao norte")).clicked() {
            self.cam.heading = 0.0;
        }
        // arrows: left/right rotate the view, up = north up, down = flip 180°
        let names = [
            lg.pick("North up", "Norte para cima"),
            lg.pick("Flip view 180°", "Girar a vista 180°"),
            lg.pick("Rotate left (hold)", "Girar para a esquerda (segure)"),
            lg.pick("Rotate right (hold)", "Girar para a direita (segure)"),
        ];
        for (i, d) in dirs.iter().enumerate() {
            let c = c1 + *d * 17.0 * k;
            let r = ui.interact(Rect::from_center_size(c, Vec2::splat(15.0 * k)), base.with(("eye", i)), Sense::click());
            let held = r.is_pointer_button_down_on();
            tri(c, *d, 4.8 * k, if held || r.hovered() { Color32::WHITE } else { arrow_col });
            let r = r.on_hover_text(names[i]);
            match i {
                0 if r.clicked() => self.cam.heading = 0.0,
                1 if r.clicked() => self.cam.heading = wrap_lon(self.cam.heading + 180.0),
                2 if held => {
                    self.cam.heading = wrap_lon(self.cam.heading + 60.0 * dt);
                    active = true;
                }
                3 if held => {
                    self.cam.heading = wrap_lon(self.cam.heading - 60.0 * dt);
                    active = true;
                }
                _ => {}
            }
        }

        // ── move (pan) disc ──
        painter.circle_filled(c2, r_disc, fill);
        painter.circle_stroke(c2, r_disc, Stroke::new(1.0, edge));
        // hand icon: palm + fingers
        painter.circle_filled(c2 + Vec2::new(0.0, 2.5 * k), 5.0 * k, Color32::WHITE);
        for fx in [-4.5f32, -1.5, 1.5, 4.5] {
            painter.line_segment([c2 + Vec2::new(fx * k, 0.0), c2 + Vec2::new(fx * k, -6.5 * k)], Stroke::new(2.2 * k, Color32::WHITE));
        }
        let center = ui.interact(Rect::from_center_size(c2, Vec2::splat(18.0 * k)), base.with("pad"), Sense::drag());
        if center.dragged() {
            let d = center.drag_delta();
            self.fly = None;
            self.pan(d.x as f64, d.y as f64, rect.height());
            active = true;
        }
        let pan_names = [lg.pick("Move north (hold)", "Mover para o norte (segure)"), lg.pick("Move south (hold)", "Mover para o sul (segure)"), lg.pick("Move west (hold)", "Mover para oeste (segure)"), lg.pick("Move east (hold)", "Mover para leste (segure)")];
        for (i, d) in dirs.iter().enumerate() {
            let c = c2 + *d * 17.0 * k;
            let r = ui.interact(Rect::from_center_size(c, Vec2::splat(15.0 * k)), base.with(("pan", i)), Sense::click());
            let held = r.is_pointer_button_down_on();
            tri(c, *d, 4.8 * k, if held || r.hovered() { Color32::WHITE } else { arrow_col });
            r.on_hover_text(pan_names[i]);
            if held {
                let sp = 350.0 * dt;
                // same convention as the arrow keys
                let (dx, dy) = match i {
                    0 => (0.0, sp),
                    1 => (0.0, -sp),
                    2 => (sp, 0.0),
                    _ => (-sp, 0.0),
                };
                self.fly = None;
                self.pan(dx, dy, rect.height());
                active = true;
            }
        }

        // ── zoom slider ──
        let y0 = c2.y + r_disc + 16.0 * k;
        let r_btn = 11.0 * k;
        let track_len = 110.0 * k;
        let (t_top, t_bot) = (y0 + r_btn + 8.0 * k, y0 + r_btn + 8.0 * k + track_len);
        let y_minus = t_bot + 8.0 * k + r_btn;
        let cplus = Pos2::new(cx, y0);
        let cminus = Pos2::new(cx, y_minus);
        let (ln_min, ln_max) = (MIN_ALT.ln(), MAX_ALT.ln());
        let t = ((self.cam.alt.ln() - ln_min) / (ln_max - ln_min)).clamp(0.0, 1.0) as f32;

        painter.line_segment([Pos2::new(cx, t_top), Pos2::new(cx, t_bot)], Stroke::new(7.0 * k, Color32::from_black_alpha(120)));
        painter.line_segment([Pos2::new(cx, t_top), Pos2::new(cx, t_bot)], Stroke::new(4.0 * k, fill));
        for (c, plus) in [(cplus, true), (cminus, false)] {
            let r = ui.interact(Rect::from_center_size(c, Vec2::splat(2.0 * r_btn)), base.with(("zoombtn", plus)), Sense::click());
            let held = r.is_pointer_button_down_on();
            painter.circle_filled(c, r_btn, if held || r.hovered() { fill_hot } else { fill });
            painter.circle_stroke(c, r_btn, Stroke::new(1.0, edge));
            let l = 5.0 * k;
            painter.line_segment([c - Vec2::new(l, 0.0), c + Vec2::new(l, 0.0)], Stroke::new(2.2 * k, Color32::WHITE));
            if plus {
                painter.line_segment([c - Vec2::new(0.0, l), c + Vec2::new(0.0, l)], Stroke::new(2.2 * k, Color32::WHITE));
            }
            r.on_hover_text(if plus { lg.pick("Zoom in (hold)", "Aproximar (segure)") } else { lg.pick("Zoom out (hold)", "Afastar (segure)") });
            if held {
                self.fly = None;
                self.zoom((if plus { -1.6 * dt } else { 1.6 * dt }).exp());
                active = true;
            }
        }
        let track = ui.interact(
            Rect::from_min_max(Pos2::new(cx - 11.0 * k, t_top - 4.0 * k), Pos2::new(cx + 11.0 * k, t_bot + 4.0 * k)),
            base.with("track"),
            Sense::click_and_drag(),
        );
        if track.dragged() || track.clicked() || track.is_pointer_button_down_on() {
            if let Some(p) = track.interact_pointer_pos() {
                let tt = ((p.y - t_top) / track_len).clamp(0.0, 1.0) as f64;
                self.cam.alt = (ln_min + tt * (ln_max - ln_min)).exp().clamp(MIN_ALT, MAX_ALT);
                self.fly = None;
                active = true;
            }
        }
        let hy = t_top + t * track_len;
        let hot = track.hovered() || track.dragged();
        painter.rect_filled(Rect::from_center_size(Pos2::new(cx, hy), Vec2::new(20.0 * k, 11.0 * k)), 4.0 * k, if hot { fill_hot } else { fill });
        painter.rect_stroke(Rect::from_center_size(Pos2::new(cx, hy), Vec2::new(20.0 * k, 11.0 * k)), 4.0 * k, Stroke::new(1.0, edge));
        painter.line_segment([Pos2::new(cx - 4.5 * k, hy), Pos2::new(cx + 4.5 * k, hy)], Stroke::new(2.0 * k, Color32::WHITE));
        track.on_hover_text(lg.pick("Drag to zoom", "Arraste para ajustar o zoom"));

        active
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
        let prov = self.effective_provider();
        let rel = if prov == Provider::Wayback { self.wayback_idx as u16 } else { 0 };
        let cfg = RenderCfg { provider: prov, rel, grid: self.show_grid, grid_step, atmo: self.show_atmo, sun };

        let mut buf = vec![0u8; w * h * 4];
        let cache = &*self.cache;
        let per_row: Vec<Vec<(TileKey, f32)>> = buf
            .par_chunks_mut(w * 4)
            .enumerate()
            .map(|(y, row)| render_row(y, w, h, &basis, &cfg, cache, row))
            .collect();

        // request missing tiles: lowest zoom first, then closest to the center
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

    fn merge_patch(&mut self, patch: Patch) {
        let b = &mut self.boundaries;
        match patch.layer {
            Layer::Countries => b.countries.extend(patch.lines),
            Layer::States => {
                b.states.extend(patch.lines);
                b.states_big.extend(patch.lines_big);
            }
            Layer::Cities => b.cities.extend(patch.lines),
            Layer::Metro => b.metro.extend(patch.lines),
        }
        b.labels.extend(patch.labels);
        dedupe_city_labels(&mut b.labels);
        self.last_state = 0; // redraw overlays immediately
    }

    fn draw_overlays(&self, painter: &egui::Painter, basis: &Basis, rect: Rect) {
        // Administrative boundaries are drawn as a separate overlay, independent of imagery.
        // Densify long geographic edges at close zoom. Many admin-1 polygons have very
        // sparse vertices; projecting only the endpoints makes their borders disappear
        // whenever an endpoint falls behind the globe's horizon. Sampling the edge keeps
        // the visible middle portions drawable, including small countries at ~500 km.
        // Far away, consecutive vertices fall on the same pixel; skipping those keeps the many
        // thousands of segments of the world borders cheap to draw.
        let min_sq: f32 = if self.cam.alt > 1500.0 { 1.4 * 1.4 } else { 0.0 };
        let draw_lines = |lines: &[Vec<(f64, f64)>], color: Color32, width: f32, max_step_deg: f64| {
            let stroke = Stroke::new(width, color);
            for line in lines {
                let mut anchor: Option<Pos2> = None;
                for pair in line.windows(2) {
                    let (lat1, lon1) = pair[0];
                    let (lat2, lon2) = pair[1];
                    let dlat = lat2 - lat1;
                    let mut dlon = lon2 - lon1;
                    if dlon > 180.0 { dlon -= 360.0; }
                    if dlon < -180.0 { dlon += 360.0; }
                    let start = basis.project(ll_to_v3(lat1, lon1), rect);
                    let end = basis.project(ll_to_v3(lat2, lon2), rect);
                    if let (Some(a), Some(b)) = (start, end) {
                        // Common case: both endpoints are visible, so no extra sampling needed.
                        let from = anchor.unwrap_or(a);
                        if (b - from).length_sq() >= min_sq {
                            painter.line_segment([from, b], stroke);
                            anchor = Some(b);
                        } else if anchor.is_none() {
                            anchor = Some(a);
                        }
                        continue;
                    }
                    anchor = None;
                    let span = dlat.abs().max(dlon.abs());
                    if span < max_step_deg { continue; }
                    let steps = ((span / max_step_deg).ceil() as usize).clamp(1, 96);
                    let mut prev = start;
                    for i in 1..=steps {
                        let t = i as f64 / steps as f64;
                        let lon = wrap_lon(lon1 + dlon * t);
                        let lat = lat1 + dlat * t;
                        let cur = basis.project(ll_to_v3(lat, lon), rect);
                        if let (Some(a), Some(b)) = (prev, cur) {
                            painter.line_segment([a, b], stroke);
                        }
                        prev = cur;
                    }
                }
            }
        };
        let close_zoom = self.cam.alt <= 500.0;
        if self.show_country_boundaries { draw_lines(&self.boundaries.countries, Color32::from_rgba_unmultiplied(255, 235, 150, 235), 1.15, if close_zoom { 0.25 } else { 2.0 }); }
        if self.show_state_boundaries {
            let step = if close_zoom { 0.12 } else if self.cam.alt <= 5000.0 { 0.5 } else { 2.0 };
            let col = Color32::from_rgba_unmultiplied(120, 220, 255, 220);
            // States of the largest countries: visible at every zoom level (whole Earth in view).
            draw_lines(&self.boundaries.states_big, col, 1.0, step);
            // States/provinces of the other countries: only from medium zoom on.
            if self.cam.alt < OTHER_STATES_ALT {
                draw_lines(&self.boundaries.states, col, 1.0, step);
            }
        }
        if self.show_city_boundaries { draw_lines(&self.boundaries.cities, Color32::from_rgba_unmultiplied(255, 160, 80, 210), 0.7, if close_zoom { 0.12 } else { 1.0 }); }
        if self.show_metro_boundaries { draw_lines(&self.boundaries.metro, Color32::from_rgba_unmultiplied(255, 100, 210, 235), 1.5, if close_zoom { 0.12 } else { 1.0 }); }
        let ms = self.settings.map_scale;
        if self.show_place_labels {
            // Collect what is visible, then place by priority (countries > states > metro > cities)
            // skipping any name that would overlap one already drawn.
            let mut cands: Vec<(u8, u8, Pos2, &MapLabel)> = Vec::new();
            for label in &self.boundaries.labels {
                let enabled = label.visible_at(self.cam.alt) && match label.kind {
                    LabelKind::Country | LabelKind::City => true,
                    LabelKind::State => self.show_state_boundaries,
                    LabelKind::Metro => self.show_metro_boundaries,
                };
                if !enabled { continue; }
                if let Some(pos) = basis.project(ll_to_v3(label.lat, label.lon), rect) {
                    if rect.contains(pos) {
                        let prio = match label.kind { LabelKind::Country => 0, LabelKind::State => 1, LabelKind::Metro => 2, LabelKind::City => 3 };
                        cands.push((prio, label.rank, pos, label));
                    }
                }
            }
            cands.sort_by_key(|c| (c.0, c.1));
            let mut placed: Vec<Rect> = Vec::new();
            for (_, _, pos, label) in cands {
                if placed.len() >= 500 { break; }
                let (color, size) = match label.kind {
                    LabelKind::Country => (Color32::WHITE, 12.0),
                    LabelKind::State => (Color32::from_rgb(180, 230, 255), 10.0),
                    LabelKind::City => (Color32::from_rgb(255, 245, 210), 9.0),
                    LabelKind::Metro => (Color32::from_rgb(255, 170, 230), 10.0),
                };
                let font = FontId::proportional(size * ms);
                let text = label.text(self.lang);
                let galley = painter.layout_no_wrap(text.to_string(), font.clone(), color);
                let area = Rect::from_center_size(pos, galley.size()).expand(2.0);
                if placed.iter().any(|q| q.intersects(area)) { continue; }
                placed.push(area);
                painter.text(pos, Align2::CENTER_CENTER, text, font.clone(), Color32::from_black_alpha(180));
                painter.text(pos + Vec2::new(0.0, -0.5), Align2::CENTER_CENTER, text, font, color);
            }
        }
        // ruler
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
                painter.text(mid + Vec2::new(1.0, 1.0), Align2::CENTER_BOTTOM, &txt, FontId::proportional(13.0 * ms), Color32::BLACK);
                painter.text(mid, Align2::CENTER_BOTTOM, &txt, FontId::proportional(13.0 * ms), Color32::WHITE);
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
                    painter.text(tp + Vec2::new(1.0, 1.0), Align2::LEFT_BOTTOM, &pm.name, FontId::proportional(13.0 * ms), Color32::BLACK);
                    painter.text(tp, Align2::LEFT_BOTTOM, &pm.name, FontId::proportional(13.0 * ms), Color32::WHITE);
                }
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let dt = ctx.input(|i| i.unstable_dt).min(0.1) as f64;
        if (self.settings.ui_scale - self.applied_ui_scale).abs() > 1e-4 {
            apply_ui_scale(ctx, self.settings.ui_scale);
            self.applied_ui_scale = self.settings.ui_scale;
        }
        // persist settings once a slider is released
        if self.settings != self.saved_settings && !ctx.input(|i| i.pointer.any_down()) {
            self.settings.save();
            self.saved_settings = self.settings.clone();
        }
        self.handle_dropped_files(ctx);
        self.ensure_wayback(ctx);
        self.poll_wayback();
        self.poll_suggestions(ctx);
        self.poll_search();
        if let Some(rx) = self.boundaries_rx.take() {
            let mut finished = false;
            loop {
                match rx.try_recv() {
                    Ok(patch) => self.merge_patch(patch),
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => { finished = true; break; }
                }
            }
            if !finished {
                self.boundaries_rx = Some(rx);
                ctx.request_repaint_after(Duration::from_millis(300)); // keep polling while layers load
            }
        }
        self.step_fly(dt);
        if self.fly.is_some() {
            ctx.request_repaint();
        }
        self.top_bar(ctx);
        self.bottom_bar(ctx);
        self.side_panel(ctx);
        self.settings_window(ctx);
        egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| self.globe_view(ui, ctx));
    }
}

fn main() -> Result<(), eframe::Error> {
    let mut viewport = egui::ViewportBuilder::default().with_inner_size([1280.0, 800.0]).with_title("EarthCraft").with_app_id("earthcraft");
    if let Ok(img) = image::load_from_memory(include_bytes!("../assets/icon_256.png")) {
        let img = img.to_rgba8();
        viewport = viewport.with_icon(egui::IconData { width: img.width(), height: img.height(), rgba: img.into_raw() });
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native("EarthCraft", options, Box::new(|cc| Box::new(App::new(cc))))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app_cam(cam: Cam) -> (Cam, Rect, f64) {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0));
        (cam, rect, 1200.0 / 800.0)
    }

    /// Mirrors App::zoom_at/rotate_cam on a bare camera to check that the point under the cursor stays put.
    fn zoom_at_cam(cam: &mut Cam, factor: f64, cursor: Pos2, rect: Rect, aspect: f64) {
        // Reuse the real implementation through a minimal App-less copy of the math.
        let before = cam.basis(aspect).pick(rect, cursor).unwrap();
        cam.alt = (cam.alt * factor).clamp(MIN_ALT, MAX_ALT);
        let p1 = cam.basis(aspect).pick(rect, cursor).unwrap();
        let (from, to) = (ll_to_v3(p1.0, p1.1), ll_to_v3(before.0, before.1));
        let axis = from.cross(to);
        let sn = axis.dot(axis).sqrt();
        if sn < 1e-13 {
            return;
        }
        let ang = sn.atan2(from.dot(to));
        let k = axis * (1.0 / sn);
        let rot = |v: V3| v * ang.cos() + k.cross(v) * ang.sin() + k * (k.dot(v) * (1.0 - ang.cos()));
        let n = ll_to_v3(cam.lat, cam.lon);
        let (la, lo) = (cam.lat.to_radians(), cam.lon.to_radians());
        let east = V3::new(-lo.sin(), lo.cos(), 0.0);
        let north = V3::new(-la.sin() * lo.cos(), -la.sin() * lo.sin(), la.cos());
        let h = cam.heading.to_radians();
        let up = north * h.cos() + east * h.sin();
        let (n2, u2) = (rot(n).norm(), rot(up));
        let (lat2, lon2) = v3_to_ll(n2);
        let (la2, lo2) = (lat2.to_radians(), lon2.to_radians());
        let east2 = V3::new(-lo2.sin(), lo2.cos(), 0.0);
        let north2 = V3::new(-la2.sin() * lo2.cos(), -la2.sin() * lo2.sin(), la2.cos());
        cam.lat = lat2;
        cam.lon = lon2;
        cam.heading = u2.dot(east2).atan2(u2.dot(north2)).to_degrees();
    }

    #[test]
    fn zoom_keeps_point_under_cursor() {
        for (lat, lon, hd, alt) in [(-15.0, -50.0, 0.0, 14_000.0), (40.0, 10.0, 70.0, 3000.0), (-60.0, 120.0, -130.0, 800.0)] {
            let (mut cam, rect, aspect) = test_app_cam(Cam { lat, lon, heading: hd, alt });
            for cursor in [Pos2::new(300.0, 250.0), Pos2::new(900.0, 600.0), Pos2::new(610.0, 395.0)] {
                let Some(before) = cam.basis(aspect).pick(rect, cursor) else { continue };
                for f in [0.7, 0.5, 1.3] {
                    zoom_at_cam(&mut cam, f, cursor, rect, aspect);
                    let after = cam.basis(aspect).pick(rect, cursor).unwrap();
                    assert!(haversine_km(before, after) < 0.01, "moved {} km", haversine_km(before, after));
                    // the point must stay the same one after repeated zooms too
                    let again = before;
                    assert!(haversine_km(again, after) < 0.01);
                }
            }
        }
    }

    #[test]
    fn tile_key_roundtrip() {
        for (p, z, x, y, rel) in [(Provider::Wayback, 17u8, 98_765u32, 123_456u32, 4321u16), (Provider::Satellite, 0, 0, 0, 0), (Provider::Streets, 19, 524_287, 524_287, 0), (Provider::Topo, 5, 17, 9, 0)] {
            let k = make_key(p, z, x, y, rel);
            assert_eq!(unkey(k), (p, z, x, y, rel));
        }
        assert_ne!(make_key(Provider::Wayback, 10, 1, 1, 1), make_key(Provider::Wayback, 10, 1, 1, 2));
    }

    #[test]
    fn wayback_config_parses() {
        let v: serde_json::Value = serde_json::json!({
            "10": {"itemTitle": "World Imagery (Wayback 2014-02-20)", "itemURL": "https://x/tile/10/{level}/{row}/{col}"},
            "5000": {"itemTitle": "World Imagery (Wayback 2020-01-01)"},
        });
        let l = parse_wayback_config(&v);
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].date, "2020-01-01"); // newest first
        assert!(l[0].url.contains("/5000/{level}/{row}/{col}"));
        assert_eq!(l[1].date, "2014-02-20");
    }

    #[test]
    fn big_countries() {
        let f = serde_json::json!({"properties": {"adm0_a3": "BRA", "admin": "Brazil"}});
        assert!(is_big_country(&f));
        let f = serde_json::json!({"properties": {"adm0_a3": "FRA", "admin": "France"}});
        assert!(!is_big_country(&f));
    }
}
