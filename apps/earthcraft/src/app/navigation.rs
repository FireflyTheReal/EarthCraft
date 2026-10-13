use super::App;
use earthcraft_core::flight::Flight;
use earthcraft_core::language::Lang;
use eframe::egui::{Pos2, Rect};
use std::time::Instant;

impl App {
    pub(super) fn set_lang(&mut self, l: Lang) {
        self.lang = l;
        self.settings_writer.language(l);
        self.status = l.text("app-ready-status").into();
        // No private Explorer field access: keep user state intact on language changes.
    }

    pub(super) fn touch(&mut self) {
        self.last_interact = Instant::now();
    }

    pub(super) fn fly_to(&mut self, lat: f64, lon: f64, alt: f64) {
        self.fly = Some(Flight::new(self.cam, lat, lon, alt));
    }

    pub(super) fn step_fly(&mut self, dt: f64) {
        if let Some(mut flight) = self.fly.take() {
            self.cam = flight.advance(dt);
            self.touch();
            if !flight.is_finished() {
                self.fly = Some(flight);
            }
        }
    }

    pub(super) fn pan(&mut self, dx: f64, dy: f64, height_pts: f32) {
        self.cam.pan(dx, dy, height_pts);
    }
    pub(super) fn zoom(&mut self, factor: f64) {
        self.cam.zoom(factor);
    }
    pub(super) fn zoom_at(&mut self, factor: f64, cursor: Option<Pos2>, rect: Rect, aspect: f64) {
        self.cam.zoom_at(factor, cursor, rect, aspect);
    }
}
