use crate::math::{ll_to_v3, v3_to_ll, wrap_lon, DVec3};
use crate::{
    EARTH_RADIUS_KM, MAX_CAMERA_ALTITUDE_KM, MIN_CAMERA_ALTITUDE_KM, VERTICAL_FOV_HALF_TAN,
};
use crate::{MAX_CAMERA_LATITUDE_DEG, METERS_PER_KM, METERS_PER_LATITUDE_DEGREE};
use emath::{Pos2, Rect};
use glam::{DMat2, DMat3, DQuat, DVec2};
// Numerical tolerances and pole protection retain the original camera behavior.
const PROJECTION_DEPTH_EPSILON: f64 = 1e-9;
const ROTATION_AXIS_EPSILON: f64 = 1e-13;
const MIN_PAN_LATITUDE_COSINE: f64 = 0.05;

// ───────────────────────────── Camera ─────────────────────────────

#[derive(Clone, Copy, Debug)]
pub struct Cam {
    pub lat: f64,
    pub lon: f64,
    pub heading: f64, // degrees: bearing the top of the screen points to
    pub alt: f64,     // km above the surface
}

#[derive(Clone, Copy)]
pub struct Basis {
    pub c: DVec3, // camera position (Earth radii)
    orientation: DMat3,
    aspect: f64,
}

impl Cam {
    pub fn basis(&self, aspect: f64) -> Basis {
        let n = ll_to_v3(self.lat, self.lon);
        let (la, lo) = (self.lat.to_radians(), self.lon.to_radians());
        let east = DVec3::new(-lo.sin(), lo.cos(), 0.0);
        let north = DVec3::new(-la.sin() * lo.cos(), -la.sin() * lo.sin(), la.cos());
        let h = self.heading.to_radians();
        let u = north * h.cos() + east * h.sin();
        let f = n * -1.0;
        let r = f.cross(u);
        Basis {
            c: n * (1.0 + self.alt / EARTH_RADIUS_KM),
            orientation: DMat3::from_cols(r, u, f),
            aspect,
        }
    }
}

impl Basis {
    pub fn dir(&self, nx: f64, ny: f64) -> DVec3 {
        let view_ray = DVec3::new(
            nx * self.aspect * VERTICAL_FOV_HALF_TAN,
            ny * VERTICAL_FOV_HALF_TAN,
            1.0,
        );
        (self.orientation * view_ray).normalize()
    }
    pub fn project(&self, p: DVec3, rect: Rect) -> Option<Pos2> {
        if p.dot(self.c) < 1.0 {
            return None; // behind the horizon
        }
        let v = p - self.c;
        let local = self.orientation.transpose() * v;
        let z = local.z;
        if z <= PROJECTION_DEPTH_EPSILON {
            return None;
        }
        let nx = local.x / (z * self.aspect * VERTICAL_FOV_HALF_TAN);
        let ny = local.y / (z * VERTICAL_FOV_HALF_TAN);
        Some(Pos2::new(
            rect.center().x + nx as f32 * rect.width() * 0.5,
            rect.center().y - ny as f32 * rect.height() * 0.5,
        ))
    }
    pub fn pick(&self, rect: Rect, pos: Pos2) -> Option<(f64, f64)> {
        let nx = ((pos.x - rect.min.x) / rect.width()) as f64 * 2.0 - 1.0;
        let ny = 1.0 - ((pos.y - rect.min.y) / rect.height()) as f64 * 2.0;
        let d = self.dir(nx, ny);
        let b = self.c.dot(d);
        let disc = b * b - (self.c.dot(self.c) - 1.0);
        if disc < 0.0 || b > 0.0 {
            return None;
        }
        let t = -b - disc.sqrt();
        Some(v3_to_ll(self.c + d * t))
    }
}

impl Cam {
    fn deg_per_pt(&self, height_pts: f32) -> f64 {
        let mpp =
            self.alt * METERS_PER_KM * 2.0 * VERTICAL_FOV_HALF_TAN / height_pts.max(1.0) as f64;
        mpp / METERS_PER_LATITUDE_DEGREE
    }

    pub fn pan(&mut self, dx: f64, dy: f64, height_pts: f32) {
        let k = self.deg_per_pt(height_pts);
        let h = self.heading.to_radians();
        let movement = DMat2::from_angle(-h) * DVec2::new(-dx * k, dy * k);
        let (de, dn) = (movement.x, movement.y);
        self.lat = (self.lat + dn).clamp(-MAX_CAMERA_LATITUDE_DEG, MAX_CAMERA_LATITUDE_DEG);
        self.lon =
            wrap_lon(self.lon + de / self.lat.to_radians().cos().max(MIN_PAN_LATITUDE_COSINE));
    }

    pub fn zoom(&mut self, factor: f64) {
        self.alt = (self.alt * factor).clamp(MIN_CAMERA_ALTITUDE_KM, MAX_CAMERA_ALTITUDE_KM);
    }

