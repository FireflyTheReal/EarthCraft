//! Project illustrative aircraft into frontend-neutral screen geometry.
use earthcraft_core::{aircraft::sample_flights, camera::Basis, math::ll_to_v3};
use emath::{Pos2, Rect, Vec2};
const ICON_SIZE_POINTS: f32 = 9.0;
const TRAIL_SIZE_POINTS: f32 = 14.0;
const WING_WIDTH_RATIO: f32 = 0.33;
const WING_BACK_RATIO: f32 = 0.30;
const TAIL_BACK_RATIO: f32 = 0.15;
const TRAIL_START_RATIO: f32 = 0.35;
const TRAIL_END_RATIO: f32 = 1.2;
pub struct ProjectedFlight {
    pub code: &'static str,
    pub flight_level: i32,
    pub position: Pos2,
    pub icon: [Pos2; 4],
    pub trail: [Pos2; 2],
}
pub fn project_flights(
    basis: &Basis,
    rect: Rect,
    unix_secs: f64,
    map_scale: f32,
) -> Vec<ProjectedFlight> {
    sample_flights(unix_secs)
        .filter_map(|flight| {
            let screen = basis.project(ll_to_v3(flight.position.0, flight.position.1), rect)?;
            if !rect.contains(screen) {
                return None;
            }
            let heading = basis
                .project(
                    ll_to_v3(flight.next_position.0, flight.next_position.1),
                    rect,
                )
                .map(|n| (n.y - screen.y).atan2(n.x - screen.x))
                .unwrap_or(0.0);
            let forward = Vec2::angled(heading) * (ICON_SIZE_POINTS * map_scale);
            let side = Vec2::new(-forward.y, forward.x) * WING_WIDTH_RATIO;
            let back = Vec2::angled(heading) * (TRAIL_SIZE_POINTS * map_scale);
            Some(ProjectedFlight {
                code: flight.code,
                flight_level: flight.flight_level,
                position: screen,
                icon: [
                    screen + forward,
                    screen - forward * WING_BACK_RATIO + side,
                    screen - forward * TAIL_BACK_RATIO,
                    screen - forward * WING_BACK_RATIO - side,
                ],
                trail: [
                    screen - back * TRAIL_START_RATIO,
                    screen - back * TRAIL_END_RATIO,
                ],
            })
        })
        .collect()
}
