//! One startup check and click-to-install updates, independent of the desktop UI.
mod release;
#[cfg(test)]
mod tests;
use release::{SelectedRelease, check, install};
use std::sync::Arc;
use tokio::sync::watch;

pub const RELEASES_URL: &str = "https://github.com/therichronicles/EarthCraft/releases/latest";

#[derive(Clone, Debug, PartialEq)]
pub enum UpdateState {
    Idle,
    Checking,
    UpToDate,
    Available {
        version: String,
    },
    Downloading {
        version: String,
        downloaded: u64,
        total: Option<u64>,
    },
    Installing {
        version: String,
    },
    ReadyToRestart {
        version: String,
    },
    Failed {
        operation: UpdateOperation,
        message: String,
    },
    Unsupported {
        version: String,
        reason: String,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateOperation {
    Check,
    Install,
}

#[derive(Clone)]
struct Snapshot {
    state: UpdateState,
    selected: Option<SelectedRelease>,
}

pub struct UpdateService {
    runtime: tokio::runtime::Handle,
    repaint: Arc<dyn Fn() + Send + Sync>,
    sender: watch::Sender<Snapshot>,
    receiver: watch::Receiver<Snapshot>,
    state: UpdateState,
    selected: Option<SelectedRelease>,
    disabled: bool,
    manual_reason: Option<String>,
}
impl UpdateService {
    pub fn new(
        runtime: &tokio::runtime::Handle,
        repaint: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        let mode = std::env::var("EARTHCRAFT_UPDATE_MODE").unwrap_or_default();
        let executable = std::env::current_exe().ok();
        let development_build = cfg!(debug_assertions)
            || executable
                .as_deref()
                .is_some_and(release::is_development_path);
        Self::configured(
            runtime,
            Arc::new(repaint),
            development_build || mode == "off",
            if mode == "manual" {
                Some("This installation uses manual or package-manager updates.".into())
            } else {
                executable
                    .as_deref()
                    .and_then(release::manual_install_reason)
            },
        )
    }
    fn configured(
        runtime: &tokio::runtime::Handle,
        repaint: Arc<dyn Fn() + Send + Sync>,
        disabled: bool,
        manual_reason: Option<String>,
    ) -> Self {
        let (sender, receiver) = watch::channel(Snapshot {
            state: UpdateState::Idle,
            selected: None,
        });
        Self {
            runtime: runtime.clone(),
            repaint,
            sender,
            receiver,
            state: UpdateState::Idle,
            selected: None,
            disabled,
            manual_reason,
        }
    }
    pub fn state(&self) -> &UpdateState {
        &self.state
    }
    /// Restore the installed state if launching the replacement failed after shutdown.
    pub fn resume_installed(&mut self, version: &str) {
        if self.state == UpdateState::Idle {
            self.state = UpdateState::ReadyToRestart {
                version: version.into(),
            };
        }
    }
    /// Called from application logic after window initialization. Never starts twice.
    pub fn start_once(&mut self) {
        if !self.disabled && self.state == UpdateState::Idle {
            self.start_check();
        }
    }
    pub fn poll(&mut self) {
        if self.receiver.has_changed().unwrap_or(false) {
            let snapshot = self.receiver.borrow_and_update().clone();
            self.state = snapshot.state;
            self.selected = snapshot.selected;
        }
    }
    pub fn activate(&mut self) {
        if self.disabled {
            return;
        }
        match self.state {
            UpdateState::Available { .. }
            | UpdateState::Failed {
                operation: UpdateOperation::Install,
                ..
            } => self.start_install(),
            UpdateState::Failed {
                operation: UpdateOperation::Check,
                ..
            } => self.start_check(),
            _ => {}
        }
    }
    fn start_check(&mut self) {
        self.state = UpdateState::Checking;
        let sender = self.sender.clone();
        let repaint = self.repaint.clone();
        let manual_reason = self.manual_reason.clone();
        self.runtime.spawn(async move {
            let snapshot = match check().await {
                Ok(Some(selected)) => Snapshot {
                    state: if let Some(reason) = manual_reason {
                        UpdateState::Unsupported {
                            version: selected.version.clone(),
                            reason,
                        }
                    } else {
                        UpdateState::Available {
                            version: selected.version.clone(),
                        }
                    },
                    selected: Some(selected),
                },
                Ok(None) => Snapshot {
                    state: UpdateState::UpToDate,
                    selected: None,
                },
                Err(error) => Snapshot {
                    state: UpdateState::Failed {
                        operation: UpdateOperation::Check,
                        message: format!("Update check failed: {error}"),
                    },
                    selected: None,
                },
            };
            sender.send_replace(snapshot);
            repaint();
        });
    }
    fn start_install(&mut self) {
        let Some(selected) = self.selected.clone() else {
            return;
        };
        self.state = UpdateState::Downloading {
            version: selected.version.clone(),
            downloaded: 0,
            total: None,
        };
        let sender = self.sender.clone();
        let repaint = self.repaint.clone();
        self.runtime.spawn(async move {
            let result = install(&selected, sender.clone(), repaint.clone()).await;
            let state = match result {
                Ok(true) => UpdateState::ReadyToRestart { version: selected.version.clone() },
                Ok(false) => UpdateState::Failed { operation: UpdateOperation::Install, message: "The updater did not install the selected release. Open Releases to update manually.".into() },
                Err(error @ self_update::Error::InstallPathNotWritable { .. }) => UpdateState::Unsupported { version: selected.version.clone(), reason: format!("Update installation is protected: {error}. Open Releases or use your package manager to update.") },
                Err(error) => UpdateState::Failed { operation: UpdateOperation::Install, message: format!("Update installation failed: {error}. Open Releases to update manually if this installation is protected.") },
            };
            sender.send_replace(Snapshot { state, selected: Some(selected) });
            repaint();
        });
    }
}
