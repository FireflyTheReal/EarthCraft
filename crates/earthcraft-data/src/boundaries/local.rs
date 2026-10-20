//! Detailed municipal layers under the camera: national source, geoBoundaries, then OSM.
use super::world::load_osm_cities;
use super::{Detail, Layer, Patch, StatusLog, adm, log_status, national};

pub(super) fn uses_viewport(iso3: &str) -> bool {
    national::uses_viewport(iso3)
}

fn detail_patch(detail: Detail) -> Patch {
    Patch {
        layer: Layer::WorldCities,
        lines: detail.lines,
        lines_big: vec![],
        labels: detail.labels,
        regions: vec![],
        replace: false,
    }
}

pub(super) async fn load_local(
    log: &StatusLog,
    iso3: &str,
    lat: f64,
    lon: f64,
) -> Result<Patch, crate::Error> {
    let mut errors = Vec::new();
    if let Some(label) = national::source_label(iso3) {
        match national::load(iso3, lat, lon).await {
            Some(Ok(detail)) if !detail.lines.is_empty() => {
                log_status(
                    log,
                    format!(
                        "✔ {label}: {} lines, {} names",
                        detail.lines.len(),
                        detail.labels.len()
                    ),
                );
                return Ok(detail_patch(detail));
            }
            Some(Ok(_)) => errors.push(format!("{label}: empty response")),
            Some(Err(error)) => {
                log_status(log, format!("↪ {label}: {error}; trying geoBoundaries"));
                errors.push(format!("{label}: {error}"));
            }
            None => {}
        }
    }

    let level = adm::level(iso3);
    match adm::load(iso3).await {
        Ok(detail) if !detail.lines.is_empty() => {
            log_status(
                log,
                format!(
                    "✔ geoBoundaries {iso3} {level}: {} lines, {} names",
                    detail.lines.len(),
                    detail.labels.len()
                ),
            );
            return Ok(detail_patch(detail));
        }
        Ok(_) => errors.push(format!("geoBoundaries {iso3} {level}: empty response")),
        Err(error) => {
            log_status(
                log,
                format!("↪ geoBoundaries {iso3} {level}: {error}; trying OSM"),
            );
            errors.push(format!("geoBoundaries {iso3} {level}: {error}"));
        }
    }

    match load_osm_cities(lat, lon).await {
        Ok(patch) if !patch.lines.is_empty() => {
            log_status(
                log,
                format!(
                    "✔ OSM municipal borders (fallback): {} lines",
                    patch.lines.len()
                ),
            );
            return Ok(patch);
        }
        Ok(_) => errors.push("OSM: empty response".into()),
        Err(error) => errors.push(format!("OSM: {error}")),
    }

    Err(format!("all detailed sources failed: {}", errors.join("; ")).into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn detailed_patch_replaces_the_world_municipal_layer() {
        let detail = Detail {
            lines: vec![Arc::new(vec![(0.0, 0.0), (1.0, 1.0)])],
            labels: vec![],
            features: 1,
        };
        let patch = detail_patch(detail);
        assert!(patch.layer == Layer::WorldCities);
        assert_eq!(patch.lines.len(), 1);
    }

    #[test]
    fn only_viewport_sources_reload_while_panning() {
        assert!(uses_viewport("FRA"));
        assert!(!uses_viewport("USA"));
    }
}
