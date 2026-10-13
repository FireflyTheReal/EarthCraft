//! Camera flight interpolation, independent of UI timing and input.
use crate::{
    camera::Cam,
    math::{haversine_km, wrap_lon},
    MAX_CAMERA_ALTITUDE_KM, MAX_CAMERA_LATITUDE_DEG, MIN_CAMERA_ALTITUDE_KM,
};
use std::f64::consts::PI;
const FLY_ARC_ALTITUDE_RATIO: f64 = 0.7;
const FLY_DISTANCE_SCALE_KM: f64 = 8000.0;
const FLY_DURATION_SCALE_SECS: f64 = 2.0;
const MIN_FLY_DURATION_SECS: f64 = 1.0;
const MAX_FLY_DURATION_SECS: f64 = 3.5;

pub struct Flight {
    a: Cam,
    b: Cam,
    t: f64,
    dur: f64,
    bump: f64,
}
impl Flight {
    pub fn new(from: Cam, lat: f64, lon: f64, alt: f64) -> Self {
        let a = from;
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
        Self {
            a,
            b,
            t: 0.0,
            dur,
            bump,
        }
    }

    pub fn advance(&mut self, dt: f64) -> Cam {
        self.t += dt / self.dur;
        let t = self.t.min(1.0);
        let s = t * t * (3.0 - 2.0 * t);
        let (a, b) = (self.a, self.b);
        let (la, lb) = (a.alt.ln(), b.alt.ln());
        if self.is_finished() {
            return b;
        }
        Cam {
            lat: a.lat + (b.lat - a.lat) * s,
            lon: wrap_lon(a.lon + wrap_lon(b.lon - a.lon) * s),
            heading: a.heading + wrap_lon(b.heading - a.heading) * s,
            alt: (la + (lb - la) * s + (PI * t).sin() * self.bump).exp(),
        }
    }
    pub fn is_finished(&self) -> bool {
        self.t >= 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flight_crosses_dateline_by_short_path_and_finishes_at_clamped_destination() {
        let from = Cam {
            lat: 0.0,
            lon: 179.0,
            heading: 0.0,
            alt: 500.0,
        };
        let mut flight = Flight::new(from, 0.0, -179.0, 500.0);
        let halfway = flight.advance(flight.dur / 2.0);
        assert!((halfway.lon.abs() - 180.0).abs() < 1e-9);
        assert!(!flight.is_finished());
        let end = flight.advance(10.0);
        assert_eq!(end.lon, -179.0);
        assert_eq!(end.alt, 500.0);
        assert!(flight.is_finished());
        let mut bounds = Flight::new(from, 100.0, 540.0, 0.0);
        let end = bounds.advance(100.0);
        assert_eq!(end.lat, MAX_CAMERA_LATITUDE_DEG);
        assert_eq!(end.lon, -180.0);
        assert_eq!(end.alt, MIN_CAMERA_ALTITUDE_KM);
    }
}
