use super::App;
use earthcraft_core::{
    camera::Basis,
    math::{fmt_dist, great_circle, haversine_km, ll_to_v3},
};
use earthcraft_data::boundaries::{dedupe_city_labels, Layer, Patch};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Stroke, Vec2};
#[cfg(test)]
mod benchmarks;
mod cache;
mod projected;
pub(super) use cache::OverlayCache;
use cache::{OverlayLayers, OverlayStyle, OverlayView};

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
        self.overlay_cache.invalidate();
        self.last_state = 0; // redraw overlays immediately
    }

    pub(super) fn draw_overlays(
        &mut self,
        painter: &egui::Painter,
        basis: &Basis,
        rect: Rect,
        altitude_km: f64,
    ) {
        let style = OverlayStyle {
            layers: OverlayLayers {
                countries: self.show_country_boundaries,
                states: self.show_state_boundaries,
                cities: self.show_city_boundaries,
                metro: self.show_metro_boundaries,
                labels: self.show_place_labels,
            },
            map_scale: self.settings.map_scale,
            lang: self.lang,
        };
        self.overlay_cache.draw(
            painter,
            OverlayView {
                basis: *basis,
                rect,
                altitude_km,
                pixels_per_point: painter.ctx().pixels_per_point(),
            },
            &self.boundaries,
            style,
        );
        let ms = self.settings.map_scale;
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
