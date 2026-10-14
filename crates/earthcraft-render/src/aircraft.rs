//! Project actual flight telemetry on the same globe as the base imagery.
use earthcraft_core::{
    camera::Basis,
    math::{destination, ll_to_v3},
};
use earthcraft_data::aircraft::Aircraft;
use emath::{Pos2, Rect, Vec2};
#[cfg(test)]
mod benchmarks;
mod worker;
pub use worker::{
    AircraftGeometry, AircraftPreparer, AircraftWorker, ProjectionInput, ProjectionView,
};

const ICON_SIZE_POINTS: f32 = 7.0;
const TRAIL_SIZE_POINTS: f32 = 12.0;
const WING_WIDTH_RATIO: f32 = 0.4;
const EXTRAPOLATION_MAX_SECS: f64 = 25.0;
const EXTRAPOLATION_MAX_SPEED_KNOTS: f64 = 1200.0;
const KM_PER_NAUTICAL_MILE: f64 = 1.852;
const SECONDS_PER_HOUR: f64 = 3600.0;
const HEADING_PROBE_KM: f64 = 3.0;
const WING_REAR_OFFSET_RATIO: f32 = 0.25;
const TAIL_OFFSET_RATIO: f32 = 0.85;
const TRAIL_START_RATIO: f32 = 0.5;
const TRAIL_END_RATIO: f32 = 1.1;

pub struct ProjectedFlight {
    pub source_index: usize,
    pub position: Pos2,
    pub icon: [Pos2; 4],
    pub trail: [Pos2; 2],
}

pub fn project_flights(
    basis: &Basis,
    rect: Rect,
    aircraft: &[Aircraft],
    unix_secs: f64,
    map_scale: f32,
) -> Vec<ProjectedFlight> {
    aircraft
        .iter()
        .enumerate()
        .filter_map(|(source_index, flight)| {
            let age = (unix_secs - flight.observed_unix).max(0.0);
            // Never invent an entire route between remote updates. Small extrapolation
            // is allowed for recent transponder reports, then the marker freezes.
            let speed_km_s = flight
                .speed_knots
                .unwrap_or(0.0)
                .clamp(0.0, EXTRAPOLATION_MAX_SPEED_KNOTS)
                * KM_PER_NAUTICAL_MILE
                / SECONDS_PER_HOUR;
            let displacement = speed_km_s * age.min(EXTRAPOLATION_MAX_SECS);
            let (lat, lon) = destination(
                flight.latitude,
                flight.longitude,
                flight.heading_deg,
                displacement,
            );
            let screen = basis.project(ll_to_v3(lat, lon), rect)?;
            if !rect.contains(screen) {
                return None;
            }
            // Project a small step along the actual aircraft track to orient its icon
            // correctly even when the camera is rotated or near a pole.
            let (ahead_lat, ahead_lon) =
                destination(lat, lon, flight.heading_deg, HEADING_PROBE_KM);
            let heading = basis
                .project(ll_to_v3(ahead_lat, ahead_lon), rect)
                .map(|ahead| (ahead.y - screen.y).atan2(ahead.x - screen.x))
                .unwrap_or(-std::f32::consts::FRAC_PI_2);
            let forward = Vec2::angled(heading) * (ICON_SIZE_POINTS * map_scale);
            let wing = Vec2::new(-forward.y, forward.x) * WING_WIDTH_RATIO;
            let tail = Vec2::angled(heading) * (TRAIL_SIZE_POINTS * map_scale);
            Some(ProjectedFlight {
                source_index,
                position: screen,
                icon: [
                    screen + forward,
                    screen + wing - forward * WING_REAR_OFFSET_RATIO,
                    screen - forward * TAIL_OFFSET_RATIO,
                    screen - wing - forward * WING_REAR_OFFSET_RATIO,
                ],
                trail: [
                    screen - tail * TRAIL_START_RATIO,
                    screen - tail * TRAIL_END_RATIO,
                ],
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn eastbound_plane_moves_east_and_westbound_wraps_dateline() {
        let (lat, lon) = destination(0.0, 0.0, 90.0, 110.0);
        assert!(lat.abs() < 0.01);
        assert!(lon > 0.9 && lon < 1.1);
        let (_, lon) = destination(0.0, 179.9, 90.0, 50.0);
        assert!(lon < -179.0);
    }

    #[test]
    fn projected_geometry_retains_the_original_upstream_fixture() {
        use earthcraft_core::camera::Cam;
        const TOLERANCE_POINTS: f32 = 0.0001;
        let flights = super::benchmarks::flights();
        let rect = Rect::from_min_size(Pos2::ZERO, emath::vec2(800.0, 600.0));
        let mut bytes = include_bytes!("../tests/fixtures/aircraft-original.bin").as_slice();
        fn read(bytes: &mut &[u8]) -> u32 {
            let (word, rest) = bytes.split_at(4);
            *bytes = rest;
            u32::from_le_bytes(word.try_into().unwrap())
        }
        for (lat, lon, heading, alt) in [
            (0.0, 0.0, 25.0, 14_000.0),
            (30.0, 179.8, 145.0, 1000.0),
            (89.9, -150.0, 90.0, 500.0),
        ] {
            let basis = Cam {
                lat,
                lon,
                heading,
                alt,
            }
            .basis(4.0 / 3.0);
            let projected = project_flights(&basis, rect, &flights, 1010.0, 1.0);
            assert_eq!(projected.len(), read(&mut bytes) as usize);
            let count = read(&mut bytes) as usize;
            for row in 0..count {
                let flight = &projected[row * projected.len() / count];
                assert_eq!(flight.source_index, read(&mut bytes) as usize);
                for point in std::iter::once(flight.position)
                    .chain(flight.icon)
                    .chain(flight.trail)
                {
                    let x = f32::from_bits(read(&mut bytes));
                    let y = f32::from_bits(read(&mut bytes));
                    assert!(
                        (point.x - x).abs() <= TOLERANCE_POINTS
                            && (point.y - y).abs() <= TOLERANCE_POINTS
                    );
                }
            }
        }
        assert!(bytes.is_empty());
    }
}
