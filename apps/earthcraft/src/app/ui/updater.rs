//! A compact, centered status-bar control. Service logic stays in data.
use earthcraft_core::language::Lang;
use earthcraft_data::updater::{RELEASES_URL, UpdateState};
use eframe::egui::{self, Align, Atom, Button, Color32, Layout, RichText, Stroke, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    None,
    InstallOrRetry,
    Restart,
    OpenReleases,
}

pub(super) fn indicator(
    ui: &mut egui::Ui,
    state: &UpdateState,
    lang: Lang,
    restart_error: Option<&str>,
    saving: bool,
) -> Action {
    let (label, tooltip, action, busy) = match state {
        UpdateState::Idle | UpdateState::Checking | UpdateState::UpToDate => return Action::None,
        UpdateState::Available { version } => (
            lang.format(
                "update-available-label",
                &[("version", version.as_str().into())],
            ),
            lang.format(
                "update-available-tooltip",
                &[("version", version.as_str().into())],
            ),
            Action::InstallOrRetry,
            false,
        ),
        UpdateState::Downloading {
            downloaded, total, ..
        } => (
            lang.text("update-downloading-label").into(),
            match total.filter(|total| *total > 0) {
                Some(total) => lang.format(
                    "update-progress-tooltip",
                    &[(
                        "percent",
                        format!(
                            "{:.0}",
                            (*downloaded as f64 / total as f64 * 100.0).clamp(0.0, 100.0)
                        )
                        .into(),
                    )],
                ),
                None => lang.text("update-downloading-label").into(),
            },
            Action::None,
            true,
        ),
        UpdateState::Installing { .. } => (
            lang.text("update-installing-label").into(),
            lang.text("update-installing-tooltip").into(),
            Action::None,
            true,
        ),
        UpdateState::ReadyToRestart { version } => (
            if saving {
                lang.text("update-saving-label")
            } else {
                lang.text("update-restart-label")
            }
            .to_owned(),
            restart_error.map(str::to_owned).unwrap_or_else(|| {
                lang.format(
                    "update-restart-tooltip",
                    &[("version", version.as_str().into())],
                )
            }),
            Action::Restart,
            saving,
        ),
        UpdateState::Failed { message, .. } => (
            lang.text("update-retry-label").into(),
            message.clone(),
            Action::InstallOrRetry,
            false,
        ),
        UpdateState::Unsupported { version, reason } => (
            lang.format(
                "update-available-label",
                &[("version", version.as_str().into())],
            ),
            reason.clone(),
            Action::OpenReleases,
            false,
        ),
    };
    let available = ui.available_size();
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let icon_size = font.size;
    let text_width = ui
        .painter()
        .layout_no_wrap(label.clone(), font, Color32::WHITE)
        .size()
        .x;
    let width =
        text_width + icon_size + ui.spacing().icon_spacing + ui.spacing().button_padding.x * 2.0;
    // Hide in cramped windows rather than covering existing status/heading readouts.
    if available.x < width {
        return Action::None;
    }
    let mut chosen = Action::None;
    ui.allocate_ui_with_layout(
        Vec2::new(available.x, ui.spacing().interact_size.y),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_clip_rect(ui.max_rect().intersect(ui.clip_rect()));
            ui.add_space((available.x - width) * 0.5);
            let icon_id = ui.id().with("update-icon");
            let button = ui
                .add_enabled_ui(!busy, |ui| {
                    Button::new((
                        Atom::custom(icon_id, Vec2::splat(icon_size)),
                        RichText::new(&label).monospace().color(Color32::WHITE),
                    ))
                    .small()
                    .frame(true)
                    .min_size(Vec2::new(width, ui.spacing().interact_size.y))
                    .atom_ui(ui)
                })
                .inner;
            if let Some(rect) = button.rect(icon_id) {
                if busy {
                    egui::Spinner::new()
                        .color(Color32::WHITE)
                        .paint_at(ui, rect);
                } else {
                    paint_icon(
                        ui.painter(),
                        rect,
                        matches!(
                            state,
                            UpdateState::ReadyToRestart { .. } | UpdateState::Failed { .. }
                        ),
                    );
                }
            }
            let response = button
                .response
                .on_disabled_hover_text(&tooltip)
                .on_hover_text(&tooltip);
            if response.clicked() {
                chosen = action;
            }
            if matches!(
                state,
                UpdateState::Failed { .. } | UpdateState::Unsupported { .. }
            ) {
                response.context_menu(|ui| {
                    if ui.button(lang.text("update-releases-action")).clicked() {
                        ui.open_url(egui::OpenUrl::new_tab(RELEASES_URL));
                        ui.close();
                    }
                });
            }
        },
    );
    chosen
}

