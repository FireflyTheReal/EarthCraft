//! Cached administrative border projection and ordered geographic labels.
use super::{labels::LabelIndex, OverlayStyle, OverlayView};
use earthcraft_core::math::{ll_to_v3, wrap_lon};
use earthcraft_data::boundaries::{
    BoundaryData, LabelKind, MapLabel, OTHER_STATES_MAX_ALTITUDE_KM,
};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Shape, Stroke, Vec2};
const SIMPLIFY_BORDER_ALTITUDE_KM: f64 = 1500.0;
const MIN_BORDER_SEGMENT_POINTS: f32 = 1.4;
const MAX_BORDER_SUBDIVISIONS: usize = 96;
const CLOSE_BORDER_ALTITUDE_KM: f64 = 500.0;
const MEDIUM_BORDER_ALTITUDE_KM: f64 = 5000.0;
const MAX_VISIBLE_LABELS: usize = 500;
const LABEL_PADDING_POINTS: f32 = 2.0;
const LABEL_HIGHLIGHT_OFFSET_POINTS: f32 = -0.5;

#[cfg(test)]
pub(super) fn build(
    painter: &egui::Painter,
    view: OverlayView,
    boundaries: &BoundaryData,
    style: OverlayStyle,
) -> Vec<Shape> {
    build_indexed(painter, view, boundaries, style, None)
}

