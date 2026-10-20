use super::brazil::load_brazil;
use super::geojson::{collect_country_geometry, collect_geojson_lines, feature_name, label_center};
use super::local;
use super::lod::DetailLevel;
use super::overture;
use super::sources::{self, FeatureSource, first_features};
use super::{
    CountryRegion, LabelKind, Layer, Line, MapLabel, Patch, StatusLog, is_big_country, log_status,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering},
    mpsc,
};
const LOCAL_MAX_ALTITUDE_KM: f64 = 110.0;
const LOCAL_LATITUDE_LIMIT_DEG: f64 = 84.0;
const LOCAL_RETRY_SECS: u64 = 75;
const LOCAL_REFRESH_SECS: u64 = 600;
const LOCAL_COUNTRY_REFRESH_SECS: u64 = 6 * 60 * 60;
const LOCAL_MOVEMENT_DEG: f64 = 0.15;
const OVERTURE_REFRESH_SECS: u64 = 600;

type Repaint = Arc<dyn Fn() + Send + Sync>;
struct ActiveJob(Arc<AtomicUsize>, Repaint);
impl Drop for ActiveJob {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
        (self.1)();
    }
}
/// Clears the "detail job running" flag when the job finishes, however it ends.
struct RunningFlag(Arc<AtomicBool>);
impl Drop for RunningFlag {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
#[derive(Default)]
struct BrazilRequests {
    labels: bool,
    geometry: bool,
}
#[derive(Clone, PartialEq)]
struct LocalRequest {
    iso3: String,
    lat: f64,
    lon: f64,
}
pub struct BoundaryLoader {
    runtime: tokio::runtime::Handle,
    tx: mpsc::Sender<Patch>,
    log: StatusLog,
    active: Arc<AtomicUsize>,
    local_active: Arc<AtomicUsize>,
    local_failed: Arc<AtomicBool>,
    metro_active: Arc<AtomicUsize>,
    requests: BrazilRequests,
    local_requested: Option<LocalRequest>,
    local_attempt: Option<std::time::Instant>,
    metro_iso3: Option<String>,
    metro_attempt: Option<std::time::Instant>,
    /// Highest round-detail level already applied to the world layers.
    world_detail: Arc<AtomicU8>,
    /// Highest level requested by the camera; the job loads this one.
    world_target: Arc<AtomicU8>,
    world_running: Arc<AtomicBool>,

    repaint: Repaint,
}
impl BoundaryLoader {
    /// Raise the world detail level as the camera approaches the surface.
    /// Repeated requests coalesce; the running job always loads the latest level.
    pub fn request_world_detail(&mut self, altitude_km: f64) {
        let wanted = DetailLevel::for_altitude(altitude_km) as u8;
        if wanted <= self.world_detail.load(Ordering::Acquire) {
            return;
        }
        self.world_target.fetch_max(wanted, Ordering::AcqRel);
        if self.world_running.swap(true, Ordering::AcqRel) {
            return;
        }
        let (log, tx) = (self.log.clone(), self.tx.clone());
        let active = self.active.clone();
        let repaint = self.repaint.clone();
        let target = self.world_target.clone();
        let detail = self.world_detail.clone();
        let running = self.world_running.clone();
        active.fetch_add(1, Ordering::Relaxed);
        self.runtime.spawn(async move {
            let _job = ActiveJob(active, repaint);
            let _running = RunningFlag(running);
            loop {
                let level = DetailLevel::from_code(target.load(Ordering::Acquire));
                if (level as u8) <= detail.load(Ordering::Acquire) {
                    break;
                }
                if let Some(p) = load_countries(&log, level).await {
                    let _ = tx.send(p);
                }
                if let Some(p) = load_states(&log, level).await {
                    let _ = tx.send(p);
                }
                if let Some(p) = load_places(&log, &sources::place_sources(level)).await {
                    let _ = tx.send(p);
                }
                detail.store(level as u8, Ordering::Release);
            }
        });
    }

