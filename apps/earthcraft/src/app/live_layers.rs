use super::App;
use earthcraft_core::{camera::Basis, math::now_unix};
use eframe::egui::{self, Align2, Color32, FontId, Rect, Stroke, Vec2};
use std::time::Duration;

const LIVE_LAYER_REPAINT_INTERVAL: Duration = Duration::from_millis(50);
const FLIGHT_LABEL_ALTITUDE_MAX_KM: f64 = 8_000.0;

impl App {
    pub(super) fn draw_live_layers(
        &self,
        painter: &egui::Painter,
        basis: &Basis,
        rect: Rect,
        altitude_km: f64,
    ) {
        let t = now_unix();
        if self.show_flights {
            self.draw_flight_overlay(painter, basis, rect, altitude_km, t);
        }
        if self.show_flights {
            painter
                .ctx()
                .request_repaint_after(LIVE_LAYER_REPAINT_INTERVAL);
        }
    }

    fn draw_flight_overlay(
        &self,
        painter: &egui::Painter,
        basis: &Basis,
        rect: Rect,
        altitude_km: f64,
        t: f64,
    ) {
        let ms = self.settings.map_scale;
        for flight in earthcraft_render::aircraft::project_flights(basis, rect, t, ms) {
            let screen = flight.position;
            let icon = flight.icon.to_vec();
            painter.add(egui::Shape::convex_polygon(
                icon,
                Color32::from_rgb(0, 255, 170),
                Stroke::new(1.0_f32, Color32::BLACK),
            ));
            let trail = flight.trail;
            painter.line_segment(
                trail,
                Stroke::new(1.4_f32, Color32::from_rgba_unmultiplied(0, 255, 170, 120)),
            );

            if altitude_km <= FLIGHT_LABEL_ALTITUDE_MAX_KM {
                let label = format!("{}  FL{}", flight.code, flight.flight_level);
                let tp = screen + Vec2::new(10.0 * ms, -8.0 * ms);
                painter.text(
                    tp + Vec2::new(1.0, 1.0),
                    Align2::LEFT_BOTTOM,
                    &label,
                    FontId::proportional(12.0 * ms),
                    Color32::BLACK,
                );
                painter.text(
                    tp,
                    Align2::LEFT_BOTTOM,
                    &label,
                    FontId::proportional(12.0 * ms),
                    Color32::from_rgb(170, 255, 220),
                );
            }
        }
    }
}
