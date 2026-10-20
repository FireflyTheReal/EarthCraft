//! geoBoundaries gbOpen fallback: one simplified per-country administrative file.
use super::Detail;
use super::geojson::{collect_geojson_lines, feature_name, label_center};
use super::{LOCAL_LABEL_RANK, LabelKind, MapLabel};
use crate::http::fetch_json;

const API: &str = "https://www.geoboundaries.org/api/current/gbOpen";
const NAME_KEYS: &[&str] = &["shapeName", "name", "NAME"];

pub(super) fn level(iso3: &str) -> &'static str {
    match iso3 {
        "CAN" => "ADM3",
        "FRA" => "ADM4",
        "DEU" => "ADM3",
        "ESP" => "ADM3",
        _ => "ADM2",
    }
}

pub(super) fn metadata_url(iso3: &str, level: &str) -> String {
    format!("{API}/{iso3}/{level}/")
}

pub(super) async fn load(iso3: &str) -> Result<Detail, crate::Error> {
    let level = level(iso3);
    let metadata = fetch_json(&metadata_url(iso3, level)).await?;
    let url = metadata
        .get("simplifiedGeometryGeoJSON")
        .or_else(|| metadata.get("gjDownloadURL"))
        .and_then(|value| value.as_str())
        .filter(|url| !url.is_empty())
        .ok_or_else(|| {
            crate::Error::from(format!("geoBoundaries {iso3} {level}: no GeoJSON URL"))
        })?;
    crate::features::fetch(url, |feature: &serde_json::Value, out: &mut Detail| {
        out.features += 1;
        let Some(geometry) = feature.get("geometry").filter(|value| !value.is_null()) else {
            return;
        };
        if let Some(coords) = geometry.get("coordinates") {
            collect_geojson_lines(coords, &mut out.lines);
        }
        let Some(name) = feature_name(feature, NAME_KEYS) else {
            return;
        };
        let Some((lat, lon)) = label_center(geometry) else {
            return;
        };
        out.labels.push(MapLabel {
            lat,
            lon,
            en: name.clone(),
            pt: name,
            kind: LabelKind::City,
            rank: LOCAL_LABEL_RANK,
            big: false,
        });
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deepest_available_level_is_selected_per_country() {
        assert_eq!(level("USA"), "ADM2");
        assert_eq!(level("CAN"), "ADM3");
        assert_eq!(level("FRA"), "ADM4");
        assert_eq!(level("DEU"), "ADM3");
        assert_eq!(level("ESP"), "ADM3");
        assert_eq!(level("MEX"), "ADM2");
    }

    #[test]
    fn metadata_endpoint_uses_the_documented_gbopen_path() {
        assert_eq!(
            metadata_url("BRA", "ADM2"),
            "https://www.geoboundaries.org/api/current/gbOpen/BRA/ADM2/"
        );
    }
}
