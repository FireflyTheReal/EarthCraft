//! Compact vector icons do not depend on platform font glyph coverage.
use super::panel_title;
use earthcraft_core::language::Lang;
use earthcraft_data::settings::sidebar::{SidebarLayout, SidebarPanel};
use eframe::egui;

const BUTTON_SIZE_POINTS: f32 = 28.0;
const ICON_SIZE_POINTS: f32 = 18.0;
const ICON_STROKE_POINTS: f32 = 1.5;

pub(super) fn show(ui: &mut egui::Ui, layout: &mut SidebarLayout, lang: Lang) {
    for panel in SidebarPanel::ALL {
        if button(ui, panel, layout.panel == panel, lang).clicked() {
            layout.panel = panel;
        }
    }
}

fn button_id(panel: SidebarPanel) -> egui::Id {
    egui::Id::new("sidebar-rail-button").with(panel as u8)
}

fn button(ui: &mut egui::Ui, panel: SidebarPanel, selected: bool, lang: Lang) -> egui::Response {
    let (_, rect) = ui.allocate_space(egui::Vec2::splat(BUTTON_SIZE_POINTS));
    let response = ui.interact(rect, button_id(panel), egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            ui.is_enabled(),
            selected,
            panel_title(panel, lang),
        )
    });
    let visuals = ui.style().interact_selectable(&response, selected);
    ui.painter().rect(
        rect,
        visuals.corner_radius,
        visuals.bg_fill,
        visuals.bg_stroke,
        egui::StrokeKind::Inside,
    );
    let stroke = egui::Stroke::new(ICON_STROKE_POINTS, visuals.fg_stroke.color);
    let icon = egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(ICON_SIZE_POINTS));
    let at = |x, y| icon.min + icon.size() * egui::vec2(x, y);
    match panel {
        SidebarPanel::Explore => {
            ui.painter()
                .circle_stroke(icon.center(), ICON_SIZE_POINTS * 0.45, stroke);
            ui.painter().add(egui::Shape::closed_line(
                vec![at(0.65, 0.25), at(0.55, 0.55), at(0.25, 0.7), at(0.4, 0.4)],
                stroke,
            ));
        }
        SidebarPanel::Layers => {
            for offset in [0.0, 0.22, 0.44] {
                ui.painter().add(egui::Shape::line(
                    vec![
                        at(0.05, 0.28 + offset),
                        at(0.5, 0.5 + offset),
                        at(0.95, 0.28 + offset),
                    ],
                    stroke,
                ));
            }
            ui.painter().add(egui::Shape::line(
                vec![at(0.05, 0.28), at(0.5, 0.06), at(0.95, 0.28)],
                stroke,
            ));
        }
        SidebarPanel::Places => {
            ui.painter().add(egui::Shape::closed_line(
                vec![
                    at(0.05, 0.3),
                    at(0.05, 0.15),
                    at(0.4, 0.15),
                    at(0.52, 0.3),
                    at(0.95, 0.3),
                    at(0.95, 0.85),
                    at(0.05, 0.85),
                ],
                stroke,
            ));
        }
    }
    response.on_hover_text(panel_title(panel, lang))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rail_buttons_accept_pointer_clicks_in_both_languages() {
        for lang in Lang::ALL {
            let ctx = egui::Context::default();
            let mut layout = SidebarLayout::default();
            let frame = |layout: &mut SidebarLayout, events| {
                crate::app::test_ui::run(
                    &ctx,
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(800.0, 600.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        ui.horizontal(|ui| show(ui, layout, lang));
                    },
                );
            };
            frame(&mut layout, vec![]);
            let rects =
                SidebarPanel::ALL.map(|panel| ctx.read_response(button_id(panel)).unwrap().rect);
            assert!(
                rects
                    .windows(2)
                    .all(|pair| pair[0].center().y == pair[1].center().y
                        && pair[0].right() < pair[1].left()),
                "selector must be horizontal"
            );
            for panel in [
                SidebarPanel::Layers,
                SidebarPanel::Places,
                SidebarPanel::Explore,
                SidebarPanel::Explore,
            ] {
                let rect = ctx
                    .read_response(button_id(panel))
                    .expect("rail button")
                    .rect;
                let pos = rect.center();
                frame(&mut layout, vec![egui::Event::PointerMoved(pos)]);
                for pressed in [true, false] {
                    frame(
                        &mut layout,
                        vec![egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        }],
                    );
                }
                assert_eq!(layout.panel, panel);
            }
        }
    }
}