    fn take_request(
        &mut self,
        cities: bool,
        metro: bool,
        labels: bool,
        altitude_km: f64,
    ) -> Option<bool> {
        let geometry = cities || metro;
        let labels = labels && altitude_km < super::OTHER_CITY_MAX_ALTITUDE_KM;
        if geometry {
            if self.requests.geometry {
                return None;
            }
            self.requests.geometry = true;
            self.requests.labels = true;
        } else if labels {
            if self.requests.labels {
                return None;
            }
            self.requests.labels = true;
        } else {
            return None;
        }
        Some(geometry)
    }
    /// Detailed municipal boundaries for the country under the camera. National sources are
    /// tried first, then geoBoundaries, then a bounded OSM request. Brazil keeps IBGE.
    pub fn request_local_boundaries(
        &mut self,
        enabled: bool,
        altitude_km: f64,
        lat: f64,
        lon: f64,
        iso3: Option<&str>,
    ) {
        let Some(iso3) = iso3 else {
            return;
        };
        if !enabled
            || iso3 == "BRA"
            || self.is_loading_local()
            || altitude_km > LOCAL_MAX_ALTITUDE_KM
            || lat.abs() >= LOCAL_LATITUDE_LIMIT_DEG
        {
            return;
        }
        if self.local_failed.swap(false, Ordering::Relaxed) {
            self.local_requested = None;
        }
        if self
            .local_attempt
            .is_some_and(|t| t.elapsed().as_secs() < LOCAL_RETRY_SECS)
        {
            return;
        }
        let viewport = local::uses_viewport(iso3);
        let refresh_secs = if viewport {
            LOCAL_REFRESH_SECS
        } else {
            LOCAL_COUNTRY_REFRESH_SECS
        };
        let request = LocalRequest {
            iso3: iso3.to_owned(),
            lat: if viewport {
                (lat / LOCAL_MOVEMENT_DEG).round()
            } else {
                0.0
            },
            lon: if viewport {
                (lon / LOCAL_MOVEMENT_DEG).round()
            } else {
                0.0
            },
        };
        if self.local_requested.as_ref() == Some(&request)
            && self
                .local_attempt
                .is_some_and(|t| t.elapsed().as_secs() < refresh_secs)
        {
            return;
        }
        self.local_requested = Some(request);
        self.local_attempt = Some(std::time::Instant::now());
        let tx = self.tx.clone();
        let log = self.log.clone();
        let active = self.local_active.clone();
        let failed = self.local_failed.clone();
        active.fetch_add(1, Ordering::Relaxed);
        let job = ActiveJob(active, self.repaint.clone());
        let iso3 = iso3.to_owned();
        self.runtime.spawn(async move {
            let _job = job;
            match local::load_local(&log, &iso3, lat, lon).await {
                Ok(patch) => {
                    let _ = tx.send(patch);
                }
                Err(error) => {
                    failed.store(true, Ordering::Relaxed);
                    log_status(&log, format!("✖ Municipal borders {iso3}: {error}"));
                }
            }
        });
    }

