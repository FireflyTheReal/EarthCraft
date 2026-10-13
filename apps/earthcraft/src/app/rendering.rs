use super::App;
use earthcraft_core::{
    camera::{Basis, Cam},
    math::{ll_to_v3, now_unix, subsolar},
};
use earthcraft_data::tiles::Provider;
use earthcraft_render::worker::{RenderRequest, RenderWorker};
use earthcraft_render::RenderCfg;
use eframe::egui;
use std::sync::Arc;

pub(super) struct Renderer {
    worker: RenderWorker,
    displayed: Option<(Cam, Basis)>,
    waiting: Vec<earthcraft_data::tiles::TileKey>,
    required: Vec<earthcraft_data::tiles::TileKey>,
    retry_at: Option<std::time::Instant>,
}

impl Renderer {
    pub(super) fn new(cache: Arc<earthcraft_data::tiles::TileCache>, ctx: egui::Context) -> Self {
        let admission_cache = cache.clone();
        Self {
            worker: RenderWorker::new(
                move |request, buffer| {
                    earthcraft_render::render_view_into(
                        request.width,
                        request.height,
                        &request.basis,
                        &request.cfg,
                        &cache,
                        buffer,
                    )
                },
                move || ctx.request_repaint(),
                move |frame| {
                    admission_cache.prioritize_view(&frame.required_tiles());
                },
            )
            .expect("start frame renderer"),
            displayed: None,
            waiting: Vec::new(),
            required: Vec::new(),
            retry_at: None,
        }
    }
    pub(super) fn tiles_changed(&self, cache: &earthcraft_data::tiles::TileCache) -> bool {
        self.waiting
            .iter()
            .any(|&key| matches!(cache.lookup(key), earthcraft_data::tiles::Lookup::Ready(_)))
    }
    pub(super) fn refill_requests(
        &mut self,
        cache: &earthcraft_data::tiles::TileCache,
        changed: bool,
        ctx: &egui::Context,
    ) {
        let now = std::time::Instant::now();
        if changed || self.retry_at.is_some_and(|deadline| now >= deadline) {
            cache.prioritize_view(&self.required);
            self.retry_at = cache.retry_delay(&self.required).map(|delay| now + delay);
        }
        if let Some(deadline) = self.retry_at {
            ctx.request_repaint_after(deadline.saturating_duration_since(now));
        }
    }
    pub(super) fn is_idle(&self) -> bool {
        self.worker.is_idle()
    }
    pub(super) fn picking_basis(&self, fallback: Basis) -> Basis {
        displayed_basis(self.displayed, fallback)
    }
    pub(super) fn displayed_view(&self) -> Option<(Cam, Basis)> {
        self.displayed
    }
}

impl App {
    pub(super) fn request_render(&mut self, w: usize, h: usize, aspect: f64, presentation: u64) {
        let basis = self.cam.basis(aspect);
        let grid_step = earthcraft_render::grid_step(self.cam.alt, w, h);
        let sun = if self.show_sun {
            let (la, lo) = subsolar(now_unix());
            Some(ll_to_v3(la, lo))
        } else {
            None
        };
        let prov = self.effective_provider();
        let rel = if prov == Provider::Wayback {
            self.wayback_idx as u16
        } else {
            0
        };
        let cfg = RenderCfg {
            provider: prov,
            rel,
            grid: self.show_grid,
            grid_step,
            atmo: self.show_atmo,
            sun,
        };
        self.renderer.worker.submit(RenderRequest {
            width: w,
            height: h,
            cam: self.cam,
            basis,
            cfg,
            presentation,
        });
    }

    pub(super) fn poll_render(&mut self, presentation: u64, ctx: &egui::Context) {
        let Some(frame) = self.renderer.worker.take_completed(presentation) else {
            return;
        };
        let (w, h, buf) = (frame.request.width, frame.request.height, frame.pixels);

        let img = egui::ColorImage::from_rgba_unmultiplied([w, h], &buf);
        match &mut self.texture {
            Some(t) if t.size() == [w, h] => {
                t.set_partial([0, 0], img, egui::TextureOptions::LINEAR)
            }
            Some(t) => t.set(img, egui::TextureOptions::LINEAR),
            None => {
                self.texture = Some(ctx.load_texture("globe", img, egui::TextureOptions::LINEAR))
            }
        }
        if let Some((_, _, previous)) = self.image.replace((w, h, buf)) {
            self.renderer.worker.recycle(previous);
        }
        self.renderer.waiting = frame.waiting;
        self.renderer.required = frame.required;
        self.renderer.retry_at = None;
        // Recheck dependencies even if a tile finished before frame publication.
        self.last_gen = u64::MAX;
        self.renderer.displayed = Some((frame.request.cam, frame.request.basis));
    }
}

// Input must describe the same globe as the frame being presented.
fn displayed_basis(displayed: Option<(Cam, Basis)>, fallback: Basis) -> Basis {
    displayed.map_or(fallback, |(_, basis)| basis)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn picking_tracks_displayed_camera_while_new_view_is_pending() {
        let shown = Cam {
            lat: 20.0,
            lon: 30.0,
            heading: 0.0,
            alt: 14000.0,
        };
        let requested = Cam { lon: 90.0, ..shown };
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let shown_basis = shown.basis(4.0 / 3.0);
        let requested_basis = requested.basis(4.0 / 3.0);
        let pick = displayed_basis(Some((shown, shown_basis)), requested_basis)
            .pick(rect, rect.center())
            .unwrap();
        assert!((pick.0 - shown.lat).abs() < 1e-8);
        assert!((pick.1 - shown.lon).abs() < 1e-8);
        assert_eq!(
            displayed_basis(None, requested_basis).pick(rect, rect.center()),
            requested_basis.pick(rect, rect.center())
        );
    }
}
