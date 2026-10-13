use super::App;
use super::{INITIAL_CAMERA, MAX_FRAME_DELTA_SECS, PAN_SPEED_POINTS_PER_SECOND};
use earthcraft_core::METERS_PER_KM;
use earthcraft_core::{
    math::{now_unix, wrap_lon},
    placemark::Placemark,
    VERTICAL_FOV_HALF_TAN,
};
use earthcraft_data::tiles::Provider;
use eframe::egui::{
    self, Align2, Color32, FontId, Key, PointerButton, Pos2, Rect, Sense, Stroke, Vec2,
};
use std::{
    hash::{Hash, Hasher},
    time::{Duration, Instant},
};
const DRAG_HEADING_DEG_PER_POINT: f64 = 0.3;
const SCROLL_ZOOM_RATE_PER_POINT: f64 = 0.0025;
const POINT_FLY_ALTITUDE_RATIO: f64 = 0.35;
const MIN_POINT_FLY_ALTITUDE_KM: f64 = 0.3;
const AUTO_ROTATE_DEG_PER_SECOND: f64 = 6.0;
// Lower resolution while interacting; recover full resolution after the idle delay.
const IDLE_RENDER_DELAY: Duration = Duration::from_millis(250);
const IDLE_RENDER_SCALE: f32 = 1.0;
const ACTIVE_RENDER_SCALE: f32 = 0.4;
const MIN_RENDER_WIDTH_PX: usize = 64;
const MAX_RENDER_WIDTH_PX: usize = 2000;
const MIN_RENDER_HEIGHT_PX: usize = 32;
const SUN_UPDATE_INTERVAL_SECS: f64 = 60.0;
const TILE_REDRAW_INTERVAL: Duration = Duration::from_millis(60);
const ACTIVE_REPAINT_INTERVAL: Duration = Duration::from_millis(260);
const OVERVIEW_WARMUP_POLL_INTERVAL: Duration = Duration::from_millis(100);
const MAX_SCALE_BAR_ALTITUDE_KM: f64 = 3000.0;
const TARGET_SCALE_BAR_WIDTH_POINTS: f64 = 120.0;

