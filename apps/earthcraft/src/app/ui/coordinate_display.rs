//! Coordinate preferences and copyable readouts; conversion stays in core.
use earthcraft_core::{
    coordinates::{CoordinateDisplay, CoordinateFormat},
    language::Lang,
};
use eframe::egui;

pub(super) fn controls(ui: &mut egui::Ui, display: &mut CoordinateDisplay, lang: Lang) {
    ui.horizontal(|ui| {
        ui.label(lang.text("coordinates-format-label"));
        egui::ComboBox::from_id_salt("coordinate_format")
            .selected_text(lang.text(display.format.name_id()))
            .show_ui(ui, |ui| {
                super::selection::size_menu(
                    ui,
                    CoordinateFormat::ALL
                        .into_iter()
                        .map(|format| lang.text(format.name_id())),
                );
                for format in CoordinateFormat::ALL {
                    let mut selected = display.format;
                    if super::selection::full_width_value(
                        ui,
                        &mut selected,
                        format,
                        lang.text(format.name_id()),
                    )
                    .changed()
                    {
                        display.select_format(selected);
                    }
                }
            });
    });
    ui.horizontal(|ui| {
        ui.label(lang.text(display.format.precision_id()));
        ui.add(
            egui::DragValue::new(&mut display.precision).range(display.format.precision_range()),
        );
    });
    ui.weak(lang.text("coordinates-input-help"));
}

pub(super) fn copyable_label(
    ui: &mut egui::Ui,
    text: String,
    coordinates: &str,
    lang: Lang,
) -> egui::Response {
    let response = ui
        .add(egui::Label::new(egui::RichText::new(text).monospace()).sense(egui::Sense::click()))
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(lang.format(
            "coordinates-copy-tooltip",
            &[("coordinates", coordinates.into())],
        ));
    if response.clicked() {
        ui.copy_text(coordinates.to_owned());
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clicking_the_readout_copies_the_selected_format_without_its_localized_prefix() {
        for lang in Lang::ALL {
            let ctx = egui::Context::default();
            let coordinates = CoordinateDisplay::default()
                .format(37.7749, -122.4194)
                .unwrap();
            let mut rect = egui::Rect::NOTHING;
            let frame = |rect: &mut egui::Rect, events| {
                crate::app::test_ui::run(
                    &ctx,
                    egui::RawInput {
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        *rect = copyable_label(
                            ui,
                            lang.format(
                                "coordinates-camera-readout",
                                &[("coordinates", coordinates.as_str().into())],
                            ),
                            &coordinates,
                            lang,
                        )
                        .rect;
                    },
                )
            };
            frame(&mut rect, vec![]);
            let pos = rect.center();
            frame(&mut rect, vec![egui::Event::PointerMoved(pos)]);
            frame(
                &mut rect,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            let output = frame(
                &mut rect,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            assert!(output.platform_output.commands.iter().any(|command| matches!(command, egui::OutputCommand::CopyText(text) if text == &coordinates)));
        }
    }
}
