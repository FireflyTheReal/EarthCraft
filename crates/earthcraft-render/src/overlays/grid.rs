//! Bounded label candidates for the raster's latitude/longitude graticule.
use earthcraft_core::{
    camera::Basis,
    math::{ll_to_v3, v3_to_ll, wrap_lon},
};
use emath::{Pos2, Rect};
const VIEW_SAMPLES: usize = 16;
const LINE_SAMPLES: usize = 96;
const MAX_LINES_PER_AXIS: usize = 64;
const LABEL_INSET_POINTS: f32 = 18.0;
const DUPLICATE_STEP_RATIO: f64 = 0.01;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GridAxis {
    Latitude,
    Longitude,
}
#[derive(Clone, Copy, Debug)]
pub struct GridLabel {
    pub position: Pos2,
    pub degrees: f64,
    pub axis: GridAxis,
}

/// Uses the displayed raster's exact grid step. Longitudes are unwrapped about
/// its center so a dateline view cannot expand into a full-world scan.
pub fn project_grid_labels(basis: &Basis, rect: Rect, step_deg: f64) -> Vec<GridLabel> {
    if !step_deg.is_finite()
        || !(crate::MIN_GRID_STEP_DEG..=180.0).contains(&step_deg)
        || rect.width() <= LABEL_INSET_POINTS * 2.0
        || rect.height() <= LABEL_INSET_POINTS * 2.0
    {
        return vec![];
    }
    let center_lon = v3_to_ll(basis.c.normalize()).1;
    let mut lat_min = f64::INFINITY;
    let mut lat_max = f64::NEG_INFINITY;
    let mut lon_min = f64::INFINITY;
    let mut lon_max = f64::NEG_INFINITY;
    for row in 0..=VIEW_SAMPLES {
        for col in 0..=VIEW_SAMPLES {
            let pos = Pos2::new(
                rect.left() + rect.width() * col as f32 / VIEW_SAMPLES as f32,
                rect.top() + rect.height() * row as f32 / VIEW_SAMPLES as f32,
            );
            if let Some((lat, lon)) = basis.pick(rect, pos) {
                let lon = center_lon + wrap_lon(lon - center_lon);
                lat_min = lat_min.min(lat);
                lat_max = lat_max.max(lat);
                lon_min = lon_min.min(lon);
                lon_max = lon_max.max(lon);
            }
        }
    }
    if !lat_min.is_finite() {
        return vec![];
    }
    lat_min = (lat_min - step_deg).max(-90.0);
    lat_max = (lat_max + step_deg).min(90.0);
    lon_min = (lon_min - step_deg).max(center_lon - 180.0);
    lon_max = (lon_max + step_deg).min(center_lon + 180.0);
    let inside = rect.shrink(LABEL_INSET_POINTS);
    let mut labels = Vec::new();
    for (axis, min, max) in [
        (GridAxis::Latitude, lat_min, lat_max),
        (GridAxis::Longitude, lon_min, lon_max),
    ] {
        let first = (min / step_deg).ceil() as i64;
        let last = (max / step_deg).floor() as i64;
        let count = (last - first + 1).max(0) as usize;
        // If a polar view spans many meridians, retain a stable subset of grid lines.
        let stride = count.div_ceil(MAX_LINES_PER_AXIS).max(1);
        for index in (first..=last).step_by(stride) {
            let degrees = index as f64 * step_deg;
            if axis == GridAxis::Latitude && degrees.abs() >= 90.0 {
                continue;
            }
            let mut best: Option<(f32, Pos2)> = None;
            for sample in 0..=LINE_SAMPLES {
                let t = sample as f64 / LINE_SAMPLES as f64;
                let (lat, lon) = match axis {
                    GridAxis::Latitude => (degrees, lon_min + (lon_max - lon_min) * t),
                    GridAxis::Longitude => (lat_min + (lat_max - lat_min) * t, degrees),
                };
                if let Some(pos) = basis
                    .project(ll_to_v3(lat, wrap_lon(lon)), rect)
                    .filter(|pos| inside.contains(*pos))
                {
                    let distance = (pos.x - inside.left())
                        .min(inside.right() - pos.x)
                        .min(pos.y - inside.top())
                        .min(inside.bottom() - pos.y);
                    if best.is_none_or(|(old, _)| distance < old) {
                        best = Some((distance, pos));
                    }
                }
            }
            if let Some((_, position)) = best {
                let degrees = if axis == GridAxis::Longitude {
                    wrap_lon(degrees)
                } else {
                    degrees
                };
                if !labels.iter().any(|label: &GridLabel| {
                    label.axis == axis
                        && (label.degrees - degrees).abs() < step_deg * DUPLICATE_STEP_RATIO
                }) {
                    labels.push(GridLabel {
                        position,
                        degrees,
                        axis,
                    });
                }
            }
        }
    }
    labels
}

#[cfg(test)]
mod tests {
    use super::*;
    use earthcraft_core::camera::Cam;
    #[test]
    fn labels_follow_visible_grid_lines_across_headings_dateline_and_poles() {
        let rect = Rect::from_min_size(Pos2::ZERO, emath::vec2(800.0, 600.0));
        for (lat, lon, heading, alt) in [
            (0.0, 0.0, 0.0, 14000.0),
            (0.0, 179.0, 90.0, 500.0),
            (89.0, 30.0, 135.0, 100.0),
            (37.0, -122.0, 45.0, 0.15),
        ] {
            let basis = Cam {
                lat,
                lon,
                heading,
                alt,
            }
            .basis(800.0 / 600.0);
            let step = crate::grid_step(alt, 800, 600);
            let labels = project_grid_labels(&basis, rect, step);
            assert!(!labels.is_empty(), "{lat} {lon} {alt}");
            assert!(labels.len() <= MAX_LINES_PER_AXIS * 2);
            for label in labels {
                assert!(rect.contains(label.position));
                let picked = basis.pick(rect, label.position).unwrap();
                let error = match label.axis {
                    GridAxis::Latitude => (picked.0 - label.degrees).abs(),
                    GridAxis::Longitude => wrap_lon(picked.1 - label.degrees).abs(),
                };
                // Projection stores screen positions as f32; polar longitude amplifies rounding.
                assert!(error < step * 0.02, "{label:?} error {error} step {step}");
            }
        }
        for step in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(project_grid_labels(
                &Cam {
                    lat: 0.0,
                    lon: 0.0,
                    heading: 0.0,
                    alt: 100.0
                }
                .basis(1.0),
                rect,
                step
            )
            .is_empty());
        }
    }
}
