use super::App;
use earthcraft_core::{
    camera::Basis,
    math::{fmt_dist, great_circle, haversine_km, ll_to_v3, wrap_lon},
};
use earthcraft_data::boundaries::{
    dedupe_city_labels, LabelKind, Layer, MapLabel, Patch, OTHER_STATES_MAX_ALTITUDE_KM,
};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Stroke, Vec2};
const SIMPLIFY_BORDER_ALTITUDE_KM: f64 = 1500.0;
const MIN_BORDER_SEGMENT_POINTS: f32 = 1.4;
const MAX_BORDER_SUBDIVISIONS: usize = 96;
const CLOSE_BORDER_ALTITUDE_KM: f64 = 500.0;
const MEDIUM_BORDER_ALTITUDE_KM: f64 = 5000.0;
const MAX_VISIBLE_LABELS: usize = 500;
const RULER_ARC_SEGMENTS: usize = 48;

impl App {
    pub(super) fn merge_patch(&mut self, patch: Patch) {
        let b = &mut self.boundaries;
        match patch.layer {
            Layer::Countries => b.countries.extend(patch.lines),
            Layer::States => {
                b.states.extend(patch.lines);
                b.states_big.extend(patch.lines_big);
            }
            Layer::Cities => b.cities.extend(patch.lines),
            Layer::Metro => b.metro.extend(patch.lines),
        }
        b.labels.extend(patch.labels);
        dedupe_city_labels(&mut b.labels);
        self.last_state = 0; // redraw overlays immediately
    }

