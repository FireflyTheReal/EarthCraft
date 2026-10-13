//! Genuine, georeferenced online imagery (not synthetic weather / cloud blobs).
//! Cloud map: Matt Eason, "Live Cloud Maps", source contains modified EUMETSAT data.
//! Radar: RainViewer Weather Maps API; recolored to green/yellow/red using dBZ lookup.
use earthcraft_core::{camera::Basis, language::Lang};
use eframe::egui::{self, Color32, ColorImage, Pos2, Rect, TextureHandle};
use image::{imageops::FilterType, RgbaImage};
use std::{sync::mpsc, time::{Duration, Instant}};

pub(super) const CLOUD_SOURCE: &str = "https://clouds.matteason.co.uk/images/8192x4096/clouds-alpha.png";
const RADAR_API: &str = "https://api.rainviewer.com/public/weather-maps.json";
const CLOUD_INTERVAL: Duration = Duration::from_secs(3 * 60 * 60);
const RADAR_INTERVAL: Duration = Duration::from_secs(10 * 60);
const RETRY_INTERVAL: Duration = Duration::from_secs(60);
const TILE_ZOOM: usize = 2; // 16 × 512-pixel global Web-Mercator tiles
const TILE_SIZE: u32 = 512;

struct RadarFrame { image: RgbaImage, time: i64 }

pub(super) struct LiveMaps {
    cloud: Option<RgbaImage>,
    radar: Option<RgbaImage>,
    cloud_rx: Option<mpsc::Receiver<Result<RgbaImage, String>>>,
    radar_rx: Option<mpsc::Receiver<Result<RadarFrame, String>>>,
    cloud_attempt: Option<Instant>,
    radar_attempt: Option<Instant>,
    cloud_error: Option<String>,
    radar_error: Option<String>,
    radar_frame_time: Option<i64>,
    generation: u64,
    projected: Option<TextureHandle>,
    // Basis refers to the DISPLAYED raster camera, which may lag input while panning.
    projected_for: Option<(Basis, usize, usize, bool, bool, u64)>,
}
impl Default for LiveMaps {
    fn default() -> Self {
        Self {
            cloud: None, radar: None, cloud_rx: None, radar_rx: None,
            cloud_attempt: None, radar_attempt: None, cloud_error: None,
            radar_error: None, radar_frame_time: None,
            generation: 0, projected: None, projected_for: None,
        }
    }
}

