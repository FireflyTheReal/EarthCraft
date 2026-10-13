use super::App;
use earthcraft_core::{
    math::{ll_to_v3, now_unix, subsolar},
    VERTICAL_FOV_HALF_TAN,
};
use earthcraft_core::{METERS_PER_KM, METERS_PER_LATITUDE_DEGREE};
use earthcraft_data::tiles::Provider;
use earthcraft_render::RenderCfg;
use eframe::egui;
const GRID_STEPS_DEG: [f64; 12] = [
    30.0, 10.0, 5.0, 2.0, 1.0, 0.5, 0.2, 0.1, 0.05, 0.01, 0.005, 0.001,
];
const TARGET_GRID_DIVISIONS: f64 = 5.0;
const MIN_GRID_STEP_DEG: f64 = 0.001;

impl App {
    pub(super) fn render(&mut self, w: usize, h: usize, aspect: f64, ctx: &egui::Context) {
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
        let buf = earthcraft_render::render_frame(w, h, &basis, &cfg, &self.cache);

        let img = egui::ColorImage::from_rgba_unmultiplied([w, h], &buf);
        match &mut self.texture {
            Some(t) => t.set(img, egui::TextureOptions::LINEAR),
            None => {
                self.texture = Some(ctx.load_texture("globe", img, egui::TextureOptions::LINEAR))
            }
        }
        self.image = Some((w, h, buf));
    }
}