    pub(super) fn draw_overlays(&self, painter: &egui::Painter, basis: &Basis, rect: Rect) {
        // Administrative boundaries are drawn as a separate overlay, independent of imagery.
        // Densify long geographic edges at close zoom. Many admin-1 polygons have very
        // sparse vertices; projecting only the endpoints makes their borders disappear
        // whenever an endpoint falls behind the globe's horizon. Sampling the edge keeps
        // the visible middle portions drawable, including small countries at ~500 km.
        // Far away, consecutive vertices fall on the same pixel; skipping those keeps the many
        // thousands of segments of the world borders cheap to draw.
        let min_sq: f32 = if self.cam.alt > SIMPLIFY_BORDER_ALTITUDE_KM {
            MIN_BORDER_SEGMENT_POINTS * MIN_BORDER_SEGMENT_POINTS
        } else {
            0.0
        };
        let draw_lines =
            |lines: &[Vec<(f64, f64)>], color: Color32, width: f32, max_step_deg: f64| {
                let stroke = Stroke::new(width, color);
                for line in lines {
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
                                painter.line_segment([from, b], stroke);
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
                        let steps = ((span / max_step_deg).ceil() as usize)
                            .clamp(1, MAX_BORDER_SUBDIVISIONS);
                        let mut prev = start;
                        for i in 1..=steps {
                            let t = i as f64 / steps as f64;
                            let lon = wrap_lon(lon1 + dlon * t);
                            let lat = lat1 + dlat * t;
                            let cur = basis.project(ll_to_v3(lat, lon), rect);
                            if let (Some(a), Some(b)) = (prev, cur) {
                                painter.line_segment([a, b], stroke);
                            }
                            prev = cur;
                        }
                    }
                }
            };
        let close_zoom = self.cam.alt <= CLOSE_BORDER_ALTITUDE_KM;
        if self.show_country_boundaries {
            draw_lines(
                &self.boundaries.countries,
                Color32::from_rgba_unmultiplied(255, 235, 150, 235),
                1.15,
                if close_zoom { 0.25 } else { 2.0 },
            );
        }
        if self.show_state_boundaries {
            let step = if close_zoom {
                0.12
            } else if self.cam.alt <= MEDIUM_BORDER_ALTITUDE_KM {
                0.5
            } else {
                2.0
            };
            let col = Color32::from_rgba_unmultiplied(120, 220, 255, 220);
            // States of the largest countries: visible at every zoom level (whole Earth in view).
            draw_lines(&self.boundaries.states_big, col, 1.0, step);
            // States/provinces of the other countries: only from medium zoom on.
            if self.cam.alt < OTHER_STATES_MAX_ALTITUDE_KM {
                draw_lines(&self.boundaries.states, col, 1.0, step);
            }
        }
        if self.show_city_boundaries {
            draw_lines(
                &self.boundaries.cities,
                Color32::from_rgba_unmultiplied(255, 160, 80, 210),
                0.7,
                if close_zoom { 0.12 } else { 1.0 },
            );
        }
        if self.show_metro_boundaries {
            draw_lines(
                &self.boundaries.metro,
                Color32::from_rgba_unmultiplied(255, 100, 210, 235),
                1.5,
                if close_zoom { 0.12 } else { 1.0 },
            );
        }
        let ms = self.settings.map_scale;
        if self.show_place_labels {
            // Collect what is visible, then place by priority (countries > states > metro > cities)
            // skipping any name that would overlap one already drawn.
            let mut cands: Vec<(u8, u8, Pos2, &MapLabel)> = Vec::new();
            for label in &self.boundaries.labels {
                let enabled = label.visible_at(self.cam.alt)
                    && match label.kind {
                        LabelKind::Country | LabelKind::City => true,
                        LabelKind::State => self.show_state_boundaries,
                        LabelKind::Metro => self.show_metro_boundaries,
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
            let mut placed: Vec<Rect> = Vec::new();
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
                let text = label.text(self.lang);
                let galley = painter.layout_no_wrap(text.to_string(), font.clone(), color);
                let area = Rect::from_center_size(pos, galley.size()).expand(2.0);
                if placed.iter().any(|q| q.intersects(area)) {
                    continue;
                }
                placed.push(area);
                painter.text(
                    pos,
                    Align2::CENTER_CENTER,
                    text,
                    font.clone(),
                    Color32::from_black_alpha(180),
                );
                painter.text(
                    pos + Vec2::new(0.0, -0.5),
                    Align2::CENTER_CENTER,
                    text,
                    font,
                    color,
                );
            }
        }
        // ruler
        let stroke = Stroke::new(2.5_f32, Color32::from_rgb(255, 80, 80));
        for pair in self.ruler_pts.windows(2) {
            let pts: Vec<Option<Pos2>> = great_circle(pair[0], pair[1], RULER_ARC_SEGMENTS)
                .into_iter()
                .map(|p| basis.project(p, rect))
                .collect();
            for s in pts.windows(2) {
                if let (Some(a), Some(b)) = (s[0], s[1]) {
                    painter.line_segment([a, b], stroke);
                }
            }
            if let (Some(a), Some(b)) = (
                basis.project(ll_to_v3(pair[0].0, pair[0].1), rect),
                basis.project(ll_to_v3(pair[1].0, pair[1].1), rect),
            ) {
                let mid = Pos2::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
                let txt = fmt_dist(haversine_km(pair[0], pair[1]));
                painter.text(
                    mid + Vec2::new(1.0, 1.0),
                    Align2::CENTER_BOTTOM,
                    &txt,
                    FontId::proportional(13.0 * ms),
                    Color32::BLACK,
                );
                painter.text(
                    mid,
                    Align2::CENTER_BOTTOM,
                    &txt,
                    FontId::proportional(13.0 * ms),
                    Color32::WHITE,
                );
            }
        }
        for p in &self.ruler_pts {
            if let Some(s) = basis.project(ll_to_v3(p.0, p.1), rect) {
                painter.circle_filled(s, 4.0, Color32::WHITE);
                painter.circle_stroke(s, 4.0, Stroke::new(1.5_f32, Color32::from_rgb(255, 80, 80)));
            }
        }
        // placemarks
        for pm in &self.placemarks {
            if let Some(s) = basis.project(ll_to_v3(pm.lat, pm.lon), rect) {
                if rect.contains(s) {
                    painter.circle_filled(s, 6.0, Color32::from_rgb(255, 200, 0));
                    painter.circle_stroke(s, 6.0, Stroke::new(1.5_f32, Color32::BLACK));
                    let tp = s + Vec2::new(9.0, -9.0);
                    painter.text(
                        tp + Vec2::new(1.0, 1.0),
                        Align2::LEFT_BOTTOM,
                        &pm.name,
                        FontId::proportional(13.0 * ms),
                        Color32::BLACK,
                    );
                    painter.text(
                        tp,
                        Align2::LEFT_BOTTOM,
                        &pm.name,
                        FontId::proportional(13.0 * ms),
                        Color32::WHITE,
                    );
                }
            }
        }
    }
}
