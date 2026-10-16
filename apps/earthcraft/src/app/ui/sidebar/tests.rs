use super::*;
use earthcraft_data::settings::sidebar::DEFAULT_SIDEBAR_WIDTH_POINTS;
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
                let panel = expanded_sidebar_panel(DEFAULT_SIDEBAR_WIDTH_POINTS);
                width = panel
                    .show(ui, |ui| {
                        sidebar_scroll_area("test").show(ui, |ui| {
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

#[test]
fn shortcuts_reveal_existing_tools_without_unpinning_places() {
    let mut layout = SidebarLayout {
        panel: SidebarPanel::Layers,
        ..SidebarLayout::default()
    };
    route_shortcuts(&mut layout, true, false);
    assert_eq!(layout.panel, SidebarPanel::Places);
    layout.panel = SidebarPanel::Layers;
    layout.places_pinned = true;
    route_shortcuts(&mut layout, true, false);
    assert_eq!(layout.panel, SidebarPanel::Layers);
    route_shortcuts(&mut layout, false, true);
    assert_eq!(layout.panel, SidebarPanel::Explore);
    assert!(layout.places_pinned);
}

#[test]
fn shell_keeps_pinned_places_visible_without_drawing_it_twice() {
    for lang in Lang::ALL {
        for height in [360.0, 800.0] {
            let ctx = egui::Context::default();
            ctx.global_style_mut(|style| style.animation_time = 0.0);
            let mut layout = SidebarLayout {
                places_pinned: true,
                ..SidebarLayout::default()
            };
            let mut open = true;
            for panel in SidebarPanel::ALL {
                layout.panel = panel;
                for _ in 0..3 {
                    let mut panels = Vec::new();
                    crate::app::test_ui::run(
                        &ctx,
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(900.0, height),
                            )),
                            ..Default::default()
                        },
                        |ui| {
                            show_shell(ui, &mut layout, &mut open, lang, |ui, panel| {
                                panels.push(panel);
                                let viewport = ui.available_rect_before_wrap();
                                assert!(viewport.is_finite(), "{viewport:?}");
                                assert!(
                                    ui.clip_rect().right() < 400.0,
                                    "sidebar must leave space for the globe"
                                );
                                ui.label(panel_title(panel, lang));
                            });
                        },
                    );
                    assert_eq!(
                        panels
                            .iter()
                            .filter(|&&panel| panel == SidebarPanel::Places)
                            .count(),
                        1
                    );
                    assert!(panels.contains(&panel));
                    assert!(open);
                    assert!(layout.places_pinned);
                    assert_eq!(layout.width_points, DEFAULT_SIDEBAR_WIDTH_POINTS);
                }
            }
        }
    }
}

#[test]
fn collapsing_sidebar_leaves_no_rail_or_panel_width() {
    for lang in Lang::ALL {
        let ctx = egui::Context::default();
        ctx.global_style_mut(|style| style.animation_time = 0.0);
        let mut layout = SidebarLayout::default();
        for open in [true, false, true, false] {
            let mut open = open;
            for _ in 0..3 {
                crate::app::test_ui::run(&ctx, egui::RawInput::default(), |ui| {
                    let before = ui.available_rect_before_wrap().left();
                    let mut drawn = false;
                    show_shell(ui, &mut layout, &mut open, lang, |_, _| drawn = true);
                    let width = ui.available_rect_before_wrap().left() - before;
                    assert_eq!(drawn, open);
                    if open {
                        assert_eq!(width, DEFAULT_SIDEBAR_WIDTH_POINTS);
                    } else {
                        assert_eq!(width, 0.0, "collapse must hide navigation too");
                    }
                });
            }
        }
    }
}
