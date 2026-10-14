//! Texture-ready body geometry without a GUI backend or texture handle.
use emath::{Pos2, Rect};
const REFERENCE_ALTITUDE_KM: f64 = 18_895.0;
const MIN_ALTITUDE_KM: f64 = 1.0;
const MIN_ZOOM: f64 = 0.08;
const MAX_ZOOM: f64 = 32.0;
const RADIUS_VIEWPORT_RATIO: f32 = 0.44;
#[derive(Default)]
pub struct PlanetMesh {
    pub vertices: Vec<PlanetVertex>,
    pub indices: Vec<u32>,
}
pub struct PlanetVertex {
    pub pos: Pos2,
    pub uv: Pos2,
}
pub fn planet_mesh(rect: Rect, lat: f64, lon: f64, altitude_km: f64) -> PlanetMesh {
    // Keep the previous apparent diameter at the initial camera distance.
    // Zooming the globe now scales the Moon/Mars/custom body too.
    let zoom = (REFERENCE_ALTITUDE_KM / altitude_km.max(MIN_ALTITUDE_KM))
        .sqrt()
        .clamp(MIN_ZOOM, MAX_ZOOM) as f32;
    let radius = rect.width().min(rect.height()) * RADIUS_VIEWPORT_RATIO * zoom;
    let center = rect.center();
    let latitude = lat.to_radians();
    let longitude = lon.to_radians();
    const RADIAL: usize = 72;
    const ANGULAR: usize = 256;

    // Build rings as exact circles. Unlike the old Cartesian raster, no cell
    // can extend past the silhouette or leave jagged stair-step boundary gaps.
    let mut mesh = PlanetMesh::default();
    for ring in 0..RADIAL {
        let inner = ring as f64 / RADIAL as f64;
        let outer = (ring + 1) as f64 / RADIAL as f64;
        for sector in 0..ANGULAR {
            let angle0 = std::f64::consts::TAU * sector as f64 / ANGULAR as f64;
            let angle1 = std::f64::consts::TAU * (sector + 1) as f64 / ANGULAR as f64;
            let corners = [
                (inner * angle0.cos(), inner * angle0.sin()),
                (outer * angle0.cos(), outer * angle0.sin()),
                (outer * angle1.cos(), outer * angle1.sin()),
                (inner * angle1.cos(), inner * angle1.sin()),
            ];
            let mut uvs = [(0.0_f64, 0.0_f64); 4];
            for (i, &(x, y)) in corners.iter().enumerate() {
                let z = (1.0 - (x * x + y * y).min(1.0)).sqrt();
                let xp = x * longitude.cos() + z * longitude.sin();
                let zp = -x * longitude.sin() + z * longitude.cos();
                let yp = y * latitude.cos() + zp * latitude.sin();
                let zz = -y * latitude.sin() + zp * latitude.cos();
                uvs[i] = (
                    (0.5 + xp.atan2(zz) / std::f64::consts::TAU).rem_euclid(1.0),
                    (0.5 - yp.clamp(-1.0, 1.0).asin() / std::f64::consts::PI).clamp(0.0, 1.0),
                );
            }
            // Prevent texture smearing across the 0/1 longitude seam.
            let u_min = uvs.iter().map(|uv| uv.0).fold(f64::INFINITY, f64::min);
            let u_max = uvs.iter().map(|uv| uv.0).fold(f64::NEG_INFINITY, f64::max);
            if u_max - u_min > 0.5 {
                // Clamp each triangle to one side of the dateline, avoiding
                // the texture interpolation jump across the entire image.
                let left_count = uvs.iter().filter(|uv| uv.0 < 0.5).count();
                if left_count >= 2 {
                    for uv in &mut uvs {
                        if uv.0 > 0.5 {
                            uv.0 = 0.0;
                        }
                    }
                } else {
                    for uv in &mut uvs {
                        if uv.0 < 0.5 {
                            uv.0 = 1.0;
                        }
                    }
                }
            }
            let base = mesh.vertices.len() as u32;
            for (i, &(x, y)) in corners.iter().enumerate() {
                mesh.vertices.push(PlanetVertex {
                    pos: Pos2::new(center.x + x as f32 * radius, center.y + y as f32 * radius),
                    uv: Pos2::new(uvs[i].0 as f32, uvs[i].1 as f32),
                });
            }
            mesh.indices.extend([base, base + 1, base + 2]);
            mesh.indices.extend([base, base + 2, base + 3]);
        }
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    use emath::Vec2;
    #[test]
    fn mesh_has_valid_winding_indices_uvs_and_dateline_seams() {
        let rect = Rect::from_min_size(Pos2::new(30.0, 50.0), Vec2::new(800.0, 600.0));
        for (lat, lon) in [(0.0, 0.0), (89.9, 180.0), (-89.9, -179.0), (40.0, 100.0)] {
            let mesh = planet_mesh(rect, lat, lon, 18_895.0);
            assert_eq!(mesh.vertices.len(), 72 * 256 * 4);
            assert_eq!(mesh.indices.len(), 72 * 256 * 6);
            for vertex in &mesh.vertices {
                assert!(rect.contains(vertex.pos));
                assert!((0.0..=1.0).contains(&vertex.uv.x));
                assert!((0.0..=1.0).contains(&vertex.uv.y));
            }
            for quad in mesh.vertices.as_chunks::<4>().0 {
                let min = quad.iter().map(|p| p.uv.x).fold(f32::INFINITY, f32::min);
                let max = quad
                    .iter()
                    .map(|p| p.uv.x)
                    .fold(f32::NEG_INFINITY, f32::max);
                assert!(max - min <= 0.500001);
            }
            assert!(
                mesh.indices
                    .iter()
                    .all(|&index| (index as usize) < mesh.vertices.len())
            );
        }
    }
}
