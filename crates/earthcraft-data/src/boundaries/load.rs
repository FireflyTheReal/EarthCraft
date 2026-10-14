use super::brazil::load_brazil;
use super::geojson::{collect_geojson_lines, feature_name, label_center};
use super::{is_big_country, log_status, LabelKind, Layer, Line, MapLabel, Patch, StatusLog};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc, Arc,
};
const NE: &str = "https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson";

type Repaint = Arc<dyn Fn() + Send + Sync>;
struct ActiveJob(Arc<AtomicUsize>, Repaint);
impl Drop for ActiveJob {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
        (self.1)();
    }
}
#[derive(Default)]
struct BrazilRequests {
    labels: bool,
    geometry: bool,
}
pub struct BoundaryLoader {
    runtime: tokio::runtime::Handle,
    tx: mpsc::Sender<Patch>,
    log: StatusLog,
    active: Arc<AtomicUsize>,
    requests: BrazilRequests,
    repaint: Repaint,
}
impl BoundaryLoader {
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
    pub fn is_loading(&self) -> bool {
        self.active.load(Ordering::Relaxed) > 0
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
// Load coarse world layers at startup; municipal geometry starts on demand.
// Background async tasks deliver each completed layer through the application channel.
pub fn load_boundaries_async(
    log: StatusLog,
    runtime: &tokio::runtime::Handle,
    repaint: impl Fn() + Send + Sync + 'static,
) -> (mpsc::Receiver<Patch>, BoundaryLoader) {
    let (tx, rx) = mpsc::channel();
    let repaint: Repaint = Arc::new(repaint);
    let active = Arc::new(AtomicUsize::new(3));

    let (t, l) = (tx.clone(), log.clone());
    let job = ActiveJob(active.clone(), repaint.clone());
    runtime.spawn(async move {
        let _job = job;
        let urls = [
            format!("{NE}/ne_50m_admin_0_countries.geojson"),
            "https://raw.githubusercontent.com/datasets/geo-countries/main/data/countries.geojson"
                .to_string(),
            format!("{NE}/ne_110m_admin_0_countries.geojson"),
        ];
        if let Some(p) = load_polygon_layer(
            &l,
            &urls,
            PolygonLayer {
                what: "Countries",
                layer: Layer::Countries,
                kind: LabelKind::Country,
                en_keys: &["NAME", "ADMIN", "name"],
                pt_keys: &["NAME_PT", "name_pt", "NAME", "ADMIN"],
                rank: 0,
            },
        )
        .await
        {
            let _ = t.send(p);
        }
    });

    let (t, l) = (tx.clone(), log.clone());
    let job = ActiveJob(active.clone(), repaint.clone());
    runtime.spawn(async move {
        let _job = job;
        let urls = [
            format!("{NE}/ne_10m_admin_1_states_provinces.geojson"),
            format!("{NE}/ne_50m_admin_1_states_provinces.geojson"),
        ];
        if let Some(p) = load_polygon_layer(
            &l,
            &urls,
            PolygonLayer {
                what: "States",
                layer: Layer::States,
                kind: LabelKind::State,
                en_keys: &["name_en", "name", "NAME_1", "gn_name"],
                pt_keys: &["name_pt", "name", "name_en", "NAME_1"],
                rank: 0,
            },
        )
        .await
        {
            let _ = t.send(p);
        }
    });

    let (t, l) = (tx.clone(), log.clone());
    let job = ActiveJob(active.clone(), repaint.clone());
    runtime.spawn(async move {
        let _job = job;
        let urls = [
            format!("{NE}/ne_10m_populated_places.geojson"),
            format!("{NE}/ne_50m_populated_places.geojson"),
        ];
        if let Some(p) = load_places(&l, &urls).await {
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
            requests: BrazilRequests::default(),
            repaint,
        },
    )
}

async fn first_features<T, F>(log: &StatusLog, what: &str, urls: &[String], consume: F) -> Option<T>
where
    T: Default + Send + 'static,
    F: Fn(&serde_json::Value, &mut T) + Clone + Send + 'static,
{
    let mut last = String::new();
    for url in urls {
        match crate::features::fetch(url, consume.clone()).await {
            Ok(value) => return Some(value),
            Err(error) => last = format!("{url}: {error}"),
        }
    }
    log_status(log, format!("✖ {what}: {last}"));
    None
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
async fn load_polygon_layer(
    log: &StatusLog,
    urls: &[String],
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
    let (lines, lines_big, labels) = first_features(
        log,
        what,
        urls,
        move |f: &serde_json::Value, out: &mut (Vec<Line>, Vec<Line>, Vec<MapLabel>)| {
            let Some(geometry) = f.get("geometry") else {
                return;
            };
            let big = layer == Layer::States && is_big_country(f);
            if let Some(coords) = geometry.get("coordinates") {
                collect_geojson_lines(coords, if big { &mut out.1 } else { &mut out.0 });
            }
            if let (Some(en), Some((lat, lon))) = (feature_name(f, en_keys), label_center(geometry))
            {
                let pt = feature_name(f, pt_keys).unwrap_or_else(|| en.clone());
                out.2.push(MapLabel {
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
            lines.len() + lines_big.len(),
            labels.len()
        ),
    );
    Some(Patch {
        layer,
        lines,
        lines_big,
        labels,
    })
}
async fn load_places(log: &StatusLog, urls: &[String]) -> Option<Patch> {
    let labels = first_features(
        log,
        "Cities",
        urls,
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn municipal_layers_start_only_when_needed_and_upgrade_once() {
        let (tx, _rx) = mpsc::channel();
        let mut loader = BoundaryLoader {
            runtime: tokio::runtime::Handle::current(),
            tx,
            log: Arc::new(std::sync::Mutex::new(Vec::new())),
            active: Arc::new(AtomicUsize::new(0)),
            requests: BrazilRequests::default(),
            repaint: Arc::new(|| {}),
        };
        assert_eq!(loader.take_request(false, false, true, 14000.0), None);
        assert_eq!(loader.take_request(false, false, true, 300.0), None);
        assert_eq!(loader.take_request(false, false, true, 299.0), Some(false));
        assert_eq!(loader.take_request(false, false, true, 299.0), None);
        assert_eq!(loader.take_request(true, false, true, 14000.0), Some(true));
        assert_eq!(loader.take_request(false, true, true, 100.0), None);
        assert!(!loader.is_loading());
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