    /// Optional Overture division slices pre-baked outside the app. Brazil keeps IBGE.
    pub fn request_overture_metro(&mut self, enabled: bool, iso3: Option<&str>) {
        let Some(iso3) = iso3 else {
            return;
        };
        if !enabled
            || iso3 == "BRA"
            || self.is_loading_overture_metro()
            || (self.metro_iso3.as_deref() == Some(iso3)
                && self
                    .metro_attempt
                    .is_some_and(|t| t.elapsed().as_secs() < OVERTURE_REFRESH_SECS))
        {
            return;
        }
        self.metro_iso3 = Some(iso3.to_owned());
        self.metro_attempt = Some(std::time::Instant::now());
        let tx = self.tx.clone();
        let log = self.log.clone();
        let active = self.metro_active.clone();
        active.fetch_add(1, Ordering::Relaxed);
        let job = ActiveJob(active, self.repaint.clone());
        let iso3 = iso3.to_owned();
        self.runtime.spawn(async move {
            let _job = job;
            match overture::load_overture_metro(&iso3).await {
                Ok(patch) => {
                    log_status(
                        &log,
                        format!("✔ Overture metro {iso3}: {} lines", patch.lines.len()),
                    );
                    let _ = tx.send(patch);
                }
                Err(error) => {
                    log_status(
                        &log,
                        format!(
                            "✖ Overture metro {iso3}: {error} (run scripts/prebake_overture_divisions.py)"
                        ),
                    );
                }
            }
        });
    }
    pub fn is_loading(&self) -> bool {
        self.is_loading_base_layers() || self.is_loading_local() || self.is_loading_overture_metro()
    }
    pub fn is_loading_base_layers(&self) -> bool {
        self.active.load(Ordering::Relaxed) > 0
    }
    pub fn is_loading_local(&self) -> bool {
        self.local_active.load(Ordering::Relaxed) > 0
    }
    pub fn is_loading_overture_metro(&self) -> bool {
        self.metro_active.load(Ordering::Relaxed) > 0
    }
    pub fn request_visible_layers(
        &mut self,
        cities: bool,
        metro: bool,
        labels: bool,
        altitude_km: f64,
    ) {
        let Some(geometry) = self.take_request(cities, metro, labels, altitude_km) else {
            return;
        };
        let (log, tx) = (self.log.clone(), self.tx.clone());
        let active = self.active.clone();
        active.fetch_add(1, Ordering::Relaxed);
        let job = ActiveJob(active, self.repaint.clone());
        self.runtime.spawn(async move {
            let _job = job;
            load_brazil(&log, &tx, geometry).await;
        });
    }
}
// World layers start at the bundled low detail, then rise with the camera.
// Background async tasks deliver each completed layer through the application channel.
pub fn load_boundaries_async(
    log: StatusLog,
    runtime: &tokio::runtime::Handle,
    repaint: impl Fn() + Send + Sync + 'static,
) -> (mpsc::Receiver<Patch>, BoundaryLoader) {
    let (tx, rx) = mpsc::channel();
    let repaint: Repaint = Arc::new(repaint);
    let active = Arc::new(AtomicUsize::new(2));

    // The 1:110m set is compiled into the binary, so a cold start needs no network.
    let (t, l) = (tx.clone(), log.clone());
    let job = ActiveJob(active.clone(), repaint.clone());
    runtime.spawn(async move {
        let _job = job;
        if let Some(p) = load_countries(&l, DetailLevel::Low).await {
            let _ = t.send(p);
        }
    });

    let (t, l) = (tx.clone(), log.clone());
    let job = ActiveJob(active.clone(), repaint.clone());
    runtime.spawn(async move {
        let _job = job;
        if let Some(p) = load_places(&l, &sources::place_sources(DetailLevel::Low)).await {
            let _ = t.send(p);
        }
    });

    (
        rx,
        BoundaryLoader {
            runtime: runtime.clone(),
            tx,
            log,
            active,
            local_active: Arc::new(AtomicUsize::new(0)),
            local_failed: Arc::new(AtomicBool::new(false)),
            metro_active: Arc::new(AtomicUsize::new(0)),
            requests: BrazilRequests::default(),
            local_requested: None,
            local_attempt: None,
            metro_iso3: None,
            metro_attempt: None,
            world_detail: Arc::new(AtomicU8::new(DetailLevel::Low as u8)),
            world_target: Arc::new(AtomicU8::new(DetailLevel::Low as u8)),
            world_running: Arc::new(AtomicBool::new(false)),
            repaint,
        },
    )
}

#[derive(Clone, Copy)]
struct PolygonLayer {
    what: &'static str,
    layer: Layer,
    kind: LabelKind,
    en_keys: &'static [&'static str],
    pt_keys: &'static [&'static str],
    rank: u8,
}
#[derive(Default)]
struct PolygonOutput {
    lines: Vec<Line>,
    lines_big: Vec<Line>,
    labels: Vec<MapLabel>,
    regions: Vec<CountryRegion>,
}
fn country_iso3(feature: &serde_json::Value) -> Option<String> {
    let props = feature.get("properties")?;
    for key in ["ADM0_A3", "ISO_A3", "SOV_A3", "adm0_a3", "iso_a3", "sov_a3"] {
        if let Some(code) = props.get(key).and_then(|v| v.as_str())
            && code.len() == 3
            && code != "-99"
            && code.chars().all(|c| c.is_ascii_alphabetic())
        {
            return Some(code.to_ascii_uppercase());
        }
    }
    None
}
async fn load_polygon_layer(
    log: &StatusLog,
    sources: &[FeatureSource],
    options: PolygonLayer,
) -> Option<Patch> {
    let PolygonLayer {
        what,
        layer,
        kind,
        en_keys,
        pt_keys,
        rank,
    } = options;
    let output = first_features(
        log,
        what,
        sources,
        move |f: &serde_json::Value, out: &mut PolygonOutput| {
            let Some(geometry) = f.get("geometry") else {
                return;
            };
            let big = layer == Layer::States && is_big_country(f);
            if layer == Layer::Countries {
                let (country_lines, rings) = collect_country_geometry(geometry);
                out.lines.extend(country_lines);
                if let Some(iso3) = country_iso3(f)
                    && let Some(region) = CountryRegion::new(iso3, rings)
                {
                    out.regions.push(region);
                }
            } else if let Some(coords) = geometry.get("coordinates") {
                collect_geojson_lines(
                    coords,
                    if big {
                        &mut out.lines_big
                    } else {
                        &mut out.lines
                    },
                );
            }
            if let (Some(en), Some((lat, lon))) = (feature_name(f, en_keys), label_center(geometry))
            {
                let pt = feature_name(f, pt_keys).unwrap_or_else(|| en.clone());
                out.labels.push(MapLabel {
                    lat,
                    lon,
                    en,
                    pt,
                    kind,
                    rank,
                    big,
                });
            }
        },
    )
    .await?;
    log_status(
        log,
        format!(
            "✔ {what}: {} lines, {} names",
            output.lines.len() + output.lines_big.len(),
            output.labels.len()
        ),
    );
    Some(Patch {
        layer,
        lines: output.lines,
        lines_big: output.lines_big,
        labels: output.labels,
        regions: output.regions,
        replace: false,
    })
}
async fn load_places(log: &StatusLog, sources: &[FeatureSource]) -> Option<Patch> {
    let labels = first_features(
        log,
        "Cities",
        sources,
        |f: &serde_json::Value, labels: &mut Vec<MapLabel>| {
            let coords = f.pointer("/geometry/coordinates");
            let (Some(coords), Some(en)) =
                (coords, feature_name(f, &["NAME", "name", "NAMEASCII"]))
            else {
                return;
            };
            let Some((lat, lon)) = crate::json_fields::coordinates(coords) else {
                return;
            };
            let pt = feature_name(f, &["NAME_PT", "name_pt"]).unwrap_or_else(|| en.clone());
            let rank = f
                .get("properties")
                .and_then(|p| p.get("SCALERANK").or_else(|| p.get("scalerank")))
                .and_then(|v| v.as_f64())
                .unwrap_or(5.0)
                .clamp(0.0, 8.0) as u8;
            labels.push(MapLabel {
                lat,
                lon,
                en,
                pt,
                kind: LabelKind::City,
                rank,
                big: false,
            });
        },
    )
    .await?;
    log_status(log, format!("✔ Cities: {} names", labels.len()));
    Some(Patch {
        layer: Layer::Cities,
        lines: vec![],
        lines_big: vec![],
        labels,
        regions: vec![],
        replace: false,
    })
}