pub(super) fn build_indexed(
    painter: &egui::Painter,
    view: OverlayView,
    boundaries: &BoundaryData,
    style: OverlayStyle,
    visible: Option<&std::collections::HashSet<usize>>,
) -> Vec<Shape> {
    let (basis, rect) = (&view.basis, view.rect);
    let mut shapes = Vec::new();
    // Administrative boundaries are drawn as a separate overlay, independent of imagery.
    // Densify long geographic edges at close zoom. Many admin-1 polygons have very
    // sparse vertices; projecting only the endpoints makes their borders disappear
    // whenever an endpoint falls behind the globe's horizon. Sampling the edge keeps
    // the visible middle portions drawable, including small countries at ~500 km.
    // Far away, consecutive vertices fall on the same pixel; skipping those keeps the many
    // thousands of segments of the world borders cheap to draw.
    let min_sq: f32 = if view.altitude_km > SIMPLIFY_BORDER_ALTITUDE_KM {
        MIN_BORDER_SEGMENT_POINTS * MIN_BORDER_SEGMENT_POINTS
    } else {
        0.0
    };
    let mut draw_lines = |lines: &[earthcraft_data::boundaries::Line],
                          color: Color32,
                          width: f32,
                          max_step_deg: f64| {
        let stroke = Stroke::new(width, color);
        for line in lines {
            if visible.is_some_and(|set| !set.contains(&(std::sync::Arc::as_ptr(line) as usize))) {
                continue;
            }
            let mut anchor: Option<Pos2> = None;
            for pair in line.windows(2) {
                let (lat1, lon1) = pair[0];
                let (lat2, lon2) = pair[1];
                let dlat = lat2 - lat1;
                let mut dlon = lon2 - lon1;
                if dlon > 180.0 {
                    dlon -= 360.0;
                }
                if dlon < -180.0 {
                    dlon += 360.0;
                }
                let start = basis.project(ll_to_v3(lat1, lon1), rect);
                let end = basis.project(ll_to_v3(lat2, lon2), rect);
                if let (Some(a), Some(b)) = (start, end) {
                    // Common case: both endpoints are visible, so no extra sampling needed.
                    let from = anchor.unwrap_or(a);
                    if (b - from).length_sq() >= min_sq {
                        shapes.push(Shape::line_segment([from, b], stroke));
                        anchor = Some(b);
                    } else if anchor.is_none() {
                        anchor = Some(a);
                    }
                    continue;
                }
                anchor = None;
                let span = dlat.abs().max(dlon.abs());
                if span < max_step_deg {
                    continue;
                }
                let steps =
                    ((span / max_step_deg).ceil() as usize).clamp(1, MAX_BORDER_SUBDIVISIONS);
                let mut prev = start;
                for i in 1..=steps {
                    let t = i as f64 / steps as f64;
                    let lon = wrap_lon(lon1 + dlon * t);
                    let lat = lat1 + dlat * t;
                    let cur = basis.project(ll_to_v3(lat, lon), rect);
                    if let (Some(a), Some(b)) = (prev, cur) {
                        shapes.push(Shape::line_segment([a, b], stroke));
                    }
                    prev = cur;
                }
            }
        }
    };
    let close_zoom = view.altitude_km <= CLOSE_BORDER_ALTITUDE_KM;
    if style.layers.countries {
        draw_lines(
            &boundaries.countries,
            Color32::from_rgba_unmultiplied(255, 235, 150, 235),
            1.15,
            if close_zoom { 0.25 } else { 2.0 },
        );
    }
    if style.layers.states {
        let step = if close_zoom {
            0.12
        } else if view.altitude_km <= MEDIUM_BORDER_ALTITUDE_KM {
            0.5
        } else {
            2.0
        };
        let col = Color32::from_rgba_unmultiplied(120, 220, 255, 220);
        // States of the largest countries: visible at every zoom level (whole Earth in view).
        draw_lines(&boundaries.states_big, col, 1.0, step);
        // States/provinces of the other countries: only from medium zoom on.
        if view.altitude_km < OTHER_STATES_MAX_ALTITUDE_KM {
            draw_lines(&boundaries.states, col, 1.0, step);
        }
    }
    if style.layers.cities {
        draw_lines(
            &boundaries.cities,
            Color32::from_rgba_unmultiplied(255, 160, 80, 210),
            0.7,
            if close_zoom { 0.12 } else { 1.0 },
        );
    }
    if style.layers.metro {
        draw_lines(
            &boundaries.metro,
            Color32::from_rgba_unmultiplied(255, 100, 210, 235),
            1.5,
            if close_zoom { 0.12 } else { 1.0 },
        );
    }
    let ms = style.map_scale;
    if style.layers.labels {
        // Collect what is visible, then place by priority (countries > states > metro > cities)
        // skipping any name that would overlap one already drawn.
        let mut cands: Vec<(u8, u8, Pos2, &MapLabel)> = Vec::new();
        for label in &boundaries.labels {
            let enabled = label.visible_at(view.altitude_km)
                && match label.kind {
                    LabelKind::Country | LabelKind::City => true,
                    LabelKind::State => style.layers.states,
                    LabelKind::Metro => style.layers.metro,
                };
            if !enabled {
                continue;
            }
            if let Some(pos) = basis.project(ll_to_v3(label.lat, label.lon), rect) {
                if rect.contains(pos) {
                    let prio = match label.kind {
                        LabelKind::Country => 0,
                        LabelKind::State => 1,
                        LabelKind::Metro => 2,
                        LabelKind::City => 3,
                    };
                    cands.push((prio, label.rank, pos, label));
                }
            }
        }
        cands.sort_by_key(|c| (c.0, c.1));
        let mut placed = LabelIndex::default();
        for (_, _, pos, label) in cands {
            if placed.len() >= MAX_VISIBLE_LABELS {
                break;
            }
            let (color, size) = match label.kind {
                LabelKind::Country => (Color32::WHITE, 12.0),
                LabelKind::State => (Color32::from_rgb(180, 230, 255), 10.0),
                LabelKind::City => (Color32::from_rgb(255, 245, 210), 9.0),
                LabelKind::Metro => (Color32::from_rgb(255, 170, 230), 10.0),
            };
            let font = FontId::proportional(size * ms);
            let text = label.text(style.lang);
            let galley = painter.layout_no_wrap(text.to_string(), font.clone(), color);
            let area = Rect::from_center_size(pos, galley.size()).expand(LABEL_PADDING_POINTS);
            if !placed.try_insert(area) {
                continue;
            }
            if galley.is_empty() {
                continue;
            }
            shapes.push(painter.fonts(|fonts| {
                Shape::text(
                    fonts,
                    pos,
                    Align2::CENTER_CENTER,
                    text,
                    font.clone(),
                    Color32::from_black_alpha(180),
                )
            }));
            shapes.push(painter.fonts(|fonts| {
                Shape::text(
                    fonts,
                    pos + Vec2::new(0.0, LABEL_HIGHLIGHT_OFFSET_POINTS),
                    Align2::CENTER_CENTER,
                    text,
                    font,
                    color,
                )
            }));
        }
    }
    shapes
}
