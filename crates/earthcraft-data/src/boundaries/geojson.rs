use super::{LabelKind, Line, MapLabel};
use earthcraft_core::language::fold_name;
use std::collections::HashMap;
/// Keeps only the segments that belong to exactly one polygon, i.e. the outer boundary of a group
/// of neighbouring municipalities (shared inner borders appear twice and are dropped).
pub(super) fn outer_outline<'a>(lines: impl Iterator<Item = &'a Line>) -> Vec<Line> {
    let q = |p: (f64, f64)| ((p.0 * 1e5).round() as i64, (p.1 * 1e5).round() as i64);
    type Point = (f64, f64);
    type QuantizedPoint = (i64, i64);
    type Edge = (QuantizedPoint, QuantizedPoint);
    type EdgeCount = (u32, Point, Point);
    let mut count: HashMap<Edge, EdgeCount> = HashMap::new();
    for line in lines {
        for w in line.windows(2) {
            let (a, b) = (q(w[0]), q(w[1]));
            if a == b {
                continue;
            }
            let key = if a <= b { (a, b) } else { (b, a) };
            count
                .entry(key)
                .and_modify(|e| e.0 += 1)
                .or_insert((1, w[0], w[1]));
        }
    }
    count
        .into_values()
        .filter(|e| e.0 == 1)
        .map(|e| vec![e.1, e.2])
        .collect()
}

pub(super) fn feature_name(feature: &serde_json::Value, keys: &[&str]) -> Option<String> {
    let props = feature.get("properties")?;
    keys.iter().find_map(|key| {
        props
            .get(*key)
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.to_owned())
    })
}

/// Label anchor: bounding-box centre of the geometry's largest outer ring (so countries with
/// far-away islands are labelled on the mainland). Returns (lat, lon).
pub(super) fn label_center(geometry: &serde_json::Value) -> Option<(f64, f64)> {
    let coords = geometry.get("coordinates")?;
    let rings: Vec<&serde_json::Value> = match geometry.get("type").and_then(|t| t.as_str())? {
        "Polygon" => coords.as_array()?.first().into_iter().collect(),
        "MultiPolygon" => coords
            .as_array()?
            .iter()
            .filter_map(|p| p.as_array()?.first())
            .collect(),
        _ => return None,
    };
    let ring = rings
        .into_iter()
        .max_by_key(|r| r.as_array().map_or(0, |a| a.len()))?;
    let (mut min_lat, mut max_lat, mut min_lon, mut max_lon) =
        (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for p in ring.as_array()? {
        let (Some(lon), Some(lat)) = (
            p.get(0).and_then(|v| v.as_f64()),
            p.get(1).and_then(|v| v.as_f64()),
        ) else {
            continue;
        };
        if lat.abs() > 90.0 || lon.abs() > 180.0 {
            continue;
        }
        min_lat = min_lat.min(lat);
        max_lat = max_lat.max(lat);
        min_lon = min_lon.min(lon);
        max_lon = max_lon.max(lon);
    }
    (min_lat <= max_lat).then_some(((min_lat + max_lat) / 2.0, (min_lon + max_lon) / 2.0))
}

/// Brazilian cities exist both in the world populated-places set and in the IBGE municipality set;
/// keep only the most prominent entry (lowest rank) per name and ~0.5° cell.
pub fn dedupe_city_labels(labels: &mut Vec<MapLabel>) {
    let mut best: HashMap<(String, i32, i32), usize> = HashMap::new();
    let mut drop = vec![false; labels.len()];
    for (i, l) in labels.iter().enumerate() {
        if l.kind != LabelKind::City {
            continue;
        }
        for (dla, dlo) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
            let key = (
                fold_name(&l.pt),
                (l.lat * 2.0).floor() as i32 + dla,
                (l.lon * 2.0).floor() as i32 + dlo,
            );
            if let Some(&j) = best.get(&key) {
                if labels[j].rank <= l.rank {
                    drop[i] = true;
                } else {
                    drop[j] = true;
                }
                break;
            }
        }
        if !drop[i] {
            best.insert(
                (
                    fold_name(&l.pt),
                    (l.lat * 2.0).floor() as i32,
                    (l.lon * 2.0).floor() as i32,
                ),
                i,
            );
        }
    }
    let mut it = drop.into_iter();
    labels.retain(|_| !it.next().unwrap_or(false));
}

// Recursively extracts GeoJSON coordinate sequences. GeoJSON order is [longitude, latitude].
pub(super) fn collect_geojson_lines(value: &serde_json::Value, out: &mut Vec<Vec<(f64, f64)>>) {
    let Some(items) = value.as_array() else {
        return;
    };
    let is_line = items
        .first()
        .and_then(|v| v.as_array())
        .is_some_and(|pair| pair.len() >= 2 && pair[0].is_number() && pair[1].is_number());
    if is_line {
        let line: Vec<(f64, f64)> = items
            .iter()
            .filter_map(|pair| {
                let p = pair.as_array()?;
                let lon = p.first()?.as_f64()?;
                let lat = p.get(1)?.as_f64()?;
                (lat.is_finite() && lon.is_finite() && lat.abs() <= 90.0 && lon.abs() <= 180.0)
                    .then_some((lat, lon))
            })
            .collect();
        if line.len() >= 2 {
            out.push(line);
        }
    } else {
        for child in items {
            collect_geojson_lines(child, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn geojson_nested_rings_keep_latitude_longitude_order() {
        let value = json!([[[10.0, 20.0], [11.0, 21.0], [181.0, 30.0]], [[0.0, 0.0]]]);
        let mut lines = Vec::new();
        collect_geojson_lines(&value, &mut lines);
        assert_eq!(lines, vec![vec![(20.0, 10.0), (21.0, 11.0)]]);
    }

    #[test]
    fn shared_municipal_edges_are_removed_in_either_direction() {
        let lines = [
            vec![(0.0, 0.0), (0.0, 1.0), (1.0, 1.0)],
            vec![(1.0, 1.0), (0.0, 1.0), (0.0, 2.0)],
        ];
        let outline = outer_outline(lines.iter());
        assert_eq!(outline.len(), 2);
        assert!(outline.contains(&vec![(0.0, 0.0), (0.0, 1.0)]));
        assert!(outline.contains(&vec![(0.0, 1.0), (0.0, 2.0)]));
    }

    #[test]
    fn duplicate_city_labels_keep_the_more_prominent_entry() {
        let city = |name: &str, rank| MapLabel {
            lat: -23.5,
            lon: -46.6,
            en: name.into(),
            pt: name.into(),
            kind: LabelKind::City,
            rank,
            big: false,
        };
        let mut labels = vec![city("São Paulo", 9), city("Sao Paulo", 1)];
        dedupe_city_labels(&mut labels);
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].rank, 1);
    }
}
