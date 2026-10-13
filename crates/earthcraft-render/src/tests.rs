use super::*;
use earthcraft_core::{camera::Cam, math::ll_to_v3};

struct ReferenceFrame {
    name: &'static str,
    cfg: RenderCfg,
    loaded: bool,
    expected: &'static [u8],
}

#[test]
fn raster_matches_the_original_implementation() {
    let (w, h) = (64, 48);
    let basis = Cam {
        lat: -15.0,
        lon: -50.0,
        heading: 25.0,
        alt: 14000.0,
    }
    .basis(w as f64 / h as f64);
    let config = |grid, atmo, sun| RenderCfg {
        provider: Provider::Satellite,
        rel: 0,
        grid,
        grid_step: 10.0,
        atmo,
        sun,
    };
    let sun = Some(ll_to_v3(20.0, 45.0));
    let scenarios = [
        ReferenceFrame {
            name: "plain",
            cfg: config(false, false, None),
            loaded: false,
            expected: include_bytes!("../tests/fixtures/plain.rgba"),
        },
        ReferenceFrame {
            name: "atmosphere",
            cfg: config(false, true, None),
            loaded: false,
            expected: include_bytes!("../tests/fixtures/atmosphere.rgba"),
        },
        ReferenceFrame {
            name: "grid-sun",
            cfg: config(true, true, sun),
            loaded: false,
            expected: include_bytes!("../tests/fixtures/grid-sun.rgba"),
        },
        ReferenceFrame {
            name: "loaded",
            cfg: config(true, true, sun),
            loaded: true,
            expected: include_bytes!("../tests/fixtures/loaded.rgba"),
        },
    ];
    let tile = Arc::new(Tile::decode(include_bytes!("../tests/fixtures/tile.png")).unwrap());
    for ReferenceFrame {
        name,
        cfg,
        loaded,
        expected,
    } in scenarios
    {
        let lookup = |key| {
            if loaded && key == make_key(Provider::Satellite, 0, 0, 0, 0) {
                Lookup::Ready(tile.clone())
            } else {
                Lookup::Absent
            }
        };
        let (buf, misses) = render_pixels(w, h, &basis, &cfg, &lookup);
        assert!(!misses.is_empty(), "missing tiles must still be reported");
        assert_eq!(buf.len(), expected.len());
        for (i, (actual, reference)) in buf.iter().zip(expected).enumerate() {
            // Permit one quantization step for platform floating-point differences.
            assert!(
                actual.abs_diff(*reference) <= 1,
                "{name}: channel {i}: {actual} != {reference}"
            );
        }
    }
}

#[test]
fn local_tile_lookup_is_deduplicated_and_keeps_the_nearest_miss() {
    let calls = std::sync::atomic::AtomicUsize::new(0);
    let lookup = |_| {
        calls.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Lookup::Absent
    };
    let mut loc = Local {
        lookup: &lookup,
        map: HashMap::default(),
    };
    for dist in [100.0, 80.0, 40.0, 30.0, 50.0] {
        assert!(loc.tile_color(1, dist, 0.0, 0.0).is_none());
    }
    assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 1);
    assert_eq!(loc.misses().collect::<Vec<_>>(), vec![(1, 30.0)]);
}
struct BatchReference {
    name: &'static str,
    width: usize,
    height: usize,
    provider: Provider,
    cam: Cam,
}
fn batch_references() -> [BatchReference; 6] {
    [
        BatchReference {
            name: "odd-satellite",
            width: 65,
            height: 49,
            provider: Provider::Satellite,
            cam: Cam {
                lat: -15.0,
                lon: -50.0,
                heading: 25.0,
                alt: 14_000.0,
            },
        },
        BatchReference {
            name: "polar-streets",
            width: 67,
            height: 51,
            provider: Provider::Streets,
            cam: Cam {
                lat: 89.9,
                lon: 179.9,
                heading: -130.0,
                alt: 1_000.0,
            },
        },
        BatchReference {
            name: "dateline-topo",
            width: 63,
            height: 47,
            provider: Provider::Topo,
            cam: Cam {
                lat: -40.0,
                lon: 179.9,
                heading: 70.0,
                alt: 500.0,
            },
        },
        BatchReference {
            name: "narrow-wayback",
            width: 3,
            height: 9,
            provider: Provider::Wayback,
            cam: Cam {
                lat: 0.0,
                lon: -180.0,
                heading: 180.0,
                alt: 30_000.0,
            },
        },
        BatchReference {
            name: "one-pixel-column",
            width: 1,
            height: 7,
            provider: Provider::Satellite,
            cam: Cam {
                lat: -15.0,
                lon: -50.0,
                heading: 25.0,
                alt: 14_000.0,
            },
        },
        BatchReference {
            name: "low-altitude",
            width: 7,
            height: 5,
            provider: Provider::Satellite,
            cam: Cam {
                lat: 89.9,
                lon: -179.9,
                heading: -20.0,
                alt: earthcraft_core::MIN_CAMERA_ALTITUDE_KM,
            },
        },
    ]
}
fn batch_reference_config(provider: Provider) -> RenderCfg {
    RenderCfg {
        provider,
        rel: 7,
        grid: true,
        grid_step: 10.0,
        atmo: true,
        sun: Some(ll_to_v3(20.0, 45.0)),
    }
}

