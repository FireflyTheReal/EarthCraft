use super::*;
use earthcraft_core::math::now_unix;
use std::{hint::black_box, time::Instant};

const AIRCRAFT_COUNT: usize = 20_000;
const ROUNDS: usize = 5;
const ITERATIONS: usize = 100;

#[test]
#[ignore = "manual isolated aircraft snapshot benchmark"]
fn aircraft_snapshot_cost() {
    let now = now_unix();
    let flights = (0..AIRCRAFT_COUNT)
        .map(|index| Aircraft {
            hex: format!("{index:06x}").into(),
            callsign: format!("FLIGHT{index}").into(),
            latitude: (index % 160) as f64 - 80.0,
            longitude: (index % 360) as f64 - 180.0,
            heading_deg: 90.0,
            speed_knots: Some(440.0),
            altitude_feet: Some(35_000.0),
            observed_unix: now,
        })
        .collect();
    let prepared = snapshot::prepare(
        snapshot::Samples {
            aircraft: flights,
            status: ProviderStatus {
                loaded: Some(Instant::now()),
                ..Default::default()
            },
        },
        snapshot::Samples::default(),
        Instant::now(),
        now,
    );
    let (control, _) = watch::channel(AircraftQuery::default());
    let (_publish, updates) = watch::channel(prepared.snapshot.clone());
    let service = LiveAircraft {
        control,
        updates,
        latest: prepared.snapshot,
        stopped: false,
    };
    let mut samples = Vec::new();
    for _ in 0..ROUNDS {
        let start = Instant::now();
        for _ in 0..ITERATIONS {
            black_box(service.snapshot());
        }
        samples.push(start.elapsed().as_secs_f64() * 1_000_000.0 / ITERATIONS as f64);
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "20,000 aircraft snapshot access: median {:.3} us; samples {samples:?}",
        samples[ROUNDS / 2]
    );
}
