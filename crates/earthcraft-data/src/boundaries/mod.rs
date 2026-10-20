use earthcraft_core::language::Lang;
use std::sync::{Arc, Mutex};
mod adm;
mod brazil;
mod geojson;
mod load;
mod local;
mod national;
mod overture;
mod updates;
mod world;
pub use geojson::dedupe_city_labels;
pub use load::{BoundaryLoader, load_boundaries_async};
pub use updates::{BoundarySnapshot, BoundaryUpdates};
pub type Line = Arc<Vec<(f64, f64)>>;

#[derive(Clone, Copy)]
pub(crate) struct GeoBounds {
    min_lat: f64,
    max_lat: f64,
    min_lon: f64,
    max_lon: f64,
}
impl GeoBounds {
    fn of(line: &[(f64, f64)]) -> Option<Self> {
        let (mut min_lat, mut max_lat) = (f64::MAX, f64::MIN);
        let (mut min_lon, mut max_lon) = (f64::MAX, f64::MIN);
        for &(lat, lon) in line {
            if !lat.is_finite() || !lon.is_finite() {
                continue;
            }
            min_lat = min_lat.min(lat);
            max_lat = max_lat.max(lat);
            min_lon = min_lon.min(lon);
            max_lon = max_lon.max(lon);
        }
        (min_lat <= max_lat).then_some(Self {
            min_lat,
            max_lat,
            min_lon,
            max_lon,
        })
    }
    fn union(self, other: Self) -> Self {
        Self {
            min_lat: self.min_lat.min(other.min_lat),
            max_lat: self.max_lat.max(other.max_lat),
            min_lon: self.min_lon.min(other.min_lon),
            max_lon: self.max_lon.max(other.max_lon),
        }
    }
    fn contains(self, lat: f64, lon: f64) -> bool {
        lat >= self.min_lat && lat <= self.max_lat && lon >= self.min_lon && lon <= self.max_lon
    }
}

/// One outer ring of a country, retained only to identify the country under the camera.
#[derive(Clone)]
pub(crate) struct CountryRing {
    line: Line,
    bounds: GeoBounds,
}
impl CountryRing {
    pub(super) fn new(line: Line) -> Option<Self> {
        let bounds = GeoBounds::of(&line)?;
        Some(Self { line, bounds })
    }
    fn contains(&self, lat: f64, lon: f64) -> bool {
        if !self.bounds.contains(lat, lon) {
            return false;
        }
        let points = self.line.as_slice();
        let mut inside = false;
        for i in 0..points.len() {
            let (lat_a, lon_a) = points[i];
            let (lat_b, lon_b) = points[(i + 1) % points.len()];
            if (lat_a > lat) != (lat_b > lat)
                && lon < (lon_b - lon_a) * (lat - lat_a) / (lat_b - lat_a) + lon_a
            {
                inside = !inside;
            }
        }
        inside
    }
}

#[derive(Clone)]
pub(crate) struct CountryRegion {
    iso3: String,
    rings: Vec<CountryRing>,
    bounds: GeoBounds,
}
impl CountryRegion {
    pub(super) fn new(iso3: String, rings: Vec<CountryRing>) -> Option<Self> {
        let bounds = rings
            .iter()
            .map(|ring| ring.bounds)
            .reduce(GeoBounds::union)?;
        Some(Self {
            iso3,
            rings,
            bounds,
        })
    }
    fn contains(&self, lat: f64, lon: f64) -> bool {
        self.bounds.contains(lat, lon) && self.rings.iter().any(|ring| ring.contains(lat, lon))
    }
}

#[derive(Clone, Default)]
pub struct BoundaryData {
    pub countries: Vec<Line>,
    pub states: Vec<Line>, // admin-1 of "ordinary" countries (shown at medium zoom)
    pub states_big: Vec<Line>, // admin-1 of the largest countries (shown even with the whole Earth in view)
    pub cities: Vec<Line>,
    pub world_cities: Vec<Line>,
    pub metro: Vec<Line>,
    pub labels: Vec<MapLabel>,
    country_regions: Vec<CountryRegion>,
    countries_loaded: bool,
}
impl BoundaryData {
    /// ISO3 of the country containing the point, from Natural Earth outer rings.
    pub fn country_at(&self, lat: f64, lon: f64) -> Option<&str> {
        self.country_regions
            .iter()
            .find(|region| region.contains(lat, lon))
            .map(|region| region.iso3.as_str())
    }
    pub fn has_countries(&self) -> bool {
        self.countries_loaded
    }
}

