use super::*;
use earthcraft_core::language::Lang;

#[test]
fn menu_catalog_has_translated_labels_populated_sections_and_unique_shortcuts() {
    use commands::Section;
    for lang in Lang::ALL {
        for section in Section::ALL {
            assert!(!section.label(lang).is_empty());
            assert!(Action::ALL.iter().any(|action| action.section() == section));
        }
        for action in Action::ALL {
            assert!(!action.label(lang).is_empty());
        }
    }
    let shortcuts: Vec<_> = Action::ALL
        .into_iter()
        .filter_map(|action| action.shortcut())
        .collect();
    for (index, shortcut) in shortcuts.iter().enumerate() {
        assert!(!shortcuts[index + 1..].contains(shortcut));
    }
}

#[test]
fn menu_enablement_and_checkmarks_follow_app_state() {
    let empty = MenuState::default();
    assert!(!empty.enabled(Action::Screenshot));
    assert!(!empty.enabled(Action::Undo));
    assert!(!empty.enabled(Action::Redo));
    assert!(empty.enabled(Action::Open));
    assert!(empty.enabled(Action::Settings));
    let state = MenuState {
        image: true,
        undo: true,
        redo: true,
        sidebar: true,
        ruler: true,
        grid: true,
        navigation: true,
        ..MenuState::default()
    };
    assert!(state.enabled(Action::Screenshot));
    assert!(state.enabled(Action::Undo));
    assert!(state.enabled(Action::Redo));
    for action in [
        Action::Sidebar,
        Action::Ruler,
        Action::Grid,
        Action::NavigationControls,
    ] {
        assert_eq!(state.checked(action), Some(true));
        assert_eq!(empty.checked(action), Some(false));
    }
    assert_eq!(state.checked(Action::Placemark), None);
}

#[test]
fn in_window_menu_routes_a_real_pointer_selection_through_the_shared_catalog() {
    let _: fn(&mut crate::app::App, &mut egui::Ui) = crate::app::App::in_window_menu;
    for lang in Lang::ALL {
        let ctx = egui::Context::default();
        let mut selected = None;
        let frame = |selected: &mut Option<Action>, events| {
            crate::app::test_ui::run(
                &ctx,
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1100.0, 600.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        ui.horizontal(|ui| {
                            *selected = commands::draw_menu(ui, lang, MenuState::default());
                        });
                    });
                },
            )
        };
        let find_text = |output: &egui::FullOutput, label: &str| {
            output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == label => {
                        Some(egui::Rect::from_min_size(text.pos, text.galley.size()).center())
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("Missing {label}"))
        };
        let output = frame(&mut selected, vec![]);
        let menu = find_text(&output, commands::Section::File.label(lang));
        let click = |selected: &mut Option<Action>, pos| {
            frame(selected, vec![egui::Event::PointerMoved(pos)]);
            frame(
                selected,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            frame(
                selected,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            )
        };
        click(&mut selected, menu);
        let output = frame(&mut selected, vec![]);
        let open = find_text(&output, Action::Open.label(lang));
        click(&mut selected, open);
        assert_eq!(selected, Some(Action::Open));
    }
}