fn disputed_type(feature: &serde_json::Value) -> bool {
    let value = feature
        .get("properties")
        .and_then(|p| p.get("TYPE").or_else(|| p.get("type")))
        .and_then(|v| v.as_str());
    matches!(
        value,
        Some("Disputed" | "Indeterminate" | "Lease" | "Overlay")
    )
}

async fn load_disputed(log: &StatusLog, sources: &[FeatureSource]) -> Option<Patch> {
    let lines = first_features(
        log,
        "Disputed boundaries",
        sources,
        |f: &serde_json::Value, out: &mut Vec<Line>| {
            if disputed_type(f)
                && let Some(coords) = f.pointer("/geometry/coordinates")
            {
                collect_geojson_lines(coords, out);
            }
        },
    )
    .await?;
    log_status(log, format!("✔ Disputed boundaries: {} lines", lines.len()));
    Some(Patch {
        layer: Layer::Countries,
        lines,
        lines_big: vec![],
        labels: vec![],
        regions: vec![],
        replace: false,
    })
}

async fn load_countries(log: &StatusLog, level: DetailLevel) -> Option<Patch> {
    let mut patch = load_polygon_layer(
        log,
        &sources::country_sources(level),
        PolygonLayer {
            what: "Countries",
            layer: Layer::Countries,
            kind: LabelKind::Country,
            en_keys: &["NAME", "ADMIN", "name"],
            pt_keys: &["NAME_PT", "name_pt", "NAME", "ADMIN"],
            rank: 0,
        },
    )
    .await?;
    // A detail upgrade replaces the previous outlines, so disputed linework must
    // travel in the same patch or it would be discarded with them.
    patch.replace = true;
    let disputed = sources::map_unit_sources(level);
    if !disputed.is_empty()
        && let Some(more) = load_disputed(log, &disputed).await
    {
        patch.lines.extend(more.lines);
    }
    Some(patch)
}

