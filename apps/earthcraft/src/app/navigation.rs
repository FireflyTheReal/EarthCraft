use super::App;
use super::Fly;
use earthcraft_core::MAX_CAMERA_LATITUDE_DEG;
use earthcraft_core::{
    camera::Cam,
    language::Lang,
    math::{haversine_km, wrap_lon},
    MAX_CAMERA_ALTITUDE_KM, MIN_CAMERA_ALTITUDE_KM,
};
use earthcraft_data::settings::save_lang;
use eframe::egui::{Pos2, Rect};
use std::{f64::consts::PI, time::Instant};
const FLY_ARC_ALTITUDE_RATIO: f64 = 0.7;
const FLY_DISTANCE_SCALE_KM: f64 = 8000.0;
const FLY_DURATION_SCALE_SECS: f64 = 2.0;
const MIN_FLY_DURATION_SECS: f64 = 1.0;
const MAX_FLY_DURATION_SECS: f64 = 3.5;

impl App {
    pub(super) fn set_lang(&mut self, l: Lang) {
        self.lang = l;
        save_lang(l);
        self.status = l
            .pick(
                "Ready. Drag to rotate, scroll to zoom.",
                "Pronto. Arraste para girar, role para aproximar.",
            )
            .into();
    }

    pub(super) fn touch(&mut self) {
        self.last_interact = Instant::now();
    }

    pub(super) fn fly_to(&mut self, lat: f64, lon: f64, alt: f64) {
        let a = self.cam;
        let b = Cam {
            lat: lat.clamp(-MAX_CAMERA_LATITUDE_DEG, MAX_CAMERA_LATITUDE_DEG),
            lon: wrap_lon(lon),
            heading: 0.0,
            alt: alt.clamp(MIN_CAMERA_ALTITUDE_KM, MAX_CAMERA_ALTITUDE_KM),
        };
        let dist = haversine_km((a.lat, a.lon), (b.lat, b.lon));
        let bump = if dist * FLY_ARC_ALTITUDE_RATIO > a.alt.max(b.alt) {
            (dist * FLY_ARC_ALTITUDE_RATIO)
                .min(MAX_CAMERA_ALTITUDE_KM)
                .ln()
                - (a.alt.ln() + b.alt.ln()) / 2.0
        } else {
            0.0
        };
        let dur = (MIN_FLY_DURATION_SECS + dist / FLY_DISTANCE_SCALE_KM * FLY_DURATION_SCALE_SECS)
            .clamp(MIN_FLY_DURATION_SECS, MAX_FLY_DURATION_SECS);
        self.fly = Some(Fly {
            a,
            b,
            t: 0.0,
            dur,
            bump,
        });
    }

    pub(super) fn step_fly(&mut self, dt: f64) {
        if let Some(mut f) = self.fly.take() {
            f.t += dt / f.dur;
            let t = f.t.min(1.0);
            let s = t * t * (3.0 - 2.0 * t);
            let (a, b) = (f.a, f.b);
            let (la, lb) = (a.alt.ln(), b.alt.ln());
            self.cam = Cam {
                lat: a.lat + (b.lat - a.lat) * s,
                lon: wrap_lon(a.lon + wrap_lon(b.lon - a.lon) * s),
                heading: a.heading + wrap_lon(b.heading - a.heading) * s,
                alt: (la + (lb - la) * s + (PI * t).sin() * f.bump).exp(),
            };
            self.touch();
            if f.t >= 1.0 {
                self.cam = b;
            } else {
                self.fly = Some(f);
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
