use super::App;
use earthcraft_core::{
    camera::Basis,
    math::{ll_to_v3, now_unix, wrap_lon},
};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Stroke, Vec2};
use std::time::Duration;

const LIVE_LAYER_REPAINT_INTERVAL: Duration = Duration::from_millis(50);
const FLIGHT_LABEL_ALTITUDE_MAX_KM: f64 = 8_000.0;

#[derive(Clone, Copy)]
struct FlightRoute {
    code: &'static str,
    from: (f64, f64),
    to: (f64, f64),
    period_secs: f64,
}

const FLIGHT_ROUTES: &[FlightRoute] = &[
    FlightRoute { code: "AZU2410", from: (-23.55, -46.64), to: (-22.91, -43.17), period_secs: 2400.0 },
    FlightRoute { code: "GLO2056", from: (-22.81, -43.25), to: (-12.97, -38.50), period_secs: 4300.0 },
    FlightRoute { code: "LAT3401", from: (-23.43, -46.47), to: (-30.03, -51.23), period_secs: 5000.0 },
    FlightRoute { code: "AFR459", from: (48.86, 2.35), to: (-23.55, -46.64), period_secs: 9000.0 },
    FlightRoute { code: "BAW247", from: (51.47, -0.45), to: (-22.91, -43.17), period_secs: 8600.0 },
    FlightRoute { code: "DAL105", from: (33.64, -84.42), to: (-23.43, -46.47), period_secs: 9500.0 },
    FlightRoute { code: "UAL905", from: (40.64, -73.78), to: (51.47, -0.45), period_secs: 4200.0 },
    FlightRoute { code: "AAL170", from: (25.79, -80.29), to: (40.64, -73.78), period_secs: 3800.0 },
    FlightRoute { code: "QFA12", from: (-33.94, 151.18), to: (34.05, -118.24), period_secs: 10500.0 },
    FlightRoute { code: "JAL701", from: (35.55, 139.78), to: (1.36, 103.99), period_secs: 5100.0 },
    FlightRoute { code: "UAE261", from: (25.25, 55.36), to: (51.47, -0.45), period_secs: 4700.0 },
    FlightRoute { code: "ETH506", from: (8.98, 38.79), to: (-26.13, 28.23), period_secs: 4200.0 },
    FlightRoute { code: "SAA223", from: (-26.13, 28.23), to: (-33.94, 18.60), period_secs: 2600.0 },
    FlightRoute { code: "ACA901", from: (43.68, -79.63), to: (49.19, -123.18), period_secs: 5300.0 },
    FlightRoute { code: "ANZ32", from: (-36.84, 174.76), to: (-33.94, 151.18), period_secs: 2400.0 },
    FlightRoute { code: "TAP074", from: (38.77, -9.13), to: (-23.55, -46.64), period_secs: 8800.0 },
];

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
        for route in FLIGHT_ROUTES {
            let progress = (t / route.period_secs).fract();
            let pos = route_position(*route, progress);
            let next = route_position(*route, (progress + 0.002).fract());
            let Some(screen) = basis.project(ll_to_v3(pos.0, pos.1), rect) else {
                continue;
            };
            if !rect.contains(screen) {
                continue;
            }
            let heading = basis
                .project(ll_to_v3(next.0, next.1), rect)
                .map(|n| (n.y - screen.y).atan2(n.x - screen.x))
                .unwrap_or(0.0);
            let icon = airplane_shape(screen, heading, 9.0 * ms);
            painter.add(egui::Shape::convex_polygon(
                icon,
                Color32::from_rgb(0, 255, 170),
                Stroke::new(1.0_f32, Color32::BLACK),
            ));
            let trail = flight_trail(screen, heading, 14.0 * ms);
            painter.line_segment(
                trail,
                Stroke::new(1.4_f32, Color32::from_rgba_unmultiplied(0, 255, 170, 120)),
            );

            if altitude_km <= FLIGHT_LABEL_ALTITUDE_MAX_KM {
                let label = format!("{}  FL{}", route.code, 300 + ((progress * 80.0) as i32));
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

fn route_position(route: FlightRoute, t: f64) -> (f64, f64) {
    let lat = route.from.0 + (route.to.0 - route.from.0) * ease_in_out(t);
    let delta_lon = wrap_lon(route.to.1 - route.from.1);
    let lon = wrap_lon(route.from.1 + delta_lon * ease_in_out(t));
    (lat, lon)
}

fn ease_in_out(t: f64) -> f64 {
    0.5 - (std::f64::consts::PI * t).cos() * 0.5
}

fn airplane_shape(center: Pos2, angle: f32, size: f32) -> Vec<Pos2> {
    let forward = Vec2::angled(angle) * size;
    let side = Vec2::new(-forward.y, forward.x) * 0.33;
    vec![
        center + forward,
        center - forward * 0.30 + side,
        center - forward * 0.15,
        center - forward * 0.30 - side,
    ]
}

fn flight_trail(center: Pos2, angle: f32, size: f32) -> [Pos2; 2] {
    let back = Vec2::angled(angle) * size;
    [center - back * 0.35, center - back * 1.2]
}