async fn load_states(log: &StatusLog, level: DetailLevel) -> Option<Patch> {
    let sources = sources::state_sources(level);
    if sources.is_empty() {
        return None;
    }
    let mut patch = load_polygon_layer(
        log,
        &sources,
        PolygonLayer {
            what: "States",
            layer: Layer::States,
            kind: LabelKind::State,
            en_keys: &["name_en", "name", "NAME_1", "gn_name"],
            pt_keys: &["name_pt", "name", "name_en", "NAME_1"],
            rank: 0,
        },
    )
    .await?;
    patch.replace = true;
    Some(patch)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_low_detail_is_the_cold_start_path() {
        let countries = sources::country_sources(DetailLevel::Low);
        assert!(matches!(countries.as_slice(), [FeatureSource::Bundled(_)]));
        assert!(!countries.iter().any(|s| matches!(s, FeatureSource::Url(_))));
    }

    #[test]
    fn country_iso3_skips_natural_earth_unknown_codes() {
        let feature = serde_json::json!({
            "properties": {"ADM0_A3": "FRA", "ISO_A3": "-99"}
        });
        assert_eq!(country_iso3(&feature).as_deref(), Some("FRA"));
        let feature = serde_json::json!({"properties": {"ISO_A3": "-99"}});
        assert_eq!(country_iso3(&feature), None);
    }

    #[test]
    fn disputed_map_units_are_limited_to_political_overlays() {
        for kind in ["Disputed", "Indeterminate", "Lease", "Overlay"] {
            assert!(disputed_type(
                &serde_json::json!({"properties": {"TYPE": kind}})
            ));
        }
        assert!(!disputed_type(
            &serde_json::json!({"properties": {"TYPE": "Sovereign country"}})
        ));
    }

    #[tokio::test]
    async fn municipal_layers_start_only_when_needed_and_upgrade_once() {
        let (tx, _rx) = mpsc::channel();
        let mut loader = BoundaryLoader {
            runtime: tokio::runtime::Handle::current(),
            tx,
            log: Arc::new(std::sync::Mutex::new(Vec::new())),
            active: Arc::new(AtomicUsize::new(0)),
            local_active: Arc::new(AtomicUsize::new(0)),
            local_failed: Arc::new(AtomicBool::new(false)),
            metro_active: Arc::new(AtomicUsize::new(0)),
            requests: BrazilRequests::default(),
            local_requested: None,
            local_attempt: None,
            metro_iso3: None,
            metro_attempt: None,
            world_detail: Arc::new(AtomicU8::new(DetailLevel::Low as u8)),
            world_target: Arc::new(AtomicU8::new(DetailLevel::Low as u8)),
            world_running: Arc::new(AtomicBool::new(false)),
            repaint: Arc::new(|| {}),
        };
        assert_eq!(loader.take_request(false, false, true, 14000.0), None);
        assert_eq!(loader.take_request(false, false, true, 300.0), None);
        assert_eq!(loader.take_request(false, false, true, 299.0), Some(false));
        assert_eq!(loader.take_request(false, false, true, 299.0), None);
        assert_eq!(loader.take_request(true, false, true, 14000.0), Some(true));
        assert_eq!(loader.take_request(false, true, true, 100.0), None);
        assert!(!loader.is_loading());
        loader.local_active.store(1, Ordering::Relaxed);
        assert!(loader.is_loading());
        assert!(loader.is_loading_local());
        assert!(!loader.is_loading_base_layers());
        loader.request_local_boundaries(true, 100.0, 10.0, 20.0, Some("USA"));
        assert!(loader.local_attempt.is_none());
        assert!(loader.local_requested.is_none());
    }
}

#[cfg(test)]
mod completion_tests {
    use super::*;
    #[test]
    fn finishing_or_cancelling_a_job_wakes_the_ui_after_updating_activity() {
        let active = Arc::new(AtomicUsize::new(1));
        let count = active.clone();
        let repaint = Arc::new(AtomicUsize::new(0));
        let calls = repaint.clone();
        let job = ActiveJob(
            active.clone(),
            Arc::new(move || {
                assert_eq!(count.load(Ordering::Relaxed), 0);
                calls.fetch_add(1, Ordering::Relaxed);
            }),
        );
        drop(job);
        assert_eq!(active.load(Ordering::Relaxed), 0);
        assert_eq!(repaint.load(Ordering::Relaxed), 1);
    }
}
