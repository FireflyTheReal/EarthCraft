use super::{
    Aircraft, AircraftQuery, AircraftSnapshot, decode,
    snapshot::{self, Samples},
};
use earthcraft_core::math::{haversine_km, now_unix};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{sync::watch, task::JoinHandle};

const GLOBAL_REFRESH: Duration = Duration::from_secs(17 * 60);
const LOCAL_REFRESH: Duration = Duration::from_secs(90);
const FAILURE_RETRY: Duration = Duration::from_secs(3 * 60);
const MOVE_RETRY_MIN: Duration = Duration::from_secs(30);
const MOVE_REFRESH_KM: f64 = 140.0;
const LOCAL_MAX_CAMERA_ALT_KM: f64 = 1800.0;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refresh_and_failure_deadlines_preserve_provider_limits() {
        let now = Instant::now();
        let mut provider = Provider::default();
        assert_eq!(provider.deadline(now, GLOBAL_REFRESH), now);
        provider.attempt = Some(now);
        assert_eq!(provider.deadline(now, GLOBAL_REFRESH), now + GLOBAL_REFRESH);
        assert_eq!(provider.deadline(now, LOCAL_REFRESH), now + LOCAL_REFRESH);
        provider.samples.status.error = Some(Arc::new("fixture error".into()));
        assert_eq!(provider.deadline(now, GLOBAL_REFRESH), now + FAILURE_RETRY);
        assert_eq!(provider.deadline(now, LOCAL_REFRESH), now + FAILURE_RETRY);
        assert!(haversine_km((0.0, 179.8), (0.0, -179.8)) < MOVE_REFRESH_KM);
    }
}

pub(super) struct Endpoints {
    pub global: String,
    pub local: String,
}
impl Default for Endpoints {
    fn default() -> Self {
        Self {
            global: "https://opensky-network.org/api/states/all".into(),
            local: "https://api.adsb.lol/v2".into(),
        }
    }
}

#[derive(Default)]
struct Provider {
    samples: Samples,
    attempt: Option<Instant>,
    job: Option<JoinHandle<Result<Arc<[Aircraft]>, crate::Error>>>,
}
impl Provider {
    fn deadline(&self, now: Instant, interval: Duration) -> Instant {
        self.attempt.map_or(now, |last| {
            last + if self.samples.status.error.is_none() {
                interval
            } else {
                FAILURE_RETRY
            }
        })
    }
    fn finish(
        &mut self,
        result: Result<Result<Arc<[Aircraft]>, crate::Error>, tokio::task::JoinError>,
    ) {
        self.job = None;
        match result.map_err(crate::Error::from).and_then(|result| result) {
            Ok(aircraft) => {
                self.samples.aircraft = aircraft;
                self.samples.status.loaded = Some(Instant::now());
                self.samples.status.error = None;
            }
            Err(error) => self.samples.status.error = Some(Arc::new(error)),
        }
    }
}
impl Drop for Provider {
    fn drop(&mut self) {
        if let Some(job) = &self.job {
            job.abort();
        }
    }
}

async fn receive(
    provider: &mut Provider,
) -> Result<Result<Arc<[Aircraft]>, crate::Error>, tokio::task::JoinError> {
    match &mut provider.job {
        Some(job) => job.await,
        None => std::future::pending().await,
    }
}

pub(super) async fn run(
    mut queries: watch::Receiver<AircraftQuery>,
    publish: watch::Sender<Arc<AircraftSnapshot>>,
    repaint: impl Fn() + Send + Sync + 'static,
    endpoints: Endpoints,
) {
    let mut global = Provider::default();
    let mut local = Provider::default();
    let mut local_center = None;
    let mut query = *queries.borrow_and_update();
    let mut dirty = false;
    let mut expires = None;
    loop {
        let now = Instant::now();
        let mut deadline = if query.enabled { expires } else { None };
        if query.enabled {
            if global.job.is_none() {
                let due = global.deadline(now, GLOBAL_REFRESH);
                if now >= due {
                    global.attempt = Some(now);
                    global.samples.status.attempted = true;
                    global.job = Some(tokio::spawn(decode::download_global(
                        endpoints.global.clone(),
                    )));
                    dirty = true;
                } else {
                    deadline = Some(deadline.map_or(due, |old: Instant| old.min(due)));
                }
            }
            if query.altitude_km <= LOCAL_MAX_CAMERA_ALT_KM && local.job.is_none() {
                let moved = local_center.is_some_and(|center| {
                    haversine_km(center, (query.latitude, query.longitude)) > MOVE_REFRESH_KM
                });
                let movement = local.attempt.map_or(now, |last| last + MOVE_RETRY_MIN);
                let due = if moved {
                    local.deadline(now, LOCAL_REFRESH).min(movement)
                } else {
                    local.deadline(now, LOCAL_REFRESH)
                };
                if now >= due {
                    local.attempt = Some(now);
                    local.samples.status.attempted = true;
                    local_center = Some((query.latitude, query.longitude));
                    local.job = Some(tokio::spawn(decode::download_local(
                        endpoints.local.clone(),
                        query.latitude,
                        query.longitude,
                    )));
                    dirty = true;
                } else {
                    deadline = Some(deadline.map_or(due, |old| old.min(due)));
                }
            }
        }
        if dirty || (query.enabled && expires.is_some_and(|time| now >= time)) {
            let (global_input, local_input) = (global.samples.clone(), local.samples.clone());
            match crate::cpu::run(crate::cpu::Service::Features, move || {
                Ok(snapshot::prepare(
                    global_input,
                    local_input,
                    Instant::now(),
                    now_unix(),
                ))
            })
            .await
            {
                Ok(prepared) => {
                    expires = prepared.expires;
                    publish.send_replace(prepared.snapshot);
                }
                Err(error) => {
                    let old = publish.borrow().clone();
                    publish.send_replace(Arc::new(AircraftSnapshot {
                        aircraft: old.aircraft.clone(),
                        global: global.samples.status.clone(),
                        local: local.samples.status.clone(),
                        error: Some(Arc::new(error)),
                        valid_until: old.valid_until,
                    }));
                    expires = Some(Instant::now() + FAILURE_RETRY);
                }
            }
            repaint();
            dirty = false;
            continue;
        }
        let wait = async {
            match deadline {
                Some(time) => tokio::time::sleep_until(time.into()).await,
                None => std::future::pending().await,
            }
        };
        tokio::select! {
            changed = queries.changed() => {
                if changed.is_err() { return; }
                let next = *queries.borrow_and_update();
                dirty |= next.enabled && !query.enabled;
                query = next;
            }
            result = receive(&mut global) => { global.finish(result); dirty = true; }
            result = receive(&mut local) => { local.finish(result); dirty = true; }
            () = wait => {}
        }
    }
}