fn paint_icon(painter: &egui::Painter, rect: egui::Rect, restart: bool) {
    const ICON_GRID_UNITS: f32 = 12.0;
    const ICON_STROKE_POINTS: f32 = 1.25;
    let point = |x: f32, y: f32| rect.min + rect.size() * egui::vec2(x, y) / ICON_GRID_UNITS;
    let stroke = Stroke::new(ICON_STROKE_POINTS, Color32::WHITE);
    if restart {
        // A circular arrow, drawn as vectors rather than relying on font glyphs.
        painter.add(egui::Shape::line(
            [
                (10.0, 4.0),
                (9.0, 2.0),
                (6.0, 1.0),
                (3.0, 2.0),
                (1.0, 5.0),
                (2.0, 9.0),
                (5.0, 11.0),
                (9.0, 10.0),
                (11.0, 7.0),
            ]
            .into_iter()
            .map(|(x, y)| point(x, y))
            .collect(),
            stroke,
        ));
        painter.add(egui::Shape::line(
            vec![point(7.0, 4.0), point(10.0, 4.0), point(10.0, 1.0)],
            stroke,
        ));
    } else {
        painter.line_segment([point(6.0, 1.0), point(6.0, 8.0)], stroke);
        painter.add(egui::Shape::line(
            vec![point(3.0, 5.0), point(6.0, 8.0), point(9.0, 5.0)],
            stroke,
        ));
        painter.add(egui::Shape::line(
            vec![
                point(1.0, 9.0),
                point(1.0, 11.0),
                point(11.0, 11.0),
                point(11.0, 9.0),
            ],
            stroke,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input(events: Vec<egui::Event>) -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(600.0, 100.0),
            )),
            events,
            ..Default::default()
        }
    }
    #[test]
    fn clicks_trigger_only_available_retry_or_completed_install_actions() {
        for (state, expected) in [
            (
                UpdateState::Available {
                    version: "1.0.0".into(),
                },
                Action::InstallOrRetry,
            ),
            (
                UpdateState::Downloading {
                    version: "1.0.0".into(),
                    downloaded: 10,
                    total: None,
                },
                Action::None,
            ),
            (
                UpdateState::Installing {
                    version: "1.0.0".into(),
                },
                Action::None,
            ),
            (
                UpdateState::ReadyToRestart {
                    version: "1.0.0".into(),
                },
                Action::Restart,
            ),
            (
                UpdateState::Failed {
                    operation: earthcraft_data::updater::UpdateOperation::Install,
                    message: "Download failed".into(),
                },
                Action::InstallOrRetry,
            ),
        ] {
            let ctx = egui::Context::default();
            let output = crate::app::test_ui::run(&ctx, input(Vec::new()), |ui| {
                indicator(ui, &state, Lang::En, None, false);
            });
            let position = output
                .shapes
                .iter()
                .find_map(|shape| {
                    if let egui::epaint::Shape::Text(text) = &shape.shape {
                        Some(text.pos + text.galley.size() * 0.5)
                    } else {
                        None
                    }
                })
                .unwrap();
            let mut action = Action::None;
            for pressed in [true, false] {
                crate::app::test_ui::run(
                    &ctx,
                    input(vec![
                        egui::Event::PointerMoved(position),
                        egui::Event::PointerButton {
                            pos: position,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ]),
                    |ui| {
                        action = indicator(ui, &state, Lang::En, None, false);
                    },
                );
            }
            assert_eq!(action, expected);
        }
    }
    #[test]
    fn hidden_states_leave_the_status_bar_empty_in_both_languages() {
        for lang in Lang::ALL {
            for state in [
                UpdateState::Idle,
                UpdateState::Checking,
                UpdateState::UpToDate,
            ] {
                let ctx = egui::Context::default();
                crate::app::test_ui::run(&ctx, input(Vec::new()), |ui| {
                    let before = ui.cursor();
                    assert_eq!(indicator(ui, &state, lang, None, false), Action::None);
                    assert_eq!(ui.cursor(), before);
                });
            }
        }
    }
    #[test]
    fn available_notice_is_centered_and_restarts_are_visually_distinct() {
        for lang in Lang::ALL {
            for (state, label) in [
                (
                    UpdateState::Available {
                        version: "1.0.0".into(),
                    },
                    lang.format("update-available-label", &[("version", "1.0.0".into())]),
                ),
                (
                    UpdateState::ReadyToRestart {
                        version: "1.0.0".into(),
                    },
                    lang.text("update-restart-label").to_owned(),
                ),
            ] {
                let ctx = egui::Context::default();
                let mut bounds = egui::Rect::NOTHING;
                let output = crate::app::test_ui::run(&ctx, input(Vec::new()), |ui| {
                    bounds = ui.available_rect_before_wrap();
                    indicator(ui, &state, lang, None, false);
                });
                let text = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::epaint::Shape::Text(text) if text.galley.text() == label.as_str() => {
                            Some(text)
                        }
                        _ => None,
                    })
                    .unwrap();
                let button = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::epaint::Shape::Rect(rect) => Some(rect),
                        _ => None,
                    })
                    .unwrap();
                assert!((button.rect.center().x - bounds.center().x).abs() < 2.0);
                assert_eq!(text.galley.job.sections[0].format.color, Color32::WHITE);
                assert_eq!(
                    text.galley.job.sections[0].format.font_id,
                    egui::TextStyle::Monospace.resolve(&ctx.global_style())
                );
                assert!(
                    output
                        .shapes
                        .iter()
                        .any(|shape| matches!(shape.shape, egui::epaint::Shape::Path(_)))
                );
            }
        }
    }
}
