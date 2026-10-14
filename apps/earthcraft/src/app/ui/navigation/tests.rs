use super::*;

#[test]
fn held_navigation_retains_pointer_until_release() {
    for move_pointer in [false, true] {
        let ctx = egui::Context::default();
        let button_rect = Rect::from_center_size(Pos2::new(100.0, 100.0), Vec2::splat(15.0));
        let frame = |time, events| {
            let mut held = false;
            let _ = crate::app::test_ui::run(
                &ctx,
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(400.0))),
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        // Navigation buttons overlap the draggable globe and rotation ring.
                        let globe =
                            ui.interact(ui.max_rect(), ui.id().with("globe"), Sense::drag());
                        let ring = ui.interact(
                            button_rect.expand(30.0),
                            ui.id().with("ring"),
                            Sense::drag(),
                        );
                        let control = hold_control(ui, button_rect, ui.id().with("control"));
                        held = control.is_pointer_button_down_on();
                        assert!(!globe.dragged());
                        assert!(!ring.dragged());
                    });
                },
            );
            held
        };
        let pointer_button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let start = button_rect.center();
        assert!(!frame(0.0, vec![egui::Event::PointerMoved(start)]));
        assert!(frame(0.1, vec![pointer_button(start, true)]));
        let mut pos = start;
        for tick in 1..=100 {
            let events = if move_pointer {
                // Also retain ownership when pointer motion exceeds the click threshold.
                pos = start + Vec2::new(20.0, 0.0);
                vec![egui::Event::PointerMoved(pos)]
            } else {
                vec![]
            };
            assert!(frame(0.1 + tick as f64 * 0.1, events), "hold tick {tick}");
        }
        assert!(!frame(10.2, vec![pointer_button(pos, false)]));
        assert!(!frame(10.3, vec![]));
    }
}
