//! Optional Overture division slices, pre-baked outside the app (see scripts/).
use super::geojson::{collect_geojson_lines, label_center};
use super::{Detail, LabelKind, Layer, MapLabel, Patch};
use std::path::PathBuf;

fn slice_path(iso3: &str) -> PathBuf {
    crate::paths::cache_dir()
        .join("overture")
        .join(format!("{}.geojson", iso3.to_ascii_uppercase()))
}

fn overture_name(feature: &serde_json::Value) -> Option<String> {
    feature
        .pointer("/properties/name")
        .and_then(|value| value.as_str())
        .or_else(|| {
            feature
                .pointer("/properties/names/primary")
                .and_then(|value| value.as_str())
        })
        .filter(|name| !name.trim().is_empty())
        .map(str::to_owned)
}

fn parse(bytes: Vec<u8>) -> Result<Detail, crate::Error> {
    crate::features::project(
        std::io::Cursor::new(bytes),
        |feature: &serde_json::Value, out: &mut Detail| {
            out.features += 1;
            let Some(geometry) = feature.get("geometry").filter(|value| !value.is_null()) else {
                return;
            };
            if let Some(coords) = geometry.get("coordinates") {
                collect_geojson_lines(coords, &mut out.lines);
            }
            if let (Some(name), Some((lat, lon))) = (overture_name(feature), label_center(geometry))
            {
                out.labels.push(MapLabel {
                    lat,
                    lon,
                    en: name.clone(),
                    pt: name,
                    kind: LabelKind::Metro,
                    rank: 0,
                    big: false,
                });
            }
        },
    )
}

pub(super) async fn load_overture_metro(iso3: &str) -> Result<Patch, crate::Error> {
    let path = slice_path(iso3);
    let bytes = tokio::fs::read(path).await?;
    let detail = crate::cpu::run(crate::cpu::Service::Features, move || parse(bytes)).await?;
    if detail.lines.is_empty() {
        return Err(format!("Overture {iso3} slice contains no divisions").into());
    }
    Ok(Patch {
        layer: Layer::Metro,
        lines: detail.lines,
        lines_big: vec![],
        labels: detail.labels,
        regions: vec![],
        replace: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slice_name_is_uppercase_and_lives_in_the_cache() {
        let path = slice_path("usa");
        assert!(path.ends_with("overture/USA.geojson"));
    }

    #[test]
    fn nested_overture_names_and_geojson_geometry_are_read() {
        let bytes = br#"{
            "type": "FeatureCollection",
            "features": [{
                "type": "Feature",
                "properties": {"names": {"primary": "Sample metro"}},
                "geometry": {"type": "Polygon", "coordinates": [[[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]]}
            }]
        }"#
        .to_vec();
        let detail = parse(bytes).unwrap();
        assert_eq!(detail.lines.len(), 1);
        assert_eq!(detail.labels[0].en, "Sample metro");
        assert!(detail.labels[0].kind == LabelKind::Metro);
    }
}
