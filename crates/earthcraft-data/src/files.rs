//! Ordered, bounded KML/PNG jobs; native dialogs and applying results stay in app.
use earthcraft_core::{language::Lang, placemark::Placemark};
use std::{
    path::PathBuf,
    sync::{mpsc, Arc},
};
const PENDING_FILE_JOBS: usize = 4;

pub enum FileOutcome {
    KmlLoaded {
        path: PathBuf,
        placemarks: Vec<Placemark>,
    },
    KmlSaved {
        path: PathBuf,
    },
    ScreenshotSaved {
        path: PathBuf,
    },
}
pub enum FileKind {
    OpenKml,
    SaveKml,
    Screenshot,
}
pub struct FileCompletion {
    pub kind: FileKind,
    pub result: Result<FileOutcome, crate::Error>,
}
enum Job {
    Open(PathBuf, Lang),
    Save(PathBuf, Vec<Placemark>),
    Screenshot(PathBuf, u32, u32, Arc<Vec<u8>>),
}
impl Job {
    fn kind(&self) -> FileKind {
        match self {
            Self::Open(..) => FileKind::OpenKml,
            Self::Save(..) => FileKind::SaveKml,
            Self::Screenshot(..) => FileKind::Screenshot,
        }
    }
    fn run(self) -> Result<FileOutcome, crate::Error> {
        match self {
            Self::Open(path, lang) => {
                let placemarks = crate::kml::load_kml(&path.to_string_lossy(), lang)?;
                Ok(FileOutcome::KmlLoaded { path, placemarks })
            }
            Self::Save(path, marks) => {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                crate::kml::save_kml(&path.to_string_lossy(), &marks)?;
                Ok(FileOutcome::KmlSaved { path })
            }
            Self::Screenshot(path, width, height, pixels) => {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                image::save_buffer(&path, &pixels, width, height, image::ColorType::Rgba8)?;
                Ok(FileOutcome::ScreenshotSaved { path })
            }
        }
    }
}
pub struct FileJobs {
    sender: tokio::sync::mpsc::Sender<Job>,
}
impl FileJobs {
    pub fn new(
        runtime: &tokio::runtime::Handle,
        repaint: impl Fn() + Send + Sync + 'static,
    ) -> (Self, mpsc::Receiver<FileCompletion>) {
        let (sender, mut jobs) = tokio::sync::mpsc::channel::<Job>(PENDING_FILE_JOBS);
        let (results, receiver) = mpsc::channel();
        let repaint: Arc<dyn Fn() + Send + Sync> = Arc::new(repaint);
        runtime.spawn(async move {
            while let Some(job) = jobs.recv().await {
                let kind = job.kind();
                let result = crate::cpu::run(crate::cpu::Service::Files, move || job.run()).await;
                if results.send(FileCompletion { kind, result }).is_err() {
                    return;
                }
                repaint();
            }
        });
        (Self { sender }, receiver)
    }
    fn reserve(&self) -> Result<tokio::sync::mpsc::Permit<'_, Job>, crate::Error> {
        self.sender.try_reserve().map_err(|error| match error {
            tokio::sync::mpsc::error::TrySendError::Full(_) => {
                crate::Error::from("file job queue is full")
            }
            tokio::sync::mpsc::error::TrySendError::Closed(_) => {
                crate::Error::from("file service is unavailable")
            }
        })
    }
    pub fn open_kml(&self, path: PathBuf, lang: Lang) -> Result<(), crate::Error> {
        self.reserve()?.send(Job::Open(path, lang));
        Ok(())
    }
    pub fn save_kml(&self, path: PathBuf, marks: &[Placemark]) -> Result<(), crate::Error> {
        self.reserve()?.send(Job::Save(path, marks.to_vec()));
        Ok(())
    }
    pub fn screenshot(
        &self,
        path: PathBuf,
        width: u32,
        height: u32,
        pixels: &Arc<Vec<u8>>,
    ) -> Result<(), crate::Error> {
        self.reserve()?
            .send(Job::Screenshot(path, width, height, pixels.clone()));
        Ok(())
    }
}

#[cfg(test)]
mod tests;
