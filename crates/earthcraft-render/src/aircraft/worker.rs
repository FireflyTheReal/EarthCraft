//! Coalesced aircraft projection; displayed-frame geometry uses the same preparer.
#[cfg(test)]
mod tests;
use super::{ProjectedFlight, project_flights};
use earthcraft_core::camera::Basis;
use earthcraft_data::aircraft::AircraftSnapshot;
use emath::Rect;
use std::sync::{Arc, Condvar, Mutex};

const LABEL_MAX_ALTITUDE_KM: f64 = 650.0;
const LABEL_MIN_SEPARATION_POINTS: f32 = 46.0;
const MAX_LABELS: usize = 110;
const MAX_PRESENTATION_AGE_SECS: f64 = 2.0;

#[derive(Clone, Copy, PartialEq)]
pub struct ProjectionView {
    pub basis: Basis,
    pub rect: Rect,
    pub altitude_km: f64,
    pub map_scale: f32,
    pub pixels_per_point: f32,
}
#[derive(Clone)]
pub struct ProjectionInput {
    pub view: ProjectionView,
    pub data: Arc<AircraftSnapshot>,
    pub unix_secs: f64,
}
impl ProjectionInput {
    pub fn same_request(&self, other: &Self) -> bool {
        self.view == other.view
            && Arc::ptr_eq(&self.data, &other.data)
            && self.unix_secs.floor() == other.unix_secs.floor()
    }
}

pub struct AircraftGeometry {
    pub view: ProjectionView,
    pub data: Arc<AircraftSnapshot>,
    pub unix_secs: f64,
    pub flights: Vec<ProjectedFlight>,
    pub labels: Vec<usize>,
}
impl AircraftGeometry {
    pub fn is_recent(&self, unix_secs: f64) -> bool {
        (0.0..=MAX_PRESENTATION_AGE_SECS).contains(&(unix_secs - self.unix_secs))
    }
    pub fn matches(&self, view: ProjectionView, data: &Arc<AircraftSnapshot>) -> bool {
        self.view == view && Arc::ptr_eq(&self.data, data)
    }
}

#[derive(Default)]
pub struct AircraftPreparer {
    latest: Option<Arc<AircraftGeometry>>,
}
impl AircraftPreparer {
    pub fn prepare(&mut self, input: ProjectionInput) -> Arc<AircraftGeometry> {
        if let Some(geometry) = &self.latest
            && geometry.matches(input.view, &input.data)
            && geometry.unix_secs.floor() == input.unix_secs.floor()
        {
            return geometry.clone();
        }
        #[cfg(feature = "profiling")]
        let _profile = earthcraft_data::profiling::Span::new("aircraft.project");
        let flights = project_flights(
            &input.view.basis,
            input.view.rect,
            &input.data.aircraft,
            input.unix_secs,
            input.view.map_scale,
        );
        let mut labels: Vec<usize> = Vec::new();
        if input.view.altitude_km <= LABEL_MAX_ALTITUDE_KM {
            for (index, flight) in flights.iter().enumerate() {
                if labels.len() == MAX_LABELS {
                    break;
                }
                if labels.iter().all(|other| {
                    (flights[*other].position - flight.position).length()
                        > LABEL_MIN_SEPARATION_POINTS * input.view.map_scale
                }) {
                    labels.push(index);
                }
            }
        }
        let geometry = Arc::new(AircraftGeometry {
            view: input.view,
            data: input.data,
            unix_secs: input.unix_secs,
            flights,
            labels,
        });
        self.latest = Some(geometry.clone());
        geometry
    }
}

#[derive(Default)]
struct Mailbox {
    pending: Option<ProjectionInput>,
    completed: Option<Arc<AircraftGeometry>>,
    stopped: bool,
}
#[derive(Default)]
struct Shared {
    mailbox: Mutex<Mailbox>,
    wake: Condvar,
}
pub struct AircraftWorker {
    shared: Arc<Shared>,
}
impl AircraftWorker {
    pub fn new(repaint: impl Fn() + Send + 'static) -> std::io::Result<Self> {
        let shared = Arc::new(Shared::default());
        let worker = shared.clone();
        std::thread::Builder::new()
            .name("earthcraft-aircraft".into())
            .spawn(move || {
                let mut preparer = AircraftPreparer::default();
                loop {
                    let input = {
                        let mut state = worker.mailbox.lock().unwrap();
                        while state.pending.is_none() && !state.stopped {
                            state = worker.wake.wait(state).unwrap();
                        }
                        if state.stopped {
                            return;
                        }
                        state.pending.take().unwrap()
                    };
                    let geometry = preparer.prepare(input);
                    let mut state = worker.mailbox.lock().unwrap();
                    if state.stopped {
                        return;
                    }
                    let previous = state.completed.replace(geometry);
                    drop(state);
                    drop(previous);
                    repaint();
                }
            })?;
        Ok(Self { shared })
    }
    pub fn submit(&self, input: ProjectionInput) {
        let previous = self.shared.mailbox.lock().unwrap().pending.replace(input);
        drop(previous);
        self.shared.wake.notify_one();
    }
    pub fn take_completed(&self) -> Option<Arc<AircraftGeometry>> {
        self.shared.mailbox.lock().unwrap().completed.take()
    }
}
impl Drop for AircraftWorker {
    fn drop(&mut self) {
        let mut state = self.shared.mailbox.lock().unwrap();
        state.stopped = true;
        let pending = state.pending.take();
        let completed = state.completed.take();
        drop(state);
        self.shared.wake.notify_one();
        drop((pending, completed));
    }
}
