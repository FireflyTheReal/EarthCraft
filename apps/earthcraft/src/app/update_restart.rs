//! Save through the existing file service, then use the ordinary window close.
use super::App;
use earthcraft_data::files::{FileKind, FileOutcome};
use eframe::egui;
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Default)]
pub(crate) struct RestartRequest {
    pub(crate) requested: AtomicBool,
    pub(crate) error: Mutex<Option<String>>,
    pub(crate) planet: Mutex<Option<PathBuf>>,
    pub(crate) installed_version: Mutex<Option<String>>,
    pub(crate) settings: Mutex<Option<earthcraft_data::settings::Settings>>,
}

pub(super) struct UpdateRestart {
    pub(super) request: Arc<RestartRequest>,
    saving: bool,
    kml: Option<PathBuf>,
    planet: Option<PathBuf>,
    pub(super) error: Option<String>,
}
impl UpdateRestart {
    pub(super) fn new(request: Arc<RestartRequest>) -> Self {
        let error = request.error.lock().unwrap().take();
        Self {
            request,
            saving: false,
            kml: None,
            planet: None,
            error,
        }
    }
    pub(super) fn is_saving(&self) -> bool {
        self.saving
    }
    pub(super) fn file_completed(
        &mut self,
        kind: FileKind,
        result: &Result<FileOutcome, earthcraft_data::Error>,
    ) {
        if !self.saving {
            return;
        }
        match result {
            Ok(FileOutcome::KmlSaved { path }) if self.kml.as_ref() == Some(path) => {
                self.kml = None
            }
            Ok(FileOutcome::PlanetSaved { path }) if self.planet.as_ref() == Some(path) => {
                self.planet = None
            }
            Err(error) if matches!(kind, FileKind::SaveKml | FileKind::SavePlanet) => {
                self.saving = false;
                self.error = Some(format!("Save before restart failed: {error}"));
            }
            _ => {}
        }
    }
}
impl App {
    pub(super) fn request_update_restart(&mut self, ctx: &egui::Context) {
        if !matches!(
            self.updater.state(),
            earthcraft_data::updater::UpdateState::ReadyToRestart { .. }
        ) || self.update_restart.saving
        {
            return;
        }
        if self.file_jobs.is_busy() {
            self.update_restart.error = Some(
                "Wait for file operations to finish, then click Restart to update again.".into(),
            );
            return;
        }
        // Choose both destinations first. Canceling either leaves the app running.
        let kml = if self.placemarks.is_empty() {
            None
        } else {
            let Some(path) = self.choose_kml_save_path() else {
                return;
            };
            match std::path::absolute(path) {
                Ok(path) => Some(path),
                Err(error) => {
                    self.update_restart.error =
                        Some(format!("Unable to resolve save path: {error}"));
                    return;
                }
            }
        };
        let planet = if self.explorer.has_custom_document() {
            let Some(mut path) = rfd::FileDialog::new()
                .add_filter("EarthCraft planet", &["earthplanet"])
                .set_file_name("my-planet.earthplanet")
                .save_file()
            else {
                return;
            };
            if path.extension().is_none() {
                path.set_extension("earthplanet");
            }
            Some(path)
        } else {
            None
        };
        let result = (|| -> Result<(), earthcraft_data::Error> {
            if let Some(path) = &kml {
                self.file_jobs.save_kml(path.clone(), &self.placemarks)?;
                self.kml_session.submitted();
            }
            if let Some(path) = &planet {
                self.file_jobs
                    .save_planet(self.explorer.save_snapshot(path.clone()))?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.update_restart.error = Some(format!("Save before restart failed: {error}"));
            return;
        }
        *self.update_restart.request.planet.lock().unwrap() = planet.clone();
        self.update_restart.kml = kml;
        self.update_restart.planet = planet;
        self.update_restart.saving = true;
        self.update_restart.error = None;
        ctx.request_repaint();
    }
    pub(super) fn poll_update_restart(&mut self, ctx: &egui::Context) {
        if self.update_restart.saving
            && self.update_restart.kml.is_none()
            && self.update_restart.planet.is_none()
            && !self.file_jobs.is_busy()
        {
            self.settings_writer.settings(&self.settings);
            *self.update_restart.request.settings.lock().unwrap() = Some(self.settings.clone());
            if let earthcraft_data::updater::UpdateState::ReadyToRestart { version } =
                self.updater.state()
            {
                *self
                    .update_restart
                    .request
                    .installed_version
                    .lock()
                    .unwrap() = Some(version.clone());
            }
            self.update_restart
                .request
                .requested
                .store(true, Ordering::Release);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restart_requires_matching_save_success_and_stays_open_on_failure() {
        let mut restart = UpdateRestart::new(Arc::new(RestartRequest::default()));
        restart.saving = true;
        restart.kml = Some("chosen.kml".into());
        restart.file_completed(
            FileKind::SaveKml,
            &Ok(FileOutcome::KmlSaved {
                path: "other.kml".into(),
            }),
        );
        assert!(restart.kml.is_some());
        restart.file_completed(
            FileKind::SaveKml,
            &Err(earthcraft_data::Error::from("disk full")),
        );
        assert!(!restart.saving);
        assert!(restart.error.unwrap().contains("disk full"));
        assert!(!restart.request.requested.load(Ordering::Acquire));
    }
}
