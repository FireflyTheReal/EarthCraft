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
