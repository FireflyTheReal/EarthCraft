//! Pointer ownership stays in the desktop adapter; projection stays in render.
use super::App;
use earthcraft_core::camera::Basis;
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
        if ctx.input(|i| i.pointer.primary_pressed()) {
            self.ruler_drag = pressed_handle(response, &basis, rect, &self.ruler_pts);
            if self.ruler_drag.is_some() {
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

fn pressed_handle(
    response: &egui::Response,
    basis: &Basis,
    rect: Rect,
    points: &[(f64, f64)],
) -> Option<usize> {
    // Use egui's input ownership so controls layered above the map keep clicks.
    if !(response.is_pointer_button_down_on()
        || response.clicked_by(egui::PointerButton::Primary)
        || response.drag_started_by(egui::PointerButton::Primary)
        || response.drag_stopped_by(egui::PointerButton::Primary))
    {
        return None;
    }
    // Several pointer events can arrive in one frame. The latest position may
    // already be far from the handle, so hit-test the primary press event itself.
    let press = response.ctx.input(|input| {
        input.events.iter().find_map(|event| match event {
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                ..
            } => Some(*pos),
            _ => None,
        })
    })?;
    if !rect.contains(press) {
        return None;
    }
    earthcraft_render::overlays::ruler::point_at(
        basis,
        rect,
        points,
        press,
        POINT_HIT_RADIUS_POINTS,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    use earthcraft_core::camera::Cam;
    fn handle_on_press(start: egui::Pos2, end: egui::Pos2) -> Option<usize> {
        let ctx = egui::Context::default();
        let rect = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 600.0));
        let basis = Cam {
            lat: 0.0,
            lon: 0.0,
            heading: 0.0,
            alt: 14000.0,
        }
        .basis(1.0);
        let mut selected = None;
        for events in [
            vec![egui::Event::PointerMoved(start)],
            vec![
                egui::Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerMoved(end),
            ],
        ] {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(rect),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
                        selected = pressed_handle(&response, &basis, rect, &[(0.0, 0.0)]);
                    });
                },
            );
        }
        selected
    }
    #[test]
    fn fast_drag_captures_the_point_at_press_not_at_frame_end() {
        assert_eq!(
            handle_on_press(egui::pos2(300.0, 300.0), egui::pos2(350.0, 320.0)),
            Some(0)
        );
    }
    #[test]
    fn dragging_onto_a_point_from_empty_map_does_not_capture_it() {
        assert_eq!(
            handle_on_press(egui::pos2(350.0, 320.0), egui::pos2(300.0, 300.0)),
            None
        );
    }
}
