//! Great-circle ruler geometry shared by frontends.
use earthcraft_core::{
    camera::Basis,
    math::{great_circle, haversine_km, ll_to_v3},
};
use emath::{Pos2, Rect};
const ARC_SEGMENTS: usize = 48;
pub struct RulerLeg {
    pub segments: Vec<[Pos2; 2]>,
    pub label_position: Option<Pos2>,
    pub distance_km: f64,
}
pub fn project_ruler(basis: &Basis, rect: Rect, points: &[(f64, f64)]) -> Vec<RulerLeg> {
    points
        .windows(2)
        .map(|pair| {
            let projected: Vec<_> = great_circle(pair[0], pair[1], ARC_SEGMENTS)
                .into_iter()
                .map(|point| basis.project(point, rect))
                .collect();
            let segments = projected
                .windows(2)
                .filter_map(|p| match (p[0], p[1]) {
                    (Some(a), Some(b)) => Some([a, b]),
                    _ => None,
                })
                .collect();
            let label_position = basis
                .project(ll_to_v3(pair[0].0, pair[0].1), rect)
                .zip(basis.project(ll_to_v3(pair[1].0, pair[1].1), rect))
                .map(|(a, b)| Pos2::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0));
            RulerLeg {
                segments,
                label_position,
                distance_km: haversine_km(pair[0], pair[1]),
            }
        })
        .collect()
}
