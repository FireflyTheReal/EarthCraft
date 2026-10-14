//! Consistent full-width selection rows in dropdown menus.
use eframe::egui;

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
    fn dropdown_rows_fill_the_width_and_accept_clicks_past_the_text() {
        let ctx = egui::Context::default();
        let mut selected = 1;
        let frame = |selected: &mut i32, events| {
            let mut rects = Vec::new();
            let _ = ctx.run(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.set_width(250.0);
                        rects.push(full_width_value(ui, selected, 0, "Short").rect);
                        rects.push(full_width_value(ui, selected, 1, "A longer option").rect);
                    });
                },
            );
            assert_eq!(rects[0].width(), rects[1].width());
            assert!(rects[0].width() >= 250.0);
            rects[0]
        };
        let rect = frame(&mut selected, vec![]);
        let pos = egui::pos2(rect.right() - 5.0, rect.center().y);
        let _ = frame(&mut selected, vec![egui::Event::PointerMoved(pos)]);
        let button = |pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: egui::Modifiers::NONE,
        };
        let _ = frame(&mut selected, vec![button(true)]);
        let _ = frame(&mut selected, vec![button(false)]);
        assert_eq!(selected, 0);
    }
}
