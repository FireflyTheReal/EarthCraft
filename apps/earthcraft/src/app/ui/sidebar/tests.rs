use super::*;
#[test]
fn sidebar_width_ignores_section_contents_and_keeps_user_width() {
    let ctx = egui::Context::default();
    let frame = |wide_content| {
        let mut width = 0.0;
        let _ = crate::app::test_ui::run(
            &ctx,
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 600.0),
                )),
                ..Default::default()
            },
            |ui| {
                let panel = expanded_sidebar_panel();
                width = panel
                    .show(ui, |ui| {
                        sidebar_scroll_area().show(ui, |ui| {
                            if wide_content {
                                ui.allocate_space(egui::vec2(700.0, 200.0));
                            } else {
                                ui.label("Layers");
                            }
                        });
                    })
                    .response
                    .rect
                    .width();
            },
        );
        width
    };
    assert_eq!(frame(false), DEFAULT_SIDEBAR_WIDTH_POINTS);
    assert_eq!(frame(true), DEFAULT_SIDEBAR_WIDTH_POINTS);
    assert_eq!(frame(false), DEFAULT_SIDEBAR_WIDTH_POINTS);
    // Restore the same outer rectangle stored after a user drags the panel edge.
    let mut resized =
        egui::containers::panel::PanelState::load(&ctx, egui::Id::new("side")).unwrap();
    resized.outer_rect.max.x = resized.outer_rect.min.x + 340.0;
    ctx.data_mut(|data| data.insert_persisted(egui::Id::new("side"), resized));
    assert_eq!(frame(false), 340.0);
    assert_eq!(frame(true), 340.0);
    assert_eq!(frame(false), 340.0);
}
