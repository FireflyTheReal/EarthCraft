//! Ruler-only display settings; camera altitude and scale-bar units stay separate.
use earthcraft_core::{language::Lang, units::RulerUnit};
use eframe::egui;

pub(super) fn unit_selector(
    ui: &mut egui::Ui,
    mut selected: RulerUnit,
    us_locale: bool,
    lang: Lang,
) -> RulerUnit {
    ui.horizontal(|ui| {
        ui.label(lang.text("ruler-units-label"));
        egui::ComboBox::from_id_source("ruler_units")
            .width(ui.available_width())
            .wrap(true)
            .selected_text(selected.label(lang))
            .show_ui(ui, |ui| {
                for unit in RulerUnit::ALL {
                    if unit.available(us_locale) {
                        let response = ui.selectable_value(&mut selected, unit, unit.label(lang));
                        if unit == RulerUnit::FootballFields {
                            response.on_hover_text(lang.text("ruler-football-fields-help"));
                        }
                    }
                }
            })
            .response
            .on_hover_text(lang.text(if selected == RulerUnit::FootballFields {
                "ruler-football-fields-help"
            } else {
                "ruler-units-help"
            }));
    });
    selected
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selector_fits_narrow_sidebar_for_both_languages_and_all_units() {
        let ctx = egui::Context::default();
        for lang in Lang::ALL {
            for unit in RulerUnit::ALL {
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(240.0, 200.0),
                        )),
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            let bounds = ui.max_rect();
                            assert_eq!(unit_selector(ui, unit, true, lang), unit);
                            assert!(ui.min_rect().right() <= bounds.right(), "{lang:?} {unit:?}");
                        });
                    },
                );
            }
        }
    }
}
