//! Consistent full-width selection rows in dropdown menus.
use eframe::egui;

/// Resolve popup width before drawing any row; later long labels must not grow
/// the popup after earlier highlights have already been sized.
pub(super) fn size_menu<'a>(ui: &mut egui::Ui, labels: impl IntoIterator<Item = &'a str>) {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let padding = ui.spacing().button_padding.x * 2.0;
    let width = labels
        .into_iter()
        .map(|label| {
            ui.painter()
                .layout_no_wrap(label.into(), font.clone(), ui.visuals().text_color())
                .size()
                .x
                + padding
        })
        .fold(ui.available_width(), f32::max);
    ui.set_min_width(width);
}

pub(super) fn full_width_value<T: PartialEq>(
    ui: &mut egui::Ui,
    current: &mut T,
    value: T,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), ui.spacing().interact_size.y),
        egui::Layout::top_down_justified(egui::Align::Min),
        |ui| ui.selectable_value(current, value, text),
    )
    .inner
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn auto_sized_popup_rows_fill_width_and_accept_clicks_past_text() {
        let ctx = egui::Context::default();
        let labels = ["English", "Português (Brasil)"];
        let mut selected = 0;
        let frame = |selected: &mut usize, events| {
            let mut button = egui::Rect::NOTHING;
            let mut rows = Vec::new();
            let _ = crate::app::test_ui::run(
                &ctx,
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(600.0, 400.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        let response = egui::ComboBox::from_id_salt("language")
                            .width(100.0)
                            .selected_text(labels[*selected])
                            .show_ui(ui, |ui| {
                                size_menu(ui, labels);
                                for (value, text) in labels.into_iter().enumerate() {
                                    rows.push(full_width_value(ui, selected, value, text).rect);
                                }
                            });
                        button = response.response.rect;
                    });
                },
            );
            (button, rows)
        };
        let (button, _) = frame(&mut selected, vec![]);
        let pointer_button = |pos, pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: egui::Modifiers::NONE,
        };
        let pos = button.center();
        let _ = frame(&mut selected, vec![egui::Event::PointerMoved(pos)]);
        let _ = frame(&mut selected, vec![pointer_button(pos, true)]);
        let _ = frame(&mut selected, vec![pointer_button(pos, false)]);
        // Let the popup register its row hit regions after opening.
        let (_, rows) = frame(&mut selected, vec![]);
        assert_eq!(rows.len(), 2);
        assert!((rows[0].width() - rows[1].width()).abs() < 0.1, "{rows:?}");
        assert!(
            rows[0].width() > 100.0,
            "long option expands the whole popup"
        );
        // Select the other option first so clicking the first row must change it.
        selected = 1;
        let pos = egui::pos2(rows[0].right() - 3.0, rows[0].center().y);
        let _ = frame(&mut selected, vec![egui::Event::PointerMoved(pos)]);
        let _ = frame(&mut selected, vec![pointer_button(pos, true)]);
        let _ = frame(&mut selected, vec![pointer_button(pos, false)]);
        assert_eq!(selected, 0);
    }
}
