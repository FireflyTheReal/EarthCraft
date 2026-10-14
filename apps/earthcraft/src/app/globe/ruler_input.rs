//! Pointer ownership stays in the desktop adapter; projection stays in render.
use super::App;
use eframe::egui::{self, Key, Rect};
const POINT_HIT_RADIUS_POINTS: f32 = 10.0;

impl App {
    pub(super) fn edit_ruler(
        &mut self,
        ui: &egui::Ui,
        ctx: &egui::Context,
        response: &egui::Response,
        rect: Rect,
        aspect: f64,
    ) -> bool {
        if !self.ruler
            || self.sidebar_animating
            || self.explorer.active != earthcraft_core::body::Body::Earth
        {
            self.ruler_drag = None;
            return false;
        }
        let basis = self.renderer.picking_basis(self.cam.basis(aspect));
        let pointer = ctx.input(|i| i.pointer.interact_pos());
        let hovered = pointer
            .filter(|p| response.hovered() && rect.contains(*p))
            .and_then(|p| {
                earthcraft_render::overlays::ruler::point_at(
                    &basis,
                    rect,
                    &self.ruler_pts,
                    p,
                    POINT_HIT_RADIUS_POINTS,
                )
            });
        if hovered.is_some() || self.ruler_drag.is_some() {
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grab);
        }
        if let Some(index) = hovered.filter(|_| !ctx.wants_keyboard_input()) {
            if ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::NONE, Key::Delete)
                    || i.consume_key(egui::Modifiers::NONE, Key::Backspace)
            }) {
                self.ruler_pts.remove(index);
                self.ruler_drag = None;
                return true;
            }
        }
        if response.hovered() && ctx.input(|i| i.pointer.primary_pressed()) {
            self.ruler_drag = hovered;
            if hovered.is_some() {
                self.fly = None;
            }
        }
        let captured = self.ruler_drag.is_some();
        if let Some(index) = self.ruler_drag {
            if let Some(point) = pointer
                .filter(|p| rect.contains(*p))
                .and_then(|p| basis.pick(rect, p))
            {
                if let Some(target) = self.ruler_pts.get_mut(index) {
                    *target = point;
                }
            }
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grabbing);
            ctx.request_repaint();
        }
        if !ctx.input(|i| i.pointer.primary_down()) {
            self.ruler_drag = None;
        }
        captured
    }
}