#[test]
fn ray_batches_preserve_partial_rows_provider_pixels_and_missing_tile_priorities() {
    let tile = Arc::new(Tile::decode(include_bytes!("../tests/fixtures/tile.png")).unwrap());
    let frames: [&[u8]; 6] = [
        include_bytes!("../tests/fixtures/batches/odd-satellite.rgba"),
        include_bytes!("../tests/fixtures/batches/polar-streets.rgba"),
        include_bytes!("../tests/fixtures/batches/dateline-topo.rgba"),
        include_bytes!("../tests/fixtures/batches/narrow-wayback.rgba"),
        include_bytes!("../tests/fixtures/batches/one-pixel-column.rgba"),
        include_bytes!("../tests/fixtures/batches/low-altitude.rgba"),
    ];
    let expected_misses: [&[u8]; 6] = [
        include_bytes!("../tests/fixtures/batches/odd-satellite.misses"),
        include_bytes!("../tests/fixtures/batches/polar-streets.misses"),
        include_bytes!("../tests/fixtures/batches/dateline-topo.misses"),
        include_bytes!("../tests/fixtures/batches/narrow-wayback.misses"),
        include_bytes!("../tests/fixtures/batches/one-pixel-column.misses"),
        include_bytes!("../tests/fixtures/batches/low-altitude.misses"),
    ];
    for ((case, expected), expected_misses) in batch_references()
        .into_iter()
        .zip(frames)
        .zip(expected_misses)
    {
        let cfg = batch_reference_config(case.provider);
        let lookup = |key| {
            if key == make_key(case.provider, 0, 0, 0, 7) {
                Lookup::Ready(tile.clone())
            } else {
                Lookup::Absent
            }
        };
        let (buf, mut misses) = render_pixels(
            case.width,
            case.height,
            &case.cam.basis(case.width as f64 / case.height as f64),
            &cfg,
            &lookup,
        );
        assert_eq!(buf.len(), expected.len(), "{}", case.name);
        for (i, (actual, reference)) in buf.iter().zip(expected).enumerate() {
            assert!(
                actual.abs_diff(*reference) <= 1,
                "{}: channel {i}: {actual} != {reference}",
                case.name
            );
        }
        misses.sort_unstable_by_key(|&(key, _)| key);
        let bytes: Vec<u8> = misses
            .into_iter()
            .flat_map(|(key, dist)| key.to_le_bytes().into_iter().chain(dist.to_le_bytes()))
            .collect();
        assert_eq!(
            bytes, expected_misses,
            "{}: padding must not add requests or change priority",
            case.name
        );
    }
}
