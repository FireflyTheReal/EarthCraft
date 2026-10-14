use eframe::egui;
const MAX_PRECISION: usize = 5;
const MIN_FONT_SCALE: f32 = 0.75;
const FONT_STEP_POINTS: f32 = 0.25;

fn coordinate_text(cursor: Option<(f64, f64)>, precision: usize, prefix: bool) -> String {
    let label = if prefix { "Cursor: " } else { "" };
    let lat_width = if precision == 0 { 3 } else { precision + 4 };
    let lon_width = lat_width + 1;
    match cursor {
        Some((lat, lon)) => {
            format!("{label}{lat:>lat_width$.precision$}°, {lon:>lon_width$.precision$}°")
        }
        None => format!(
            "{label}{:>lat_width$}, {:>lon_width$}",
            "-",
            "-",
            lat_width = lat_width + 1,
            lon_width = lon_width + 1
        ),
    }
}

/// Fit both coordinates without ellipsis, keeping comma and separator positions stable.
pub(super) fn cursor_readout(ui: &mut egui::Ui, cursor: Option<(f64, f64)>, sidebar_border_x: f32) {
    let width = (sidebar_border_x - ui.next_widget_position().x - ui.spacing().item_spacing.x)
        .clamp(0.0, ui.available_width().max(0.0));
    let mut font = egui::TextStyle::Monospace.resolve(ui.style());
    let height = ui.text_style_height(&egui::TextStyle::Monospace);
    let minimum_size = font.size * MIN_FONT_SCALE;
    let measure = |text: String, font: egui::FontId| {
        ui.painter()
            .layout_no_wrap(text, font, ui.visuals().text_color())
            .size()
            .x
    };
    let mut precision = MAX_PRECISION;
    let mut prefix = true;
    let mut minimum_font = font.clone();
    minimum_font.size = minimum_size;
    // Choose precision from the worst-case values, never from the current cursor.
    while precision > 0
        && measure(
            coordinate_text(Some((-90.0, -180.0)), precision, prefix),
            minimum_font.clone(),
        ) > width
    {
        precision -= 1;
    }
    if measure(
        coordinate_text(Some((-90.0, -180.0)), precision, prefix),
        minimum_font,
    ) > width
    {
        prefix = false;
    }
    let sample = coordinate_text(Some((-90.0, -180.0)), precision, prefix);
    while font.size > minimum_size && measure(sample.clone(), font.clone()) > width {
        font.size = (font.size - FONT_STEP_POINTS).max(minimum_size);
    }
    // At extreme widths, prioritize fitting the two whole coordinates over the label.
    while font.size > FONT_STEP_POINTS && measure(sample.clone(), font.clone()) > width {
        font.size = (font.size - FONT_STEP_POINTS).max(FONT_STEP_POINTS);
    }
    ui.allocate_ui_with_layout(
        egui::vec2(width, height),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_min_width(width);
            ui.set_max_width(width);
            if width > 0.0 {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(coordinate_text(cursor, precision, prefix)).font(font),
                    )
                    .extend(),
                );
            }
        },
    );
    ui.add(egui::Separator::default().vertical().spacing(0.0));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn precision_and_placeholder_slots_keep_the_comma_fixed() {
        for precision in 0..=MAX_PRECISION {
            let a = coordinate_text(Some((-90.0, -180.0)), precision, true);
            let b = coordinate_text(None, precision, true);
            assert_eq!(
                a.chars().position(|c| c == ','),
                b.chars().position(|c| c == ',')
            );
        }
    }
    #[test]
    fn narrow_slots_fit_both_coordinates_without_ellipsis_or_layout_shift() {
        for border_x in [120.0, 220.0, 340.0] {
            for cursor in [None, Some((-90.0, -180.0))] {
                let ctx = egui::Context::default();
                let output = crate::app::test_ui::run(&ctx, egui::RawInput::default(), |ui| {
                    egui::Panel::bottom("test").show(ui, |ui| {
                        ui.horizontal(|ui| {
                            cursor_readout(ui, cursor, border_x);
                            let next = ui.label("Center");
                            assert!(
                                (next.rect.left() - border_x - ui.spacing().item_spacing.x).abs()
                                    < 0.1
                            );
                        });
                    });
                });
                let label = output
                    .shapes
                    .iter()
                    .find_map(|s| match &s.shape {
                        egui::Shape::Text(t) if t.galley.job.text.contains(',') => Some(t),
                        _ => None,
                    })
                    .unwrap();
                assert!(!label.galley.elided);
                assert!(label.pos.x + label.galley.size().x <= border_x);
                if border_x == 340.0 && cursor.is_some() {
                    assert!(label.galley.job.text.contains(".00000"));
                }
            }
        }
    }
}
