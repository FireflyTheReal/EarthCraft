//! Relative text-size tracks share a 100% midpoint despite different limits.
use eframe::egui;
const DEFAULT_SCALE: f64 = 1.0;
const SCALE_STEP: f64 = 0.05;
const PERCENT_PER_SCALE: f64 = 100.0;

fn position(scale: f64, minimum: f64, maximum: f64) -> f64 {
    if scale <= DEFAULT_SCALE {
        0.5 * (scale - minimum) / (DEFAULT_SCALE - minimum)
    } else {
        0.5 + 0.5 * (scale - DEFAULT_SCALE) / (maximum - DEFAULT_SCALE)
    }
}
fn scale_at(position: f64, minimum: f64, maximum: f64) -> f64 {
    if position <= 0.5 {
        minimum + position * 2.0 * (DEFAULT_SCALE - minimum)
    } else {
        DEFAULT_SCALE + (position - 0.5) * 2.0 * (maximum - DEFAULT_SCALE)
    }
}
fn stepped(scale: f64, minimum: f64, maximum: f64) -> f32 {
    ((scale / SCALE_STEP).round() * SCALE_STEP).clamp(minimum, maximum) as f32
}

pub(super) fn relative_scale_slider(
    ui: &mut egui::Ui,
    scale: &mut f32,
    minimum: f32,
    maximum: f32,
) -> egui::Response {
    let minimum = minimum as f64;
    let maximum = maximum as f64;
    ui.horizontal(|ui| {
        let before = *scale;
        let mut track = ui.add(
            egui::Slider::from_get_set(0.0..=1.0, |value| {
                if let Some(value) = value {
                    *scale = stepped(scale_at(value, minimum, maximum), minimum, maximum);
                }
                position(*scale as f64, minimum, maximum)
            })
            .handle_shape(egui::style::HandleShape::Circle)
            .show_value(false)
            .smart_aim(false),
        );
        // Preserve five-percentage-point keyboard steps in actual scale units.
        if track.has_focus() {
            let delta = ui.input(|input| {
                input.num_presses(egui::Key::ArrowRight) as i32
                    - input.num_presses(egui::Key::ArrowLeft) as i32
            });
            if delta != 0 {
                *scale = stepped(before as f64 + delta as f64 * SCALE_STEP, minimum, maximum);
                if *scale != before {
                    track.mark_changed();
                }
            }
        }
        // The numeric field edits real scale values, independently of track position.
        let mut number = ui.add(
            egui::DragValue::new(scale)
                .speed(SCALE_STEP)
                .range(minimum..=maximum)
                .custom_formatter(|value, _| format!("{:.0}%", value * PERCENT_PER_SCALE))
                .custom_parser(|text| {
                    let value: f64 = text.trim().trim_end_matches('%').trim().parse().ok()?;
                    value.is_finite().then_some(value / PERCENT_PER_SCALE)
                }),
        );
        if number.changed() {
            *scale = stepped(*scale as f64, minimum, maximum);
            number.mark_changed();
        }
        track | number
    })
    .inner
}

#[cfg(test)]
mod tests {
    use super::*;
    use earthcraft_data::settings::{MAX_MAP_SCALE, MIN_MAP_SCALE, MIN_UI_SCALE};
    #[test]
    fn both_ranges_share_midpoint_and_preserve_limits() {
        for (minimum, maximum) in [
            (MIN_UI_SCALE, super::super::settings::MAX_UI_SLIDER_SCALE),
            (MIN_MAP_SCALE, MAX_MAP_SCALE),
        ] {
            let (minimum, maximum) = (minimum as f64, maximum as f64);
            assert_eq!(position(DEFAULT_SCALE, minimum, maximum), 0.5);
            assert_eq!(scale_at(0.5, minimum, maximum), DEFAULT_SCALE);
            assert_eq!(scale_at(0.0, minimum, maximum), minimum);
            assert_eq!(scale_at(1.0, minimum, maximum), maximum);
            for scale in [minimum, 0.8, 1.0, 1.5, maximum] {
                assert!(
                    (scale_at(position(scale, minimum, maximum), minimum, maximum) - scale).abs()
                        < 1e-12
                );
            }
        }
    }
    #[test]
    fn keyboard_changes_actual_scale_by_five_percent_on_both_tracks() {
        for (minimum, maximum) in [
            (MIN_UI_SCALE, super::super::settings::MAX_UI_SLIDER_SCALE),
            (MIN_MAP_SCALE, MAX_MAP_SCALE),
        ] {
            for (key, expected) in [(egui::Key::ArrowLeft, 0.95), (egui::Key::ArrowRight, 1.05)] {
                let ctx = egui::Context::default();
                let mut scale = 1.0;
                let _ = crate::app::test_ui::run(&ctx, egui::RawInput::default(), |ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        relative_scale_slider(ui, &mut scale, minimum, maximum).request_focus();
                    });
                });
                let _ = crate::app::test_ui::run(
                    &ctx,
                    egui::RawInput {
                        events: vec![egui::Event::Key {
                            key,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: egui::Modifiers::NONE,
                        }],
                        ..Default::default()
                    },
                    |ui| {
                        egui::CentralPanel::default().show(ui, |ui| {
                            relative_scale_slider(ui, &mut scale, minimum, maximum);
                        });
                    },
                );
                assert_eq!(scale, expected);
            }
        }
    }
    #[test]
    fn actual_slider_handles_align_at_one_hundred_percent() {
        let ctx = egui::Context::default();
        let output = crate::app::test_ui::run(&ctx, egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                relative_scale_slider(
                    ui,
                    &mut 1.0,
                    MIN_UI_SCALE,
                    super::super::settings::MAX_UI_SLIDER_SCALE,
                );
                relative_scale_slider(ui, &mut 1.0, MIN_MAP_SCALE, MAX_MAP_SCALE);
            });
        });
        let handles: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Circle(circle) => Some(circle.center.x),
                _ => None,
            })
            .collect();
        assert_eq!(handles.len(), 2);
        assert_eq!(handles[0], handles[1]);
    }
}
