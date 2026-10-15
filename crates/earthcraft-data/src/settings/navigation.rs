//! Navigation sensitivity and a validated startup camera; no UI dependencies.
use earthcraft_core::{
    MAX_CAMERA_ALTITUDE_KM, MIN_CAMERA_ALTITUDE_KM, camera::Cam, coordinates::validate_coordinates,
};
use serde::Serialize;
pub const MIN_SENSITIVITY: f64 = 0.1;
pub const MAX_SENSITIVITY: f64 = 4.0;
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct StartupView {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude_km: f64,
    pub heading_deg: f64,
}
impl Default for StartupView {
    fn default() -> Self {
        Self {
            latitude: -15.0,
            longitude: -50.0,
            altitude_km: 14_000.0,
            heading_deg: 0.0,
        }
    }
}
impl StartupView {
    pub fn camera(self) -> Cam {
        Cam {
            lat: self.latitude,
            lon: self.longitude,
            alt: self.altitude_km,
            heading: self.heading_deg,
        }
    }
    pub fn from_camera(camera: Cam) -> Self {
        Self {
            latitude: camera.lat,
            longitude: camera.lon,
            altitude_km: camera.alt,
            heading_deg: camera.heading,
        }
    }
    fn apply(&mut self, value: &serde_json::Value) {
        let lat = value
            .get("latitude")
            .and_then(|v| v.as_f64())
            .unwrap_or(self.latitude);
        let lon = value
            .get("longitude")
            .and_then(|v| v.as_f64())
            .unwrap_or(self.longitude);
        if let Ok((latitude, longitude)) = validate_coordinates(lat, lon) {
            self.latitude = latitude.clamp(
                -earthcraft_core::MAX_CAMERA_LATITUDE_DEG,
                earthcraft_core::MAX_CAMERA_LATITUDE_DEG,
            );
            self.longitude = longitude;
        }
        if let Some(alt) = value
            .get("altitude_km")
            .and_then(|v| v.as_f64())
            .filter(|v| v.is_finite())
        {
            self.altitude_km = alt.clamp(MIN_CAMERA_ALTITUDE_KM, MAX_CAMERA_ALTITUDE_KM);
        }
        if let Some(heading) = value
            .get("heading_deg")
            .and_then(|v| v.as_f64())
            .filter(|v| v.is_finite())
        {
            self.heading_deg = earthcraft_core::math::wrap_lon(heading);
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NavigationPreferences {
    pub pan_sensitivity: f64,
    pub zoom_sensitivity: f64,
    pub rotation_sensitivity: f64,
    pub invert_scroll: bool,
    pub startup: StartupView,
}
impl Default for NavigationPreferences {
    fn default() -> Self {
        Self {
            pan_sensitivity: 1.0,
            zoom_sensitivity: 1.0,
            rotation_sensitivity: 1.0,
            invert_scroll: false,
            startup: StartupView::default(),
        }
    }
}
impl NavigationPreferences {
    pub(super) fn apply(&mut self, value: &serde_json::Value) {
        for (key, target) in [
            ("pan_sensitivity", &mut self.pan_sensitivity),
            ("zoom_sensitivity", &mut self.zoom_sensitivity),
            ("rotation_sensitivity", &mut self.rotation_sensitivity),
        ] {
            if let Some(number) = value
                .get(key)
                .and_then(|v| v.as_f64())
                .filter(|v| v.is_finite())
            {
                *target = number.clamp(MIN_SENSITIVITY, MAX_SENSITIVITY);
            }
        }
        if let Some(invert) = value.get("invert_scroll").and_then(|v| v.as_bool()) {
            self.invert_scroll = invert;
        }
        if let Some(startup) = value.get("startup") {
            self.startup.apply(startup);
        }
    }
}
