//! Bounded live telemetry; the desktop only swaps immutable snapshots.
#[cfg(test)]
mod benchmarks;
mod decode;
mod snapshot;
#[cfg(test)]
mod tests;
mod worker;
use std::{sync::Arc, time::Instant};
use tokio::sync::watch;

const MAX_POSITION_AGE_SECS: f64 = 120.0;

#[derive(Debug, Clone)]
pub struct Aircraft {
    pub hex: Arc<str>,
    pub callsign: Arc<str>,
    pub latitude: f64,
    pub longitude: f64,
    pub heading_deg: f64,
    pub speed_knots: Option<f64>,
    pub altitude_feet: Option<f64>,
    pub observed_unix: f64,
}

#[derive(Clone, Default)]
pub struct ProviderStatus {
    pub loaded: Option<Instant>,
    pub attempted: bool,
    pub error: Option<Arc<crate::Error>>,
}

#[derive(Default)]
pub struct AircraftSnapshot {
    pub aircraft: Arc<[Aircraft]>,
    pub global: ProviderStatus,
    pub local: ProviderStatus,
    pub error: Option<Arc<crate::Error>>,
    pub valid_until: Option<Instant>,
}

#[derive(Clone, Copy, Default, PartialEq)]
pub struct AircraftQuery {
    pub enabled: bool,
    pub latitude: f64,
    pub longitude: f64,
    pub altitude_km: f64,
}

pub struct LiveAircraft {
    control: watch::Sender<AircraftQuery>,
    updates: watch::Receiver<Arc<AircraftSnapshot>>,
    latest: Arc<AircraftSnapshot>,
    stopped: bool,
}
impl LiveAircraft {
    pub fn new(
        runtime: &tokio::runtime::Handle,
        repaint: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self::with_endpoints(runtime, repaint, worker::Endpoints::default())
    }

    fn with_endpoints(
        runtime: &tokio::runtime::Handle,
        repaint: impl Fn() + Send + Sync + 'static,
        endpoints: worker::Endpoints,
    ) -> Self {
        let (control, queries) = watch::channel(AircraftQuery::default());
        let latest = Arc::new(AircraftSnapshot::default());
        let (publish, updates) = watch::channel(latest.clone());
        runtime.spawn(worker::run(queries, publish, repaint, endpoints));
        Self {
            control,
            updates,
            latest,
            stopped: false,
        }
    }

    pub fn update(&mut self, query: AircraftQuery) {
        let enabling = query.enabled && !self.control.borrow().enabled;
        match self.updates.has_changed() {
            Ok(true) => {
                let latest = self.updates.borrow_and_update().clone();
                self.latest = latest;
            }
            Ok(false) => {}
            Err(_) if !self.stopped => {
                self.stopped = true;
                self.latest = Arc::new(AircraftSnapshot {
                    aircraft: self.latest.aircraft.clone(),
                    global: self.latest.global.clone(),
                    local: self.latest.local.clone(),
                    error: Some(Arc::new(
                        "Aircraft service stopped before publishing its result".into(),
                    )),
                    valid_until: self.latest.valid_until,
                });
            }
            Err(_) => {}
        }
        if enabling
            && self
                .latest
                .valid_until
                .is_some_and(|deadline| Instant::now() >= deadline)
        {
            // Revalidate expired cached sources off-thread before displaying them again.
            // Clear before sending the query so a fast completion cannot be overwritten.
            self.latest = Arc::new(AircraftSnapshot {
                global: self.latest.global.clone(),
                local: self.latest.local.clone(),
                error: self.latest.error.clone(),
                ..Default::default()
            });
        }
        self.control.send_if_modified(|previous| {
            if *previous == query {
                false
            } else {
                *previous = query;
                true
            }
        });
    }

    pub fn snapshot(&self) -> Arc<AircraftSnapshot> {
        self.latest.clone()
    }
}
