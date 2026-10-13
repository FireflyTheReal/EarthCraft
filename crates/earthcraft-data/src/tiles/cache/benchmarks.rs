//! Manual comparison of production cached-hit throughput; no timing assertions.
use super::*;
use crate::tiles::Provider;
use std::{hint::black_box, sync::Barrier};

const BENCH_TILE_COUNT: usize = 256;
const BENCH_LOOKUPS_PER_THREAD: usize = 50_000;
const BENCH_ROUNDS: usize = 5;
const BENCH_TILE_SIZE_PX: usize = 64;

#[test]
#[ignore = "manual cache throughput benchmark; run with --ignored --nocapture"]
fn cached_hit_throughput() {
    let (tx, _rx) = tokio::sync::mpsc::channel(MAX_INFLIGHT);
    let cache = Arc::new(TileCache::with_budget(tx, DEFAULT_TILE_CACHE_BUDGET_BYTES));
    let keys: Vec<_> = (0..BENCH_TILE_COUNT)
        .map(|i| {
            super::super::make_key(Provider::Satellite, 4, (i % 16) as u32, (i / 16) as u32, 0)
        })
        .collect();
    for &key in &keys {
        cache.complete(
            key,
            Ok(Tile {
                w: BENCH_TILE_SIZE_PX,
                h: BENCH_TILE_SIZE_PX,
                px: vec![128; BENCH_TILE_SIZE_PX * BENCH_TILE_SIZE_PX * 4],
            }),
        );
    }
    cache.ready.run_pending_tasks();
    for threads in [1, 8] {
        let mut samples = Vec::new();
        for _ in 0..BENCH_ROUNDS {
            let barrier = Barrier::new(threads + 1);
            let elapsed = std::thread::scope(|scope| {
                let mut jobs = Vec::new();
                for worker in 0..threads {
                    let (cache, keys, barrier) = (&cache, &keys, &barrier);
                    jobs.push(scope.spawn(move || {
                        barrier.wait();
                        for i in 0..BENCH_LOOKUPS_PER_THREAD {
                            let key = keys[(i * 17 + worker * 31) % keys.len()];
                            assert!(matches!(black_box(cache.lookup(key)), Lookup::Ready(_)));
                        }
                    }));
                }
                let started = Instant::now();
                barrier.wait();
                for job in jobs {
                    job.join().unwrap();
                }
                started.elapsed()
            });
            samples.push(threads as f64 * BENCH_LOOKUPS_PER_THREAD as f64 / elapsed.as_secs_f64());
        }
        samples.sort_by(f64::total_cmp);
        println!(
            "{threads} thread(s): median {:.2} million cached hits/s; 256 tiles, 5 rounds",
            samples[BENCH_ROUNDS / 2] / 1_000_000.0
        );
    }
}
