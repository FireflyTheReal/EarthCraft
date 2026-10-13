//! Independent f64 ray lanes; geographic/trigonometric sampling stays scalar.
use earthcraft_core::{camera::Basis, VERTICAL_FOV_HALF_TAN};
use glam::{DMat3, DVec3};
use wide::f64x4;

pub(super) const RAY_LANES: usize = 4;

pub(super) struct RayCaster {
    rotation: DMat3,
    aspect: f64,
    camera: DVec3,
    pub(super) camera_len_squared: f64,
    width: f64,
    height: f64,
}

pub(super) struct RaySample {
    pub(super) direction: DVec3,
    pub(super) camera_dot: f64,
    pub(super) discriminant: f64,
    pub(super) distance: f64,
    pub(super) point: DVec3,
    pub(super) incidence: f64,
}

impl RayCaster {
    pub(super) fn new(width: usize, height: usize, basis: &Basis) -> Self {
        Self {
            rotation: basis.rotation_from_view(),
            aspect: basis.aspect_ratio(),
            camera: basis.c,
            camera_len_squared: basis.c.length_squared(),
            width: width as f64,
            height: height as f64,
        }
    }

    pub(super) fn cast(&self, first_x: usize, y: usize) -> [RaySample; RAY_LANES] {
        let nx = f64x4::new(std::array::from_fn(|lane| (first_x + lane) as f64));
        let nx = (nx + 0.5) / self.width * 2.0 - 1.0;
        let ny = 1.0 - (y as f64 + 0.5) / self.height * 2.0;
        let vx = nx * self.aspect * VERTICAL_FOV_HALF_TAN;
        let vy = ny * VERTICAL_FOV_HALF_TAN;
        let m = self.rotation;
        // Preserve glam's column accumulation and scalar normalize order. No FMA
        // helpers, reciprocal approximations or reduced geographic precision.
        let dx = vx * m.x_axis.x + f64x4::splat(m.y_axis.x * vy) - m.z_axis.x;
        let dy = vx * m.x_axis.y + f64x4::splat(m.y_axis.y * vy) - m.z_axis.y;
        let dz = vx * m.x_axis.z + f64x4::splat(m.y_axis.z * vy) - m.z_axis.z;
        let reciprocal_length = f64x4::ONE / (dx * dx + dy * dy + dz * dz).sqrt();
        let (dx, dy, dz) = (
            dx * reciprocal_length,
            dy * reciprocal_length,
            dz * reciprocal_length,
        );
        let c = self.camera;
        let dot = dx * c.x + dy * c.y + dz * c.z;
        let disc = dot * dot - (self.camera_len_squared - 1.0);
        let surface_lanes = disc.simd_ge(f64x4::ZERO) & dot.simd_lt(f64x4::ZERO);
        let (distance, px, py, pz, incidence) = if surface_lanes.none() {
            (
                f64x4::ZERO,
                f64x4::ZERO,
                f64x4::ZERO,
                f64x4::ZERO,
                f64x4::ZERO,
            )
        } else {
            // Miss lanes may contain NaN here; their hit fields are never used.
            let distance = -dot - disc.sqrt();
            let px = f64x4::splat(c.x) + dx * distance;
            let py = f64x4::splat(c.y) + dy * distance;
            let pz = f64x4::splat(c.z) + dz * distance;
            let incidence = (-(dx * px + dy * py + dz * pz)).max(f64x4::ZERO);
            (distance, px, py, pz, incidence)
        };
        let (dx, dy, dz, dot, disc) = (
            dx.to_array(),
            dy.to_array(),
            dz.to_array(),
            dot.to_array(),
            disc.to_array(),
        );
        let (distance, px, py, pz, incidence) = (
            distance.to_array(),
            px.to_array(),
            py.to_array(),
            pz.to_array(),
            incidence.to_array(),
        );
        std::array::from_fn(|lane| RaySample {
            direction: DVec3::new(dx[lane], dy[lane], dz[lane]),
            camera_dot: dot[lane],
            discriminant: disc[lane],
            distance: distance[lane],
            point: DVec3::new(px[lane], py[lane], pz[lane]),
            incidence: incidence[lane],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use earthcraft_core::{camera::Cam, MAX_CAMERA_ALTITUDE_KM, MIN_CAMERA_ALTITUDE_KM};

    #[test]
    fn packed_rays_match_scalar_camera_directions_and_sphere_geometry() {
        for (width, height) in [(1, 7), (3, 9), (65, 49), (127, 95)] {
            for (lat, lon, heading) in [
                (-15.0, -50.0, 25.0),
                (89.9, 179.9, -130.0),
                (-89.9, -179.9, 180.0),
                (0.0, 180.0, 0.0),
            ] {
                for alt in [
                    MIN_CAMERA_ALTITUDE_KM,
                    1000.0,
                    14_000.0,
                    MAX_CAMERA_ALTITUDE_KM,
                ] {
                    let basis = Cam {
                        lat,
                        lon,
                        heading,
                        alt,
                    }
                    .basis(width as f64 / height as f64);
                    let caster = RayCaster::new(width, height, &basis);
                    for y in 0..height {
                        for first_x in (0..width).step_by(RAY_LANES) {
                            for (lane, ray) in caster
                                .cast(first_x, y)
                                .into_iter()
                                .enumerate()
                                .take(width - first_x)
                            {
                                let x = first_x + lane;
                                let nx = (x as f64 + 0.5) / width as f64 * 2.0 - 1.0;
                                let ny = 1.0 - (y as f64 + 0.5) / height as f64 * 2.0;
                                let scalar = basis.dir(nx, ny);
                                assert!(
                                    ray.direction.abs_diff_eq(scalar, 1e-14),
                                    "{width}x{height} ({x},{y}), alt {alt}"
                                );
                                assert!((ray.direction.length_squared() - 1.0).abs() < 1e-14);
                                let dot = basis.c.dot(scalar);
                                let disc = dot * dot - (basis.c.length_squared() - 1.0);
                                assert_eq!(
                                    ray.discriminant >= 0.0 && ray.camera_dot < 0.0,
                                    disc >= 0.0 && dot < 0.0,
                                    "hit classification ({x},{y}), alt {alt}"
                                );
                                if ray.discriminant >= 0.0 && ray.camera_dot < 0.0 {
                                    assert!(ray.distance >= 0.0);
                                    assert!((ray.point.length_squared() - 1.0).abs() < 1e-10);
                                    assert!((ray.point - basis.c)
                                        .normalize()
                                        .abs_diff_eq(scalar, 1e-9));
                                    assert!((ray.incidence + scalar.dot(ray.point)).abs() < 1e-12);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