impl LiveMaps {
    pub fn update(&mut self, ctx: &egui::Context, runtime: &earthcraft_data::runtime::ServiceRuntime, clouds: bool, radar: bool) {
        if let Some(rx) = &self.cloud_rx {
            match rx.try_recv() {
                Ok(Ok(image)) => {
                    self.cloud = Some(image);
                    self.cloud_error = None;
                    self.cloud_rx = None;
                    self.generation += 1;
                    ctx.request_repaint();
                }
                Ok(Err(err)) => { self.cloud_error = Some(err); self.cloud_rx = None; }
                Err(mpsc::TryRecvError::Disconnected) => { self.cloud_error = Some("Cloud download interrupted".into()); self.cloud_rx = None; }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if let Some(rx) = &self.radar_rx {
            match rx.try_recv() {
                Ok(Ok(frame)) => {
                    self.radar_frame_time = Some(frame.time);
                    self.radar = Some(frame.image);
                    self.radar_error = None;
                    self.radar_rx = None;
                    self.generation += 1;
                    ctx.request_repaint();
                }
                Ok(Err(err)) => { self.radar_error = Some(err); self.radar_rx = None; }
                Err(mpsc::TryRecvError::Disconnected) => { self.radar_error = Some("Radar download interrupted".into()); self.radar_rx = None; }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if clouds || radar { ctx.request_repaint_after(Duration::from_secs(60)); }
        if clouds && self.cloud_rx.is_none() && refresh_due(self.cloud_attempt, self.cloud.is_some() && self.cloud_error.is_none(), CLOUD_INTERVAL) {
            let (tx, rx) = mpsc::channel();
            self.cloud_rx = Some(rx);
            self.cloud_attempt = Some(Instant::now());
            let ctx = ctx.clone();
            runtime.spawn(async move {
                let result = download_clouds().await;
                let _ = tx.send(result);
                ctx.request_repaint();
            });
        }
        if radar && self.radar_rx.is_none() && refresh_due(self.radar_attempt, self.radar.is_some() && self.radar_error.is_none(), RADAR_INTERVAL) {
            let (tx, rx) = mpsc::channel();
            self.radar_rx = Some(rx);
            self.radar_attempt = Some(Instant::now());
            let ctx = ctx.clone();
            runtime.spawn(async move {
                let result = download_radar().await;
                let _ = tx.send(result);
                ctx.request_repaint();
            });
        }
    }

    pub fn draw(&mut self, painter: &egui::Painter, basis: &Basis, rect: Rect, clouds: bool, radar: bool) {
        let show_clouds = clouds && self.cloud.is_some();
        let show_radar = radar && self.radar.is_some();
        if !show_clouds && !show_radar { return; }
        // Projection is cached and updated only when camera or imagery changes.
        let max_width = if painter.ctx().input(|i| i.pointer.any_down()) { 480 } else { 960 };
        let width = ((rect.width() * painter.ctx().pixels_per_point()) as usize).clamp(160, max_width);
        let height = ((width as f32 * rect.height() / rect.width().max(1.0)) as usize).clamp(1, 960);
        let key = (*basis, width, height, show_clouds, show_radar, self.generation);
        if self.projected_for != Some(key) {
            let mut rgba = vec![0u8; width * height * 4];
            let rsize = rect.size();
            for y in 0..height {
                let sy = rect.top() + (y as f32 + 0.5) / height as f32 * rsize.y;
                for x in 0..width {
                    let sx = rect.left() + (x as f32 + 0.5) / width as f32 * rsize.x;
                    let Some((lat,lon)) = basis.pick(rect, Pos2::new(sx, sy)) else {continue};
                    let i = (y * width + x) * 4;
                    if show_clouds {
                        if let Some(img) = &self.cloud {
                            let c = equirect_pixel(img, lat, lon);
                            over(&mut rgba[i..i+4], [c[0],c[1],c[2], (c[3] as f32 * 0.78) as u8]);
                        }
                    }
                    if show_radar && lat.abs() < 85.0 {
                        if let Some(img) = &self.radar {
                            let r = mercator_pixel(img, lat, lon);
                            over(&mut rgba[i..i+4], [r[0],r[1],r[2], (r[3] as f32 * 0.88) as u8]);
                        }
                    }
                }
            }
            let im = ColorImage::from_rgba_unmultiplied([width, height], &rgba);
            if let Some(tex) = self.projected.as_mut() { tex.set(im, egui::TextureOptions::LINEAR); }
            else { self.projected = Some(painter.ctx().load_texture("live-weather-cloud-map", im, egui::TextureOptions::LINEAR)); }
            self.projected_for = Some(key);
        }
        if let Some(tex) = &self.projected {
            painter.image(tex.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        }
    }

    pub fn status(&self, clouds: bool, radar: bool, lang: Lang) -> String {
        let mut lines = Vec::new();
        if clouds {
            if let Some(err) = &self.cloud_error {
                let failed = lang.text("live-clouds-failed");
                let cached = if self.cloud.is_some() {lang.text("live-cached-image")} else {""};
                lines.push(format!("{failed}{cached}: {err}"));
            } else if self.cloud.is_some() {
                lines.push(lang.text("live-clouds-ok").to_owned());
            } else {
                lines.push(lang.text("live-clouds-loading").to_owned());
            }
        }
        if radar {
            if let Some(err) = &self.radar_error {
                let failed = lang.text("live-radar-failed");
                let cached = if self.radar.is_some() {lang.text("live-cached-image")} else {""};
                lines.push(format!("{failed}{cached}: {err}"));
            } else if self.radar.is_some() {
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
                let age = (now - self.radar_frame_time.unwrap_or(now)).max(0) / 60;
                lines.push(lang.format("live-radar-age", &[("minutes",age.to_string().into())]));
            } else {
                lines.push(lang.text("live-radar-loading").to_owned());
            }
        }
        lines.join("\n")
    }
}
fn refresh_due(last: Option<Instant>, loaded: bool, interval: Duration) -> bool {
    last.is_none_or(|t| t.elapsed() >= if loaded {interval} else {RETRY_INTERVAL})
}

async fn download_clouds() -> Result<RgbaImage, String> {
    let client = reqwest::Client::builder().timeout(Duration::from_secs(65))
        .user_agent("EarthCraft/1.0 (attribution: Matt Eason, EUMETSAT)")
        .build().map_err(|e|e.to_string())?;
    // Cache bust by 3-hour slots; URL remains the requested source image.
    let epoch = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default().as_secs() / (3*60*60);
    let url = format!("{CLOUD_SOURCE}?slot={epoch}");
    let bytes = client.get(url).send().await.map_err(|e|e.to_string())?
        .error_for_status().map_err(|e|e.to_string())?.bytes().await.map_err(|e|e.to_string())?;
    // Decode off the UI thread and keep only the working resolution in memory.
    tokio::task::spawn_blocking(move || {
        let image = image::load_from_memory(&bytes).map_err(|e|e.to_string())?;
        Ok(image.resize_exact(2048, 1024, FilterType::Triangle).to_rgba8())
    }).await.map_err(|e|e.to_string())?
}

async fn download_radar() -> Result<RadarFrame,String> {
    let client = reqwest::Client::builder().timeout(Duration::from_secs(12))
        .user_agent("EarthCraft/1.0 radar viewer").build().map_err(|e|e.to_string())?;
    let json: serde_json::Value = client.get(RADAR_API).send().await.map_err(|e|e.to_string())?
        .error_for_status().map_err(|e|e.to_string())?.json().await.map_err(|e|e.to_string())?;
    let frame = json.pointer("/radar/past").and_then(|v|v.as_array()).and_then(|v|v.last())
        .ok_or("No radar frames available")?;
    let path = frame.get("path").and_then(|v|v.as_str()).ok_or("Radar path missing")?;
    if !path.starts_with("/v2/radar/") || !path[10..].chars().all(|c| c.is_ascii_alphanumeric() || c=='/') {
        return Err("Unexpected radar path".into());
    }
    let time = frame.get("time").and_then(|v|v.as_i64()).ok_or("Radar time missing")?;
    let n = 1usize << TILE_ZOOM;
    let mut image = RgbaImage::new((n as u32)*TILE_SIZE,(n as u32)*TILE_SIZE);
    let mut accepted = 0;
    let limiter = std::sync::Arc::new(tokio::sync::Semaphore::new(4));
    let mut tasks = tokio::task::JoinSet::new();
    for y in 0..n {
        for x in 0..n {
            // RainViewer 2026 free API: ONLY color scheme 2 and max zoom 7.
            let url = format!("https://tilecache.rainviewer.com{path}/{TILE_SIZE}/{TILE_ZOOM}/{x}/{y}/2/1_0.png");
            let client = client.clone();
            let limiter = limiter.clone();
            tasks.spawn(async move {
                let _permit = limiter.acquire_owned().await.ok()?;
                let bytes = client.get(url).send().await.ok()?.error_for_status().ok()?.bytes().await.ok()?;
                let image = image::load_from_memory(&bytes).ok()?.to_rgba8();
                Some((x,y,image))
            });
        }
    }
    while let Some(result) = tasks.join_next().await {
        let Ok(Some((x,y,tile))) = result else { continue };
        accepted += 1;
        for (px,py,c) in tile.enumerate_pixels() {
            if px >= TILE_SIZE || py >= TILE_SIZE { continue; }
            let mapped = recolor_radar(c.0);
            image.put_pixel(x as u32 * TILE_SIZE + px, y as u32 * TILE_SIZE + py, image::Rgba(mapped));
        }
    }
    if accepted == 0 { return Err("RainViewer radar tiles unavailable (check network/coverage)".into()); }
    Ok(RadarFrame { image, time })
}

// The API only delivers Universal Blue as of 2026. Map its RGB palette to an
// approximate reflectivity (dBZ) and display a radar-style green-yellow-red ramp.
// This is an intensity visualization, not temperature or satellite infrared.
fn recolor_radar(src: [u8;4]) -> [u8;4] {
    if src[3] == 0 {return [0,0,0,0];}
    const PALETTE: &[(f32, [u8;3])] = &[
        (0.0,[0,90,0]),(5.0,[0,126,0]),(10.0,[8,127,219]),
        (15.0,[28,71,232]),(20.0,[110,13,198]),(25.0,[200,15,134]),
        (30.0,[192,100,135]),(35.0,[210,136,59]),(40.0,[250,196,49]),
        (45.0,[254,251,2]),(50.0,[254,154,88]),(55.0,[254,95,5]),
        (60.0,[253,52,28]),(65.0,[190,190,190]),
    ];
    let mut min_error = u32::MAX;
    let mut dbz = 0.0;
    for (value, rgb) in PALETTE {
        let err = (src[0] as i32-rgb[0] as i32).pow(2)
            +(src[1] as i32-rgb[1] as i32).pow(2)
            +(src[2] as i32-rgb[2] as i32).pow(2);
        if (err as u32) < min_error { min_error = err as u32; dbz = *value; }
    }
    let (r,g,b) = if dbz < 18.0 { (42,205,60) }
        else if dbz < 28.0 {(155,220,35)}
        else if dbz < 38.0 {(247,230,20)}
        else if dbz < 47.0 {(255,145,0)}
        else {(240,35,30)};
    [r,g,b,src[3].min(200)]
}
fn equirect_pixel(img:&RgbaImage,lat:f64,lon:f64)->[u8;4]{
    let x = (((lon+180.0)/360.0 * img.width() as f64) as u32).min(img.width()-1);
    let y = (((90.0-lat)/180.0 * img.height() as f64) as u32).min(img.height()-1);
    img.get_pixel(x,y).0
}
fn mercator_pixel(img:&RgbaImage,lat:f64,lon:f64)->[u8;4]{
    let x = (((lon+180.0)/360.0 * img.width() as f64) as u32).min(img.width()-1);
    let radians=lat.to_radians();
    let y = ((1.0 - radians.tan().asinh()/std::f64::consts::PI)/2.0 * img.height() as f64)
        .clamp(0.0,(img.height()-1) as f64) as u32;
    img.get_pixel(x,y).0
}
fn over(dst:&mut [u8],src:[u8;4]){
    let a=src[3] as f32/255.0;
    if a == 0.0 {return;}
    let previous=dst[3] as f32/255.0;
    let total=a + previous*(1.0-a);
    if total == 0.0{return;}
    for i in 0..3 { dst[i] = ((src[i] as f32*a + dst[i] as f32*previous*(1.0-a))/total) as u8; }
    dst[3]=(total*255.0) as u8;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn global_coords() {
        let mut img=RgbaImage::new(360,180);
        img.put_pixel(180,90,image::Rgba([123,45,67,255]));
        assert_eq!(equirect_pixel(&img,0.0,0.0),[123,45,67,255]);
    }
    #[test] fn alpha_blend() {
        let mut pixel=[0u8;4];over(&mut pixel,[0,200,0,128]);
        assert!(pixel[1]>190 && pixel[3]>120);
    }
}
