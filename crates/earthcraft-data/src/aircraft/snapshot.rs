use super::{Aircraft, AircraftSnapshot, MAX_POSITION_AGE_SECS, ProviderStatus};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

const GLOBAL_MAX_AGE: Duration = Duration::from_secs(20 * 60);
const LOCAL_MAX_AGE: Duration = Duration::from_secs(3 * 60);
const MIN_EXPIRY_REFRESH: Duration = Duration::from_secs(1);

#[derive(Clone, Default)]
pub(super) struct Samples {
    pub aircraft: Arc<[Aircraft]>,
    pub status: ProviderStatus,
}

pub(super) struct Prepared {
    pub snapshot: Arc<AircraftSnapshot>,
    pub expires: Option<Instant>,
}

pub(super) fn prepare(global: Samples, local: Samples, now: Instant, unix_secs: f64) -> Prepared {
    #[cfg(feature = "profiling")]
    let _profile = crate::profiling::Span::new("aircraft.snapshot");
    let mut by_hex = HashMap::with_capacity(global.aircraft.len() + local.aircraft.len());
    let mut expires = None;
    if let Some(loaded) = global
        .status
        .loaded
        .filter(|loaded| now.saturating_duration_since(*loaded) <= GLOBAL_MAX_AGE)
    {
        expires = Some(loaded + GLOBAL_MAX_AGE);
        for flight in global.aircraft.iter() {
            by_hex.insert(flight.hex.as_ref(), flight);
        }
    }
    if let Some(loaded) = local
        .status
        .loaded
        .filter(|loaded| now.saturating_duration_since(*loaded) <= LOCAL_MAX_AGE)
    {
        let deadline = loaded + LOCAL_MAX_AGE;
        expires = Some(expires.map_or(deadline, |old| old.min(deadline)));
        for flight in local.aircraft.iter() {
            let remaining = MAX_POSITION_AGE_SECS - (unix_secs - flight.observed_unix);
            if remaining >= 0.0 {
                let deadline = now + Duration::from_secs_f64(remaining);
                expires = Some(expires.map_or(deadline, |old| old.min(deadline)));
                if by_hex
                    .get(flight.hex.as_ref())
                    .is_none_or(|old: &&Aircraft| flight.observed_unix >= old.observed_unix)
                {
                    by_hex.insert(flight.hex.as_ref(), flight);
                }
            }
        }
    }
    let mut aircraft: Vec<_> = by_hex.into_values().cloned().collect();
    aircraft.sort_unstable_by(|a, b| a.hex.cmp(&b.hex));
    let expires = expires.map(|deadline| deadline.max(now + MIN_EXPIRY_REFRESH));
    Prepared {
        snapshot: Arc::new(AircraftSnapshot {
            aircraft: aircraft.into(),
            global: global.status,
            local: local.status,
            error: None,
            valid_until: expires,
        }),
        expires,
    }
}