/// States/provinces of these countries (the 10 largest by area) are visible when zoomed far out.
const BIG_COUNTRIES_A3: [&str; 10] = [
    "RUS", "CAN", "USA", "CHN", "BRA", "AUS", "IND", "ARG", "KAZ", "DZA",
];
const BIG_COUNTRIES_NAME: [&str; 10] = [
    "Russia",
    "Canada",
    "United States of America",
    "China",
    "Brazil",
    "Australia",
    "India",
    "Argentina",
    "Kazakhstan",
    "Algeria",
];
/// Below this camera altitude (km) the states of every other country appear too.
pub const OTHER_STATES_MAX_ALTITUDE_KM: f64 = 3000.0;
const BIG_STATE_LABEL_MAX_ALTITUDE_KM: f64 = 22_000.0;

/// Municipal labels from live/national boundary sources; base populated places rank 0-8.
pub(super) const LOCAL_LABEL_RANK: u8 = 10;
const METRO_LABEL_MAX_ALTITUDE_KM: f64 = 5000.0;
const CITY_RANK_0_1_MAX_ALTITUDE_KM: f64 = 9000.0;
const CITY_RANK_2_3_MAX_ALTITUDE_KM: f64 = 4000.0;
const CITY_RANK_4_5_MAX_ALTITUDE_KM: f64 = 2000.0;
const CITY_RANK_6_7_MAX_ALTITUDE_KM: f64 = 900.0;
const OTHER_CITY_MAX_ALTITUDE_KM: f64 = 300.0;

pub(super) fn is_big_country(feature: &serde_json::Value) -> bool {
    let Some(props) = feature.get("properties") else {
        return false;
    };
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

#[derive(Clone)]
pub struct MapLabel {
    pub lat: f64,
    pub lon: f64,
    pub en: String,
    pub pt: String,
    pub kind: LabelKind,
    pub rank: u8,
    pub big: bool,
}
impl MapLabel {
    pub fn text(&self, lg: Lang) -> &str {
        match lg {
            Lang::En => &self.en,
            Lang::PtBr => &self.pt,
        }
    }
    /// Whether the label should be shown at the given camera altitude (km).
    /// Bigger places (lower rank) appear first, smaller ones only when zoomed in.
    pub fn visible_at(&self, alt: f64) -> bool {
        match self.kind {
            LabelKind::Country => true,
            LabelKind::State => {
                alt < if self.big {
                    BIG_STATE_LABEL_MAX_ALTITUDE_KM
                } else {
                    OTHER_STATES_MAX_ALTITUDE_KM
                }
            }
            LabelKind::Metro => alt < METRO_LABEL_MAX_ALTITUDE_KM,
            LabelKind::City => {
                alt < match self.rank {
                    0..=1 => CITY_RANK_0_1_MAX_ALTITUDE_KM,
                    2..=3 => CITY_RANK_2_3_MAX_ALTITUDE_KM,
                    4..=5 => CITY_RANK_4_5_MAX_ALTITUDE_KM,
                    6..=7 => CITY_RANK_6_7_MAX_ALTITUDE_KM,
                    _ => OTHER_CITY_MAX_ALTITUDE_KM,
                }
            }
        }
    }
}
#[derive(Clone, Copy, PartialEq)]
pub enum LabelKind {
    Country,
    State,
    City,
    Metro,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Layer {
    Countries,
    States,
    Cities,
    WorldCities,
    Metro,
}

/// One finished piece of overlay data, sent from a loader thread to the UI thread.
pub struct Patch {
    pub layer: Layer,
    pub lines: Vec<Line>,
    pub lines_big: Vec<Line>,
    pub labels: Vec<MapLabel>,
    pub(crate) regions: Vec<CountryRegion>,
}

#[derive(Default)]
pub(super) struct Detail {
    pub(super) lines: Vec<Line>,
    pub(super) labels: Vec<MapLabel>,
    pub(super) features: usize,
}

/// Human-readable load log shown in the Layers panel, so failures are never silent.
pub type StatusLog = Arc<Mutex<Vec<String>>>;

pub(super) fn log_status(log: &StatusLog, msg: String) {
    if let Ok(mut v) = log.lock() {
        v.push(msg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn big_countries() {
        let f = serde_json::json!({"properties": {"adm0_a3": "BRA", "admin": "Brazil"}});
        assert!(is_big_country(&f));
        let f = serde_json::json!({"properties": {"adm0_a3": "FRA", "admin": "France"}});
        assert!(!is_big_country(&f));
    }

    #[test]
    fn country_lookup_uses_the_outer_ring_not_its_label_centre() {
        let ring = CountryRing::new(Arc::new(vec![
            (0.0, 0.0),
            (0.0, 10.0),
            (10.0, 10.0),
            (10.0, 0.0),
            (0.0, 0.0),
        ]))
        .unwrap();
        let region = CountryRegion::new("TST".into(), vec![ring]).unwrap();
        let data = BoundaryData {
            country_regions: vec![region],
            ..Default::default()
        };
        assert_eq!(data.country_at(5.0, 5.0), Some("TST"));
        assert_eq!(data.country_at(15.0, 5.0), None);
    }
}
