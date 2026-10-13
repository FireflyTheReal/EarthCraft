use super::App;
use super::{INITIAL_CAMERA, MAX_FRAME_DELTA_SECS, PAN_SPEED_POINTS_PER_SECOND};
use earthcraft_core::{
    math::{now_unix, wrap_lon},
    placemark::Placemark,
};
use earthcraft_data::tiles::Provider;
use eframe::egui::{
    self, Align2, Color32, FontId, Key, PointerButton, Pos2, Rect, Sense, Stroke, Vec2,
};
use std::time::{Duration, Instant};
const DRAG_HEADING_DEG_PER_POINT: f64 = 0.3;
const SCROLL_ZOOM_RATE_PER_POINT: f64 = 0.0025;
const POINT_FLY_ALTITUDE_RATIO: f64 = 0.35;
const MIN_POINT_FLY_ALTITUDE_KM: f64 = 0.3;
const AUTO_ROTATE_DEG_PER_SECOND: f64 = 6.0;
// Lower resolution while interacting; recover full resolution after the idle delay.
const IDLE_RENDER_DELAY: Duration = Duration::from_millis(250);
const TILE_REDRAW_INTERVAL: Duration = Duration::from_millis(60);
const OVERVIEW_WARMUP_POLL_INTERVAL: Duration = Duration::from_millis(100);

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
            let d = ctx.input(|i| i.pointer.delta());
            self.fly = None;
            if self.explorer.active == earthcraft_core::body::Body::Earth {
                self.pan(d.x as f64, d.y as f64, rect.height());
            } else {
                self.cam.orbit_body(d.x as f64, d.y as f64, rect.height());
            }
            interacting = true;
        } else if resp.dragged_by(PointerButton::Secondary)
            || (resp.dragged_by(PointerButton::Primary) && shift)
        {
            let drag = ctx.input(|i| i.pointer.delta());
            self.cam.heading =
                wrap_lon(self.cam.heading - drag.x as f64 * DRAG_HEADING_DEG_PER_POINT);
            interacting = true;
        }
        if resp.hovered() {
            let sc = ctx.input(|i| i.smooth_scroll_delta.y) as f64;
            let zd = ctx.input(|i| i.zoom_delta()) as f64;
            let cur = resp.hover_pos();
            if sc != 0.0 {
                self.fly = None;
                let factor = (-sc * SCROLL_ZOOM_RATE_PER_POINT).exp();
                if self.explorer.active == earthcraft_core::body::Body::Earth {
                    self.zoom_at(factor, cur, rect, aspect);
                } else {
                    self.zoom(factor);
                }
                interacting = true;
            }
            if (zd - 1.0).abs() > 1e-4 {
                self.fly = None;
                if self.explorer.active == earthcraft_core::body::Body::Earth {
                    self.zoom_at(1.0 / zd, cur, rect, aspect);
                } else {
                    self.zoom(1.0 / zd);
                }
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
                if self.explorer.active == earthcraft_core::body::Body::Earth {
                    self.pan(dx, dy, rect.height());
                } else {
                    self.cam.orbit_body(dx, dy, rect.height());
                }
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
        let (w, h) = earthcraft_render::view::raster_size(rect.size(), ppp, idle);
        let keys = earthcraft_render::view::RasterView {
            camera: self.cam,
            width: w,
            height: h,
            aspect,
            provider: self.effective_provider(),
            release_index: self.wayback_idx,
            grid: self.show_grid,
            atmosphere: self.show_atmo,
            sunlight: self.show_sun,
        }
        .keys(now_unix());
        let state = keys.state;
        let gen = self.cache.generation();
        let presentation = keys.presentation;
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
            self.request_render(w, h, aspect, presentation, rect);
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
            ctx.request_repaint();
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
        if self.explorer.active != earthcraft_core::body::Body::Earth {
            painter.rect_filled(rect, 0.0, Color32::BLACK);
            if let Some(sky) = self
                .explorer
                .sky
                .as_ref()
                .filter(|_| self.explorer.active == earthcraft_core::body::Body::Custom)
            {
                painter.image(
                    sky.id(),
                    rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            if let Some(body) = self.explorer.active_texture() {
                draw_planet(
                    &painter,
                    rect,
                    body,
                    self.cam.lat,
                    self.cam.lon,
                    self.cam.alt,
                );
            }
        }
        // Keep projected borders and labels aligned with the displayed raster
        // while the coordinator works on the latest camera position.
        let (display_cam, display_basis) =
            self.renderer.displayed_view().unwrap_or((self.cam, basis));
        if self.explorer.active == earthcraft_core::body::Body::Earth {
            self.live_maps.draw(
                &painter,
                &display_basis,
                rect,
                self.show_clouds,
                self.show_weather,
            );
            self.draw_overlays(&painter, &display_basis, rect, display_cam.alt);
            self.draw_live_layers(&painter, &display_basis, rect, display_cam.alt);
        }

        // Google Earth style navigation controls (rotate / move / zoom)
        let dt_nav = ctx.input(|i| i.stable_dt).min(MAX_FRAME_DELTA_SECS) as f64;
        if self.nav_controls(ui, rect, dt_nav) {
            self.touch();
            ctx.request_repaint();
        }

        // scale bar
        if let Some(scale_bar) =
            earthcraft_render::scale_bar::scale_bar(display_cam.alt, rect.height())
        {
            let len = scale_bar.width_points;
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
            let label = scale_bar.label();
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

        if self.explorer.active == earthcraft_core::body::Body::Earth
            && (self.show_clouds || self.show_weather)
        {
            let source = self
                .live_maps
                .status(self.show_clouds, self.show_weather, lg);
            let credit = if self.show_clouds && self.show_weather {
                format!(
                    "{}\n{}",
                    lg.text("live-cloud-credit"),
                    lg.text("live-radar-credit")
                )
            } else if self.show_clouds {
                lg.text("live-cloud-credit").into()
            } else {
                lg.text("live-radar-credit").into()
            };
            painter.text(
                rect.left_bottom() + Vec2::new(12.0, -58.0),
                Align2::LEFT_BOTTOM,
                format!("{source}\n{credit}"),
                FontId::proportional(11.0),
                Color32::WHITE,
            );
        }

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

fn draw_planet(
    painter: &egui::Painter,
    rect: Rect,
    tex: &egui::TextureHandle,
    lat: f64,
    lon: f64,
    altitude_km: f64,
) {
    let geometry = earthcraft_render::planet::planet_mesh(rect, lat, lon, altitude_km);
    let mut mesh = egui::Mesh::with_texture(tex.id());
    mesh.vertices = geometry
        .vertices
        .into_iter()
        .map(|vertex| egui::epaint::Vertex {
            pos: vertex.pos,
            uv: vertex.uv,
            color: Color32::WHITE,
        })
        .collect();
    mesh.indices = geometry.indices;
    painter.add(egui::Shape::mesh(mesh));
}
