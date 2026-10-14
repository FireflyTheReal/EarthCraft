//! Ruler-only display settings; camera altitude and scale-bar units stay separate.
use earthcraft_core::{language::Lang, units::RulerUnit};
use eframe::egui;
mod points;
pub(in crate::app) use points::PointEditor;

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
            .selected_text(selected.selector_label(lang, us_locale))
            .show_ui(ui, |ui| {
                super::selection::size_menu(
                    ui,
                    RulerUnit::ALL
                        .into_iter()
                        .map(|unit| unit.selector_label(lang, us_locale)),
                );
                for unit in RulerUnit::ALL {
                    super::selection::full_width_value(
                        ui,
                        &mut selected,
                        unit,
                        unit.selector_label(lang, us_locale),
                    )
                    .on_hover_text(unit.help(lang));
                }
            });
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
                for us_locale in [true, false] {
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
                                assert_eq!(unit_selector(ui, unit, us_locale, lang), unit);
                                assert!(
                                    ui.min_rect().right() <= bounds.right(),
                                    "{lang:?} {unit:?}"
                                );
                            });
                        },
                    );
                }
            }
        }
    }
}
