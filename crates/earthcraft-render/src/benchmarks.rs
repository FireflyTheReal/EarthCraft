//! Manual CPU-frame comparison using isolated disk-cache fixtures and production APIs.
use super::*;
use earthcraft_core::{camera::Cam, math::now_unix};
use std::{
    hint::black_box,
    path::PathBuf,
    time::{Duration, Instant},
};

const FRAME_WIDTH_PX: usize = 512;
const FRAME_HEIGHT_PX: usize = 384;
const BENCH_FRAMES: usize = 20;
const BENCH_ROUNDS: usize = 5;

struct FixtureCache {
    root: PathBuf,
    previous: Option<std::ffi::OsString>,
}
impl FixtureCache {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "earthcraft-render-bench-{}-{}",
            std::process::id(),
            now_unix()
        ));
        let previous = std::env::var_os("LOCALAPPDATA");
        // This ignored benchmark runs alone, before constructing Tokio/Rayon workers.
        std::env::set_var("LOCALAPPDATA", &root);
        Self { root, previous }
    }
    fn prepare(&self) -> Vec<TileKey> {
        let mut keys = Vec::new();
        for z in 0..=4 {
            for x in 0..1u32 << z {
                for y in 0..1u32 << z {
                    let path = self
                        .root
                        .join("earthcraft_tiles/esri_sat")
                        .join(z.to_string())
                        .join(x.to_string())
                        .join(format!("{y}.tile"));
                    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                    std::fs::write(path, include_bytes!("../tests/fixtures/tile.png")).unwrap();
                    keys.push(make_key(Provider::Satellite, z, x, y, 0));
                }
            }
        }
        keys
    }
}
impl Drop for FixtureCache {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => std::env::set_var("LOCALAPPDATA", value),
            None => std::env::remove_var("LOCALAPPDATA"),
        }
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
#[ignore = "manual warm CPU-frame benchmark; run alone with --ignored --nocapture --test-threads=1"]
fn warm_cpu_frame_time() {
    let fixture = FixtureCache::new();
    let keys = fixture.prepare();
    let runtime = earthcraft_data::runtime::ServiceRuntime::new().unwrap();
    let cache = TileCache::new(|| {}, runtime.handle());
    let started = Instant::now();
    for &key in &keys {
        while !cache.request(key) {
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "fixture requests must finish"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    while keys
        .iter()
        .any(|&key| !matches!(cache.lookup(key), Lookup::Ready(_)))
    {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "fixture tiles must load from isolated disk cache"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    let basis = Cam {
        lat: -15.0,
        lon: -50.0,
        heading: 25.0,
        alt: 14000.0,
    }
    .basis(FRAME_WIDTH_PX as f64 / FRAME_HEIGHT_PX as f64);
    let config = RenderCfg {
        provider: Provider::Satellite,
        rel: 0,
        grid: true,
        grid_step: 10.0,
        atmo: true,
        sun: Some(earthcraft_core::math::ll_to_v3(20.0, 45.0)),
    };
    black_box(render_frame(
        FRAME_WIDTH_PX,
        FRAME_HEIGHT_PX,
        &basis,
        &config,
        &cache,
    ));
    let mut samples = Vec::new();
    for _ in 0..BENCH_ROUNDS {
        let started = Instant::now();
        for _ in 0..BENCH_FRAMES {
            black_box(render_frame(
                FRAME_WIDTH_PX,
                FRAME_HEIGHT_PX,
                &basis,
                &config,
                &cache,
            ));
        }
        samples.push(started.elapsed().as_secs_f64() * 1000.0 / BENCH_FRAMES as f64);
    }
    assert_eq!(
        cache.inflight(),
        0,
        "warm frame must not request network tiles"
    );
    samples.sort_by(f64::total_cmp);
    println!(
        "CPU-frame samples (ms): {samples:?}; Rayon threads: {}",
        rayon::current_num_threads()
    );
    println!(
        "Warm 512x384 CPU frame: median {:.3} ms; 341 fixture tiles, 5 rounds",
        samples[BENCH_ROUNDS / 2]
    );
}
