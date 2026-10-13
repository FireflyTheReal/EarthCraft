//! One active raster job, one replaceable pending view, and one completed frame.
use crate::RenderCfg;
use earthcraft_core::camera::{Basis, Cam};
use std::sync::{Arc, Condvar, Mutex};

pub struct RenderRequest {
    pub width: usize,
    pub height: usize,
    pub cam: Cam,
    pub basis: Basis,
    pub cfg: RenderCfg,
    // Camera motion may lag by one job; provider/style/size changes must not.
    pub presentation: u64,
}

pub struct RenderedFrame {
    pub request: RenderRequest,
    pub pixels: Vec<u8>,
    pub waiting: Vec<earthcraft_data::tiles::TileKey>,
    pub required: Vec<earthcraft_data::tiles::TileKey>,
}

#[derive(Default)]
struct Mailbox {
    pending: Option<RenderRequest>,
    completed: Option<RenderedFrame>,
    active: bool,
    stopped: bool,
    spare: Option<Vec<u8>>,
}

#[derive(Default)]
struct Shared {
    mailbox: Mutex<Mailbox>,
    wake: Condvar,
}

pub struct RenderWorker {
    shared: Arc<Shared>,
}

impl RenderWorker {
    pub fn new(
        render: impl Fn(&RenderRequest, Vec<u8>) -> crate::RenderedView + Send + 'static,
        repaint: impl Fn() + Send + 'static,
        admit: impl Fn(&crate::RenderedView) + Send + 'static,
    ) -> std::io::Result<Self> {
        let shared = Arc::new(Shared::default());
        let worker = shared.clone();
        std::thread::Builder::new()
            .name("earthcraft-frame".into())
            .spawn(move || loop {
                let (request, buffer) = {
                    let mut mailbox = worker.mailbox.lock().unwrap();
                    while !mailbox.stopped && mailbox.pending.is_none() {
                        mailbox = worker.wake.wait(mailbox).unwrap();
                    }
                    if mailbox.stopped {
                        return;
                    }
                    mailbox.active = true;
                    (
                        mailbox.pending.take().unwrap(),
                        mailbox.spare.take().unwrap_or_default(),
                    )
                };
                // Rasterization holds no mailbox lock. Admission is synchronized below.
                let frame = render(&request, buffer);
                let mut mailbox = worker.mailbox.lock().unwrap();
                mailbox.active = false;
                if mailbox.stopped {
                    return;
                }
                let obsolete = mailbox
                    .pending
                    .as_ref()
                    .is_some_and(|next| next.presentation != request.presentation);
                if !obsolete {
                    // Admission is short and bounded; synchronize it with pending
                    // presentation changes so incompatible rasters cannot fetch tiles.
                    admit(&frame);
                    if let Some(previous) = mailbox.completed.take() {
                        mailbox.spare = Some(previous.pixels);
                    }
                    let required = frame.required_tiles();
                    mailbox.completed = Some(RenderedFrame {
                        request,
                        pixels: frame.pixels,
                        waiting: frame.waiting,
                        required,
                    });
                } else {
                    mailbox.spare = Some(frame.pixels);
                }
                drop(mailbox);
                repaint();
            })?;
        Ok(Self { shared })
    }

    pub fn recycle(&self, buffer: Vec<u8>) {
        self.shared.mailbox.lock().unwrap().spare = Some(buffer);
    }

    pub fn submit(&self, request: RenderRequest) {
        self.shared.mailbox.lock().unwrap().pending = Some(request);
        self.shared.wake.notify_one();
    }

    pub fn take_completed(&self, presentation: u64) -> Option<RenderedFrame> {
        self.shared
            .mailbox
            .lock()
            .unwrap()
            .completed
            .take()
            .filter(|frame| frame.request.presentation == presentation)
    }

    pub fn is_idle(&self) -> bool {
        let mailbox = self.shared.mailbox.lock().unwrap();
        !mailbox.active && mailbox.pending.is_none()
    }
}

impl Drop for RenderWorker {
    fn drop(&mut self) {
        let mut mailbox = self.shared.mailbox.lock().unwrap();
        mailbox.stopped = true;
        mailbox.pending = None;
        mailbox.completed = None;
        drop(mailbox);
        self.shared.wake.notify_one();
        // Closing the window never waits for a raster job. The coordinator exits
        // after the current job, releasing its cache and repaint callback.
    }
}

#[cfg(test)]
mod tests;
