use super::super::make_key;
use super::*;
#[test]
fn tile_key_roundtrip() {
    for (p, z, x, y, rel) in [
        (Provider::Wayback, 17u8, 98_765u32, 123_456u32, 4321u16),
        (Provider::Satellite, 0, 0, 0, 0),
        (Provider::Streets, 19, 524_287, 524_287, 0),
        (Provider::Topo, 5, 17, 9, 0),
    ] {
        let k = make_key(p, z, x, y, rel);
        assert_eq!(unkey(k), (p, z, x, y, rel));
    }
    assert_ne!(
        make_key(Provider::Wayback, 10, 1, 1, 1),
        make_key(Provider::Wayback, 10, 1, 1, 2)
    );
}

fn test_cache(
    budget: usize,
) -> (
    Arc<TileCache>,
    tokio::sync::mpsc::UnboundedReceiver<TileKey>,
) {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    (Arc::new(TileCache::with_budget(tx, budget)), rx)
}
fn tiny_tile(value: u8) -> Tile {
    Tile {
        w: 1,
        h: 1,
        px: vec![value, value, value, 255],
    }
}

#[test]
fn cache_evicts_least_recently_used_with_a_byte_weighted_target() {
    let (cache, _rx) = test_cache(8);
    cache.complete(1, Ok(tiny_tile(1)));
    cache.complete(2, Ok(tiny_tile(2)));
    cache.ready.run_pending_tasks();
    assert!(matches!(cache.lookup(1), Lookup::Ready(_)));
    cache.ready.run_pending_tasks();
    cache.complete(3, Ok(tiny_tile(3)));
    cache.ready.run_pending_tasks();
    assert!(matches!(cache.lookup(2), Lookup::Absent));
    assert!(matches!(cache.lookup(1), Lookup::Ready(_)));
    assert_eq!(cache.resident_bytes(), 8);
    cache.complete(1, Ok(tiny_tile(4)));
    cache.ready.run_pending_tasks();
    assert_eq!(cache.resident_bytes(), 8);
    let Lookup::Ready(tile) = cache.lookup(1) else {
        panic!("replacement must be cached")
    };
    assert_eq!(tile.px[0], 4);
}

#[test]
fn failed_requests_and_oversized_tiles_keep_their_retry_delay_without_pixel_weight() {
    let (cache, _rx) = test_cache(4);
    cache.complete(1, Err("network failure".into()));
    cache.complete(
        2,
        Ok(Tile {
            w: 2,
            h: 1,
            px: vec![0; 8],
        }),
    );
    cache.ready.run_pending_tasks();
    assert_eq!(cache.resident_bytes(), 0);
    assert!(matches!(cache.lookup(1), Lookup::Pending));
    assert!(matches!(cache.lookup(2), Lookup::Pending));
    assert!(!cache.request(1));
    assert!(!cache.request(2));
    cache.requests.lock().unwrap().insert(
        1,
        RequestState::Failed(Instant::now() - FAILED_TILE_RETRY_DELAY),
    );
    assert!(matches!(cache.lookup(1), Lookup::Absent));
    assert!(cache.request(1));
    assert_eq!(cache.inflight(), 1);
    cache.complete(1, Err("network failure".into()));
    assert_eq!(cache.inflight(), 0);
    assert_eq!(cache.generation(), 3);
}

#[test]
fn concurrent_requests_for_one_tile_enqueue_once() {
    let (cache, mut rx) = test_cache(8);
    let barrier = std::sync::Barrier::new(8);
    let admitted = std::thread::scope(|scope| {
        let jobs: Vec<_> = (0..8)
            .map(|_| {
                let (cache, barrier) = (&cache, &barrier);
                scope.spawn(move || {
                    barrier.wait();
                    cache.request(1)
                })
            })
            .collect();
        jobs.into_iter()
            .map(|job| job.join().unwrap())
            .filter(|admitted| *admitted)
            .count()
    });
    assert_eq!(admitted, 1);
    assert_eq!(cache.inflight(), 1);
    assert_eq!(rx.try_recv().unwrap(), 1);
    assert!(rx.try_recv().is_err());
}

#[test]
fn request_limit_completion_and_closed_queue_account_for_inflight_work() {
    let (cache, mut rx) = test_cache(8);
    for key in 0..MAX_INFLIGHT as u64 {
        assert!(cache.request(key));
    }
    assert!(!cache.request(MAX_INFLIGHT as u64));
    assert!(!cache.request(0));
    assert_eq!(cache.inflight(), MAX_INFLIGHT);
    assert_eq!(
        (0..MAX_INFLIGHT).map(|_| rx.try_recv().unwrap()).count(),
        MAX_INFLIGHT
    );
    cache.complete(0, Ok(tiny_tile(1)));
    assert_eq!(cache.inflight(), MAX_INFLIGHT - 1);
    assert_eq!(cache.generation(), 1);
    assert!(!cache.request(0));
    assert!(cache.request(MAX_INFLIGHT as u64));
    assert_eq!(cache.inflight(), MAX_INFLIGHT);

    let (closed, rx) = test_cache(8);
    drop(rx);
    assert!(!closed.request(1));
    assert_eq!(closed.inflight(), 0);
    assert!(matches!(closed.lookup(1), Lookup::Pending));
}

#[test]
fn cached_hits_do_not_wait_for_request_coordination() {
    let (cache, _rx) = test_cache(8);
    cache.complete(1, Ok(tiny_tile(1)));
    let guard = cache.requests.lock().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let reader = cache.clone();
    let job = std::thread::spawn(move || {
        tx.send(matches!(reader.lookup(1), Lookup::Ready(_)))
            .unwrap();
    });
    let result = rx.recv_timeout(Duration::from_secs(5));
    drop(guard);
    job.join().unwrap();
    assert!(result.expect("cached hit should bypass request mutex"));
}