    /// Rigidly rotates the camera (centre and "up" direction) by the rotation that takes `from` to `to`.
    fn rotate(&mut self, from: DVec3, to: DVec3) {
        let axis = from.cross(to);
        let sn = axis.dot(axis).sqrt();
        if sn < ROTATION_AXIS_EPSILON {
            return;
        }
        let ang = sn.atan2(from.dot(to));
        let k = axis * (1.0 / sn);
        let rotation = DQuat::from_axis_angle(k, ang);
        let n = ll_to_v3(self.lat, self.lon);
        let (la, lo) = (self.lat.to_radians(), self.lon.to_radians());
        let east = DVec3::new(-lo.sin(), lo.cos(), 0.0);
        let north = DVec3::new(-la.sin() * lo.cos(), -la.sin() * lo.sin(), la.cos());
        let h = self.heading.to_radians();
        let up = north * h.cos() + east * h.sin();
        let (n2, u2) = ((rotation * n).normalize(), rotation * up);
        let (lat2, lon2) = v3_to_ll(n2);
        let (la2, lo2) = (lat2.to_radians(), lon2.to_radians());
        let east2 = DVec3::new(-lo2.sin(), lo2.cos(), 0.0);
        let north2 = DVec3::new(-la2.sin() * lo2.cos(), -la2.sin() * lo2.sin(), la2.cos());
        self.lat = lat2.clamp(-MAX_CAMERA_LATITUDE_DEG, MAX_CAMERA_LATITUDE_DEG);
        self.lon = wrap_lon(lon2);
        self.heading = wrap_lon(u2.dot(east2).atan2(u2.dot(north2)).to_degrees());
    }

    /// Zooms keeping the geographic point under `cursor` fixed on screen (like Google Earth).
    pub fn zoom_at(&mut self, factor: f64, cursor: Option<Pos2>, rect: Rect, aspect: f64) {
        let before = cursor.and_then(|c| self.basis(aspect).pick(rect, c));
        self.zoom(factor);
        if let (Some(p0), Some(c)) = (before, cursor) {
            if let Some(p1) = self.basis(aspect).pick(rect, c) {
                self.rotate(ll_to_v3(p1.0, p1.1), ll_to_v3(p0.0, p0.1));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::haversine_km;
    use emath::Vec2;

    fn test_app_cam(cam: Cam) -> (Cam, Rect, f64) {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0));
        (cam, rect, 1200.0 / 800.0)
    }

    #[test]
    fn zoom_keeps_point_under_cursor() {
        for (lat, lon, hd, alt) in [
            (-15.0, -50.0, 0.0, 14_000.0),
            (40.0, 10.0, 70.0, 3000.0),
            (-60.0, 120.0, -130.0, 800.0),
        ] {
            let (mut cam, rect, aspect) = test_app_cam(Cam {
                lat,
                lon,
                heading: hd,
                alt,
            });
            for cursor in [
                Pos2::new(300.0, 250.0),
                Pos2::new(900.0, 600.0),
                Pos2::new(610.0, 395.0),
            ] {
                let Some(before) = cam.basis(aspect).pick(rect, cursor) else {
                    continue;
                };
                for f in [0.7, 0.5, 1.3] {
                    cam.zoom_at(f, Some(cursor), rect, aspect);
                    let after = cam.basis(aspect).pick(rect, cursor).unwrap();
                    assert!(
                        haversine_km(before, after) < 0.01,
                        "moved {} km",
                        haversine_km(before, after)
                    );
                    // the point must stay the same one after repeated zooms too
                    let again = before;
                    assert!(haversine_km(again, after) < 0.01);
                }
            }
        }
    }
}

#[cfg(test)]
mod bounds_tests {
    use super::*;

    #[test]
    fn zoom_clamps_altitude_without_a_surface_pick() {
        let mut cam = Cam {
            lat: 10.0,
            lon: 20.0,
            heading: 30.0,
            alt: 500.0,
        };
        let rect = Rect::from_min_max(Pos2::ZERO, Pos2::new(800.0, 600.0));
        cam.zoom_at(1e10, None, rect, 4.0 / 3.0);
        assert_eq!(cam.alt, MAX_CAMERA_ALTITUDE_KM);
        cam.zoom_at(1e-10, None, rect, 4.0 / 3.0);
        assert_eq!(cam.alt, MIN_CAMERA_ALTITUDE_KM);
        assert_eq!((cam.lat, cam.lon, cam.heading), (10.0, 20.0, 30.0));
    }

    #[test]
    fn pan_preserves_camera_bounds_near_poles_and_dateline() {
        let mut cam = Cam {
            lat: 89.0,
            lon: 179.0,
            heading: 0.0,
            alt: 1000.0,
        };
        cam.pan(-10000.0, 10000.0, 600.0);
        assert_eq!(cam.lat, 89.9);
        assert!((-180.0..180.0).contains(&cam.lon));
        assert_eq!(cam.alt, 1000.0);
    }

    #[test]
    fn screen_center_picks_camera_location() {
        let rect = Rect::from_min_max(Pos2::ZERO, Pos2::new(800.0, 600.0));
        let cam = Cam {
            lat: -15.0,
            lon: -50.0,
            heading: 70.0,
            alt: 14000.0,
        };
        let point = cam.basis(4.0 / 3.0).pick(rect, rect.center()).unwrap();
        assert!(crate::math::haversine_km(point, (cam.lat, cam.lon)) < 1e-6);
    }
}
