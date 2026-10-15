use earthcraft_core::{coordinates::CoordinateDisplay, language::Lang};
use eframe::egui;
const MIN_FONT_SCALE: f32 = 0.75;
const FONT_STEP_POINTS: f32 = 0.25;
const WIDTH_SAMPLES: [(f64, f64); 4] = [
    (-89.999_999, -179.999_999),
    (-85.0, 45.0),
    (-0.000_001, -179.999_999),
    (-79.999_999, 179.999_999),
];

fn coordinate_text(cursor: Option<(f64, f64)>, display: CoordinateDisplay) -> String {
    let sample = display
        .format(-90.0, -180.0)
        .unwrap_or_else(|error| error.to_string());
    let text = cursor.map(|(lat, lon)| {
        display
            .format(lat, lon)
            .unwrap_or_else(|error| error.to_string())
    });
    if let Some((sample_lat, sample_lon)) = sample.split_once(", ") {
        let (lat, lon) = text
            .as_deref()
            .and_then(|s| s.split_once(", "))
            .unwrap_or(("—", "—"));
        format!(
            "{lat:>lat_width$}, {lon:>lon_width$}",
            lat_width = sample_lat.chars().count(),
            lon_width = sample_lon.chars().count()
        )
    } else {
        text.unwrap_or_else(|| "—".into())
    }
}

/// Keep selected precision and both coordinates; fit the font instead of rounding the value.
pub(super) fn cursor_readout(
    ui: &mut egui::Ui,
    cursor: Option<(f64, f64)>,
    sidebar_border_x: f32,
    display: CoordinateDisplay,
    lang: Lang,
) {
    let width = (sidebar_border_x - ui.next_widget_position().x - ui.spacing().item_spacing.x)
        .clamp(0.0, ui.available_width().max(0.0));
    let height = ui.text_style_height(&egui::TextStyle::Monospace);
    let mut font = egui::TextStyle::Monospace.resolve(ui.style());
    let minimum_size = font.size * MIN_FONT_SCALE;
    let wrap = |coordinates: String, prefix| {
        if prefix {
            lang.format(
                "coordinates-cursor-readout",
                &[("coordinates", coordinates.into())],
            )
        } else {
            coordinates
        }
    };
    let samples = WIDTH_SAMPLES.map(|position| coordinate_text(Some(position), display));
    let measure = |prefix, size| {
        samples
            .iter()
            .map(|sample| {
                let text = wrap(sample.clone(), prefix);
                ui.painter()
                    .layout_no_wrap(
                        text,
                        egui::FontId::monospace(size),
                        ui.visuals().text_color(),
                    )
                    .size()
                    .x
            })
            .fold(0.0_f32, f32::max)
    };
    let prefix = measure(true, minimum_size) <= width;
    while font.size > FONT_STEP_POINTS && measure(prefix, font.size) > width {
        font.size = (font.size - FONT_STEP_POINTS).max(FONT_STEP_POINTS);
    }
    ui.allocate_ui_with_layout(
        egui::vec2(width, height),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_min_width(width);
            ui.set_max_width(width);
            if width > 0.0 {
                let text = wrap(coordinate_text(cursor, display), prefix);
                let response = ui.add(
                    egui::Label::new(egui::RichText::new(text).font(font))
                        .extend()
                        .sense(if cursor.is_some() {
                            egui::Sense::click()
                        } else {
                            egui::Sense::hover()
                        }),
                );
                if let Some((lat, lon)) = cursor {
                    match display.format(lat, lon) {
                        Ok(coordinates) => {
                            if response.clicked() {
                                ui.copy_text(coordinates.clone());
                            }
                            response
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .on_hover_text(lang.format(
                                    "coordinates-copy-tooltip",
                                    &[("coordinates", coordinates.into())],
                                ));
                        }
                        Err(error) => {
                            response.on_hover_text(error.to_string());
                        }
                    }
                }
            }
        },
    );
    ui.add(egui::Separator::default().vertical().spacing(0.0));
}
#[cfg(test)]
mod tests {
    use super::*;
    use earthcraft_core::coordinates::CoordinateFormat;
    #[test]
    fn angular_placeholder_and_coordinates_keep_the_comma_fixed() {
        for format in [
            CoordinateFormat::DecimalDegrees,
            CoordinateFormat::Dms,
            CoordinateFormat::Ddm,
        ] {
            for precision in format.precision_range() {
                let display = CoordinateDisplay { format, precision };
                let a = coordinate_text(Some((-90.0, -180.0)), display);
                let b = coordinate_text(None, display);
                let c = coordinate_text(Some((1.0, 2.0)), display);
                assert_eq!(
                    a.chars().position(|c| c == ','),
                    b.chars().position(|c| c == ',')
                );
                assert_eq!(
                    a.chars().position(|c| c == ','),
                    c.chars().position(|c| c == ',')
                );
            }
        }
    }
    #[test]
    fn narrow_readouts_fit_without_losing_the_configured_precision_or_shifting_the_next_item() {
        for format in CoordinateFormat::ALL {
            let display = CoordinateDisplay {
                format,
                precision: format.default_precision(),
            };
            for border_x in [120.0, 220.0, 340.0] {
                let ctx = egui::Context::default();
                let output = crate::app::test_ui::run(&ctx, egui::RawInput::default(), |ui| {
                    egui::Panel::bottom("test").show(ui, |ui| {
                        ui.horizontal(|ui| {
                            cursor_readout(
                                ui,
                                Some((-12.123_456, -123.123_456)),
                                border_x,
                                display,
                                Lang::En,
                            );
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
                        egui::Shape::Text(t) if t.galley.job.text != "Center" => Some(t),
                        _ => None,
                    })
                    .unwrap();
                assert!(!label.galley.elided);
                assert!(
                    label.pos.x + label.galley.size().x <= border_x,
                    "{:?}",
                    label.galley.job.text
                );
                assert!(
                    label
                        .galley
                        .job
                        .text
                        .contains(&display.format(-12.123_456, -123.123_456).unwrap()),
                    "{:?}",
                    label.galley.job.text
                );
            }
        }
    }
}
