use super::App;
use earthcraft_core::{
    camera::{Basis, Cam},
    math::{ll_to_v3, now_unix, subsolar},
    VERTICAL_FOV_HALF_TAN,
};
use earthcraft_core::{METERS_PER_KM, METERS_PER_LATITUDE_DEGREE};
use earthcraft_data::tiles::Provider;
use earthcraft_render::RenderCfg;
use eframe::egui;
use std::sync::Arc;
mod worker;
use worker::{RenderRequest, RenderWorker};

pub(super) struct Renderer {
    worker: RenderWorker,
    displayed: Option<(Cam, Basis)>,
}

impl Renderer {
    pub(super) fn new(cache: Arc<earthcraft_data::tiles::TileCache>, ctx: egui::Context) -> Self {
        Self {
            worker: RenderWorker::new(
                move |request| {
                    earthcraft_render::render_frame(
                        request.width,
                        request.height,
                        &request.basis,
                        &request.cfg,
                        &cache,
                    )
                },
                move || ctx.request_repaint(),
            )
            .expect("start frame renderer"),
            displayed: None,
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
const GRID_STEPS_DEG: [f64; 12] = [
    30.0, 10.0, 5.0, 2.0, 1.0, 0.5, 0.2, 0.1, 0.05, 0.01, 0.005, 0.001,
];
const TARGET_GRID_DIVISIONS: f64 = 5.0;
const MIN_GRID_STEP_DEG: f64 = 0.001;

impl App {
    pub(super) fn request_render(&mut self, w: usize, h: usize, aspect: f64, presentation: u64) {
        let basis = self.cam.basis(aspect);
        let mpp = self.cam.alt * METERS_PER_KM * 2.0 * VERTICAL_FOV_HALF_TAN / h as f64;
        let span = (mpp * w as f64 / METERS_PER_LATITUDE_DEGREE).min(180.0);

        let grid_step = GRID_STEPS_DEG
            .iter()
            .copied()
            .find(|s| *s <= span / TARGET_GRID_DIVISIONS)
            .unwrap_or(MIN_GRID_STEP_DEG);
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
            Some(t) => t.set(img, egui::TextureOptions::LINEAR),
            None => {
                self.texture = Some(ctx.load_texture("globe", img, egui::TextureOptions::LINEAR))
            }
        }
        self.image = Some((w, h, buf));
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