impl App {
    pub(super) fn globe_view(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let lg = self.lang;
        let rect = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(rect, Sense::click_and_drag());
        let ppp = ctx.pixels_per_point();
        let aspect = (rect.width() / rect.height()) as f64;
        let shift = ctx.input(|i| i.modifiers.shift);

        // ── input
        let mut interacting = false;
        if resp.dragged_by(PointerButton::Primary) && !shift {
            let d = resp.drag_delta();
            self.fly = None;
            self.pan(d.x as f64, d.y as f64, rect.height());
            interacting = true;
        } else if resp.dragged_by(PointerButton::Secondary)
            || (resp.dragged_by(PointerButton::Primary) && shift)
        {
            self.cam.heading = wrap_lon(
                self.cam.heading - resp.drag_delta().x as f64 * DRAG_HEADING_DEG_PER_POINT,
            );
            interacting = true;
        }
        if resp.hovered() {
            let sc = ctx.input(|i| i.smooth_scroll_delta.y) as f64;
            let zd = ctx.input(|i| i.zoom_delta()) as f64;
            let cur = resp.hover_pos();
            if sc != 0.0 {
                self.fly = None;
                self.zoom_at((-sc * SCROLL_ZOOM_RATE_PER_POINT).exp(), cur, rect, aspect);
                interacting = true;
            }
            if (zd - 1.0).abs() > 1e-4 {
                self.fly = None;
                self.zoom_at(1.0 / zd, cur, rect, aspect);
                interacting = true;
            }
        }
        if !ctx.wants_keyboard_input() {
            let dt = ctx.input(|i| i.unstable_dt).min(MAX_FRAME_DELTA_SECS) as f64;
            let sp = PAN_SPEED_POINTS_PER_SECOND * dt;
            let (mut dx, mut dy) = (0.0, 0.0);
            ctx.input(|i| {
                if i.key_down(Key::ArrowLeft) {
                    dx += sp;
                }
                if i.key_down(Key::ArrowRight) {
                    dx -= sp;
                }
                if i.key_down(Key::ArrowUp) {
                    dy += sp;
                }
                if i.key_down(Key::ArrowDown) {
                    dy -= sp;
                }
            });
            if dx != 0.0 || dy != 0.0 {
                self.fly = None;
                self.pan(dx, dy, rect.height());
                interacting = true;
            }
            let (zin, zout, north) = ctx.input(|i| {
                (
                    i.key_down(Key::Plus) || i.key_down(Key::Equals) || i.key_down(Key::PageUp),
                    i.key_down(Key::Minus) || i.key_down(Key::PageDown),
                    i.key_pressed(Key::N),
                )
            });
            if zin {
                self.zoom((-1.2 * dt).exp());
                interacting = true;
            }
            if zout {
                self.zoom((1.2 * dt).exp());
                interacting = true;
            }
            if north {
                self.cam.heading = 0.0;
            }
        }
        if interacting || resp.dragged() {
            self.touch();
        }

        let basis = self.cam.basis(aspect);
        // ── rendering
        let idle = self.last_interact.elapsed() > IDLE_RENDER_DELAY && !resp.dragged();
        let scale = if idle {
            IDLE_RENDER_SCALE
        } else {
            ACTIVE_RENDER_SCALE
        };
        let w =
            ((rect.width() * ppp * scale) as usize).clamp(MIN_RENDER_WIDTH_PX, MAX_RENDER_WIDTH_PX);
        let h = ((w as f32 * rect.height() / rect.width()) as usize).max(MIN_RENDER_HEIGHT_PX);
        let sun_key = if self.show_sun {
            (now_unix() / SUN_UPDATE_INTERVAL_SECS) as u64
        } else {
            0
        };

        let mut hs = std::collections::hash_map::DefaultHasher::new();
        (
            (
                self.cam.lat.to_bits(),
                self.cam.lon.to_bits(),
                self.cam.heading.to_bits(),
                self.cam.alt.to_bits(),
                w,
                h,
                self.effective_provider().id(),
                self.show_grid,
            ),
            (self.show_atmo, sun_key, self.wayback_idx),
        )
            .hash(&mut hs);
        let state = hs.finish();
        let gen = self.cache.generation();
        let mut presentation = std::collections::hash_map::DefaultHasher::new();
        (
            w,
            h,
            aspect.to_bits(),
            self.effective_provider().id(),
            self.wayback_idx,
            self.show_grid,
            self.show_atmo,
            sun_key,
        )
            .hash(&mut presentation);
        let presentation = presentation.finish();
        self.poll_render(presentation, ctx);

        let pick_basis = self.renderer.picking_basis(basis);
        self.cursor = resp.hover_pos().and_then(|p| pick_basis.pick(rect, p));

        if resp.double_clicked() {
            if let Some((la, lo)) = resp
                .interact_pointer_pos()
                .and_then(|p| pick_basis.pick(rect, p))
            {
                self.fly_to(
                    la,
                    lo,
                    (self.cam.alt * POINT_FLY_ALTITUDE_RATIO).max(MIN_POINT_FLY_ALTITUDE_KM),
                );
            }
        } else if resp.clicked() && self.ruler {
            if let Some(pt) = resp
                .interact_pointer_pos()
                .and_then(|p| pick_basis.pick(rect, p))
            {
                self.ruler_pts.push(pt);
            }
        }
        if resp.secondary_clicked() {
            self.menu_pick = resp
                .interact_pointer_pos()
                .and_then(|p| pick_basis.pick(rect, p));
        }

        let mut need = state != self.last_state
            || (gen != self.last_gen && self.renderer.tiles_changed(&self.cache))
            || (self.texture.is_none() && self.renderer.is_idle());
        if need
            && state == self.last_state
            && self.texture.is_some()
            && self.last_render.elapsed() < TILE_REDRAW_INTERVAL
        {
            ctx.request_repaint_after(TILE_REDRAW_INTERVAL);
            need = false;
        }
        if need {
            self.request_render(w, h, aspect, presentation);
            self.last_state = state;
            self.last_gen = gen;
            self.last_render = Instant::now();
        }
        if state == self.last_state && self.renderer.is_idle() {
            self.renderer
                .refill_requests(&self.cache, gen != self.last_gen, ctx);
        }
        self.last_gen = gen;
        if !idle {
            ctx.request_repaint_after(ACTIVE_REPAINT_INTERVAL);
        }
        // The frame worker admits visible tiles before becoming idle. Speculative
        // work waits for it, and pauses during interaction/closer zoom/other layers.
        if idle
            && self.renderer.is_idle()
            && self.provider == Provider::Satellite
            && self.cam.alt >= INITIAL_CAMERA.alt
            && self.tile_warmup.pump(&self.cache)
        {
            ctx.request_repaint_after(OVERVIEW_WARMUP_POLL_INTERVAL);
        }

        // ── drawing
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::BLACK);
        if let Some(tex) = &self.texture {
            painter.image(
                tex.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        // Keep projected borders and labels aligned with the displayed raster
        // while the coordinator works on the latest camera position.
        let (display_cam, display_basis) =
            self.renderer.displayed_view().unwrap_or((self.cam, basis));
        self.draw_overlays(&painter, &display_basis, rect, display_cam.alt);

        // Google Earth style navigation controls (rotate / move / zoom)
        let dt_nav = ctx.input(|i| i.stable_dt).min(MAX_FRAME_DELTA_SECS) as f64;
        if self.nav_controls(ui, rect, dt_nav) {
            self.touch();
            ctx.request_repaint();
        }

        // scale bar
        if display_cam.alt < MAX_SCALE_BAR_ALTITUDE_KM {
            let mpp = display_cam.alt * METERS_PER_KM * 2.0 * VERTICAL_FOV_HALF_TAN
                / rect.height() as f64;
            let target = mpp * TARGET_SCALE_BAR_WIDTH_POINTS;
            let pow = 10f64.powf(target.log10().floor());
            let nice = [5.0, 2.0, 1.0]
                .iter()
                .map(|m| m * pow)
                .find(|v| *v <= target)
                .unwrap_or(pow);
            let len = (nice / mpp) as f32;
            let y = rect.bottom() - 28.0;
            let x0 = rect.left() + 18.0;
            painter.line_segment(
                [Pos2::new(x0, y), Pos2::new(x0 + len, y)],
                Stroke::new(3.0_f32, Color32::BLACK),
            );
            painter.line_segment(
                [Pos2::new(x0, y), Pos2::new(x0 + len, y)],
                Stroke::new(1.5_f32, Color32::WHITE),
            );
            let label = if nice >= METERS_PER_KM {
                format!("{} km", nice / METERS_PER_KM)
            } else {
                format!("{} m", nice)
            };
            painter.text(
                Pos2::new(x0, y - 6.0),
                Align2::LEFT_BOTTOM,
                label,
                FontId::proportional(12.0 * self.settings.map_scale),
                Color32::WHITE,
            );
        }
        let mut attribution = self.provider.attribution(lg).to_string();
        if self.provider == Provider::Wayback {
            if let Some(r) = self.wayback_list.get(self.wayback_idx) {
                attribution = lg.format(
                    "wayback-snapshot-attribution",
                    &[
                        ("attribution", attribution.into()),
                        ("date", r.date.as_str().into()),
                    ],
                );
            }
        }
        painter.text(
            rect.right_bottom() - Vec2::new(8.0, 6.0),
            Align2::RIGHT_BOTTOM,
            attribution,
            FontId::proportional(11.0 * self.settings.map_scale),
            Color32::from_white_alpha(200),
        );

        // context menu (right-click)
        let pick = self.menu_pick;
        let mut act: Option<u8> = None;
        resp.context_menu(|ui| match pick {
            Some((la, lo)) => {
                ui.label(format!("{la:.5}, {lo:.5}"));
                ui.separator();
                if ui.button(lg.text("globe-placemark-action")).clicked() {
                    act = Some(0);
                    ui.close_menu();
                }
                if ui.button(lg.text("globe-fly-to-action")).clicked() {
                    act = Some(1);
                    ui.close_menu();
                }
                if ui.button(lg.text("globe-ruler-point-action")).clicked() {
                    act = Some(2);
                    ui.close_menu();
                }
                if ui
                    .button(lg.text("globe-copy-coordinates-action"))
                    .clicked()
                {
                    act = Some(3);
                    ui.close_menu();
                }
            }
            None => {
                ui.label(lg.text("globe-context-menu-hint"));
            }
        });
        if let (Some(a), Some((la, lo))) = (act, pick) {
            match a {
                0 => self.placemarks.push(Placemark {
                    name: lg.format(
                        "placemarks-generated-name",
                        &[("number", (self.placemarks.len() + 1).to_string().into())],
                    ),
                    desc: String::new(),
                    lat: la,
                    lon: lo,
                }),
                1 => self.fly_to(
                    la,
                    lo,
                    (self.cam.alt * POINT_FLY_ALTITUDE_RATIO).max(MIN_POINT_FLY_ALTITUDE_KM),
                ),
                2 => {
                    self.ruler = true;
                    self.ruler_pts.push((la, lo));
                }
                _ => ctx.copy_text(format!("{la:.6}, {lo:.6}")),
            }
        }

        // auto-rotate (keeps repainting)
        if self.auto_rotate && self.fly.is_none() {
            let dt = ctx.input(|i| i.unstable_dt).min(MAX_FRAME_DELTA_SECS) as f64;
            self.cam.lon = wrap_lon(self.cam.lon + AUTO_ROTATE_DEG_PER_SECOND * dt);
            self.touch();
            ctx.request_repaint();
        }
    }
}
