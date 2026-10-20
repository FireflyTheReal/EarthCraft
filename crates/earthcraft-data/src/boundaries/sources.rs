//! Where a world overlay's feature collection comes from.
//!
//! Low detail ships with the binary so a cold start draws the world without a
//! network request. Higher detail is fetched from upstream and replaces it.
use super::lod::DetailLevel;
use super::{StatusLog, log_status};
use std::fmt;

const NE: &str = "https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson";

/// Natural Earth 1:110m, public domain. Bundled from `assets/geo`.
const BUNDLED_COUNTRIES: &str =
    include_str!("../../../../assets/geo/ne_110m_admin_0_countries.geojson");
const BUNDLED_PLACES: &str =
    include_str!("../../../../assets/geo/ne_110m_populated_places.geojson");

pub(super) enum FeatureSource {
    Bundled(&'static str),
    Url(String),
}

impl fmt::Display for FeatureSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bundled(_) => f.write_str("bundled 1:110m"),
            Self::Url(url) => f.write_str(url),
        }
    }
}

/// Try each source in order and keep the first collection that decodes.
pub(super) async fn first_features<T, F>(
    log: &StatusLog,
    what: &str,
    sources: &[FeatureSource],
    consume: F,
) -> Option<T>
where
    T: Default + Send + 'static,
    F: Fn(&serde_json::Value, &mut T) + Clone + Send + 'static,
{
    let mut last = String::from("no source configured");
    for source in sources {
        let result = match source {
            FeatureSource::Bundled(text) => {
                let text = *text;
                let consume = consume.clone();
                crate::cpu::run(crate::cpu::Service::Features, move || {
                    crate::features::project(text.as_bytes(), consume)
                })
                .await
            }
            FeatureSource::Url(url) => crate::features::fetch(url, consume.clone()).await,
        };
        match result {
            Ok(value) => return Some(value),
            Err(error) => last = format!("{source}: {error}"),
        }
    }
    log_status(log, format!("✖ {what}: {last}"));
    None
}

fn natural_earth(name: &str) -> FeatureSource {
    FeatureSource::Url(format!("{NE}/{name}.geojson"))
}

pub(super) fn country_sources(level: DetailLevel) -> Vec<FeatureSource> {
    match level {
        DetailLevel::Low => vec![FeatureSource::Bundled(BUNDLED_COUNTRIES)],
        DetailLevel::Medium => vec![
            natural_earth("ne_50m_admin_0_countries"),
            FeatureSource::Bundled(BUNDLED_COUNTRIES),
        ],
        DetailLevel::High => vec![
            natural_earth("ne_10m_admin_0_countries"),
            natural_earth("ne_50m_admin_0_countries"),
            FeatureSource::Bundled(BUNDLED_COUNTRIES),
        ],
    }
}

/// Admin-1 has no 1:110m edition, so the lowest level draws none.
pub(super) fn state_sources(level: DetailLevel) -> Vec<FeatureSource> {
    match level {
        DetailLevel::Low => Vec::new(),
        DetailLevel::Medium => vec![natural_earth("ne_50m_admin_1_states_provinces")],
        DetailLevel::High => vec![
            natural_earth("ne_10m_admin_1_states_provinces"),
            natural_earth("ne_50m_admin_1_states_provinces"),
        ],
    }
}

pub(super) fn place_sources(level: DetailLevel) -> Vec<FeatureSource> {
    match level {
        DetailLevel::Low => vec![FeatureSource::Bundled(BUNDLED_PLACES)],
        DetailLevel::Medium => vec![
            natural_earth("ne_50m_populated_places"),
            FeatureSource::Bundled(BUNDLED_PLACES),
        ],
        DetailLevel::High => vec![natural_earth("ne_10m_populated_places")],
    }
}

/// Disputed linework, drawn on top of the country outlines.
pub(super) fn map_unit_sources(level: DetailLevel) -> Vec<FeatureSource> {
    match level {
        DetailLevel::Low => Vec::new(),
        DetailLevel::Medium => vec![natural_earth("ne_50m_admin_0_map_units")],
        DetailLevel::High => vec![
            natural_earth("ne_10m_admin_0_map_units"),
            natural_earth("ne_50m_admin_0_map_units"),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_detail_is_bundled_and_needs_no_network() {
        assert!(matches!(
            country_sources(DetailLevel::Low).as_slice(),
            [FeatureSource::Bundled(_)]
        ));
        assert!(matches!(
            place_sources(DetailLevel::Low).as_slice(),
            [FeatureSource::Bundled(_)]
        ));
        assert!(state_sources(DetailLevel::Low).is_empty());
        assert!(map_unit_sources(DetailLevel::Low).is_empty());
    }

    #[test]
    fn higher_detail_prefers_finer_upstream_with_coarse_fallbacks() {
        let high = country_sources(DetailLevel::High);
        assert!(
            matches!(&high[0], FeatureSource::Url(url) if url.ends_with("ne_10m_admin_0_countries.geojson"))
        );
        assert!(
            matches!(&high[1], FeatureSource::Url(url) if url.ends_with("ne_50m_admin_0_countries.geojson"))
        );
        assert!(matches!(high.last(), Some(FeatureSource::Bundled(_))));

        let medium = state_sources(DetailLevel::Medium);
        assert!(
            matches!(&medium[0], FeatureSource::Url(url) if url.ends_with("ne_50m_admin_1_states_provinces.geojson"))
        );
    }

    #[test]
    fn bundled_collections_decode() {
        #[derive(Default)]
        struct Count(usize);
        let count = crate::features::project(BUNDLED_COUNTRIES.as_bytes(), |_, out: &mut Count| {
            out.0 += 1
        })
        .unwrap();
        assert_eq!(count.0, 177);

        let places =
            crate::features::project(BUNDLED_PLACES.as_bytes(), |_, out: &mut Count| out.0 += 1)
                .unwrap();
        assert_eq!(places.0, 243);
    }
}
