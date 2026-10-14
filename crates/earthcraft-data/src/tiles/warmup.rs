//! Bounded, one-pass satellite overview warming; visible requests take precedence.
use super::{Lookup, Provider, TileCache, TileKey, make_key};
use std::collections::VecDeque;

// z0..=4: 341 standard RGBA tiles, about 85 MiB within the 256 MiB cache target.
// This is a global overview, not a download of detailed imagery at every zoom.
const STARTUP_OVERVIEW_MAX_ZOOM: u8 = 4;
// Six workers serve visible tiles; overview work occupies at most two idle slots.
const OVERVIEW_INFLIGHT_LIMIT: usize = 2;

pub struct TileWarmup {
    remaining: VecDeque<TileKey>,
}

impl TileWarmup {
    /// Warm only the initial satellite layer. Other providers stay on demand;
    /// in particular, the public OSM tile service prohibits global prefetching.
    pub fn startup_satellite() -> Self {
        let remaining = (0..=STARTUP_OVERVIEW_MAX_ZOOM)
            .flat_map(|z| {
                let side = 1u32 << z;
                (0..side).flat_map(move |x| {
                    (0..side).map(move |y| make_key(Provider::Satellite, z, x, y, 0))
                })
            })
            .collect();
        Self { remaining }
    }

    /// Call after visible-tile admission while the overview is useful. Returns
    /// whether another pass is needed; does no I/O or waiting on the caller.
    /// Already cached/loading tiles and recent failures are skipped. A failed
    /// warmup is retried by normal on-demand loading, rather than an endless job.
    pub fn pump(&mut self, cache: &TileCache) -> bool {
        while let Some(&key) = self.remaining.front() {
            if matches!(cache.lookup(key), Lookup::Absent)
                && !cache.request_under_limit(key, OVERVIEW_INFLIGHT_LIMIT)
            {
                return true;
            }
            self.remaining.pop_front();
        }
        false
    }
    /// Stop speculative requests after the user explicitly clears the disk cache.
    pub fn cancel(&mut self) {
        self.remaining.clear();
    }
}
