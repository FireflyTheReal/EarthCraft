use super::super::App;
use crate::app::cache::cache_size_text;
use earthcraft_core::language::Lang;
use earthcraft_data::settings::Settings;
use earthcraft_data::settings::{MAX_MAP_SCALE, MIN_MAP_SCALE, MIN_UI_SCALE};
use eframe::egui;
const MAX_UI_SLIDER_SCALE: f32 = 2.0;

impl App {
    pub(in crate::app) fn settings_window(&mut self, ctx: &egui::Context) {
        self.refresh_disk_usage(ctx);
        let lg = self.lang;
        let mut open = self.show_settings;
        egui::Window::new(lg.text("settings-title"))
            .open(&mut open)
            .resizable(false)
            .default_width(420.0)
            .show(ctx, |ui| {
                ui.strong(lg.text("settings-text-section-title"));
                egui::Grid::new("settings_text")
                    .num_columns(2)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        ui.label(lg.text("settings-ui-scale-label"));
                        // The window auto-sizes to its (scaled) content, so changing the scale while dragging
                        // moves the slider under the cursor and feeds back into itself (shaking).
                        // Fix: only commit the new scale when the drag ends.
                        let r = ui.add(
                            egui::Slider::new(
                                &mut self.ui_scale_edit,
                                MIN_UI_SCALE..=MAX_UI_SLIDER_SCALE,
                            )
                            .step_by(0.05)
                            .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
                        );
                        if r.drag_stopped() || (r.changed() && !r.dragged()) {
                            self.settings.ui_scale = self.ui_scale_edit;
                        }
                        ui.end_row();
                        ui.label(lg.text("settings-map-scale-label"));
                        ui.add(
                            egui::Slider::new(
                                &mut self.settings.map_scale,
                                MIN_MAP_SCALE..=MAX_MAP_SCALE,
                            )
                            .step_by(0.05)
                            .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
                        );
                        ui.end_row();
                    });
                ui.separator();
                ui.strong(lg.text("settings-folders-section-title"));
                let folder_row =
                    |ui: &mut egui::Ui, label: &str, value: &mut String, title: &str| {
                        ui.label(label);
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(value).desired_width(300.0));
                            if ui
                                .button(lg.text("settings-folder-browse-action"))
                                .clicked()
                            {
                                let mut dlg = rfd::FileDialog::new().set_title(title);
                                if std::path::Path::new(value.as_str()).is_dir() {
                                    dlg = dlg.set_directory(value.as_str());
                                }
                                if let Some(dir) = dlg.pick_folder() {
                                    *value = dir.to_string_lossy().into_owned();
                                }
                            }
                        });
                    };
                folder_row(
                    ui,
                    lg.text("settings-screenshot-folder-label"),
                    &mut self.settings.screenshot_dir,
                    lg.text("settings-screenshot-folder-dialog-title"),
                );
                folder_row(
                    ui,
                    lg.text("settings-kml-folder-label"),
                    &mut self.settings.kml_dir,
                    lg.text("settings-kml-folder-dialog-title"),
                );
                ui.weak(lg.text("settings-folder-creation-help"));
                ui.separator();
                ui.strong(lg.text("settings-cache-section-title"));
                ui.weak(lg.text("settings-cache-clear-help"));
                let clearing = self.cache_controls.clearing();
                ui.horizontal(|ui| {
                    let label = lg.text(if clearing {
                        "settings-cache-clearing-label"
                    } else {
                        "settings-cache-clear-action"
                    });
                    if ui
                        .add_enabled(!clearing, egui::Button::new(label))
                        .clicked()
                    {
                        self.clear_disk_cache(ctx);
                    }
                    if clearing {
                        ui.spinner();
                    }
                    match &self.cache_controls.usage {
                        Some(Ok(bytes)) => {
                            ui.label(lg.format(
                                "settings-cache-disk-usage",
                                &[("size", cache_size_text(*bytes).into())],
                            ));
                        }
                        Some(Err(error)) => {
                            ui.label(Lang::En.text("settings-cache-usage-unavailable"))
                                .on_hover_text(error);
                        }
                        None => {
                            ui.weak(lg.text("settings-cache-measuring-label"));
                        }
                    }
                });
                ui.separator();
                if ui.button(lg.text("settings-reset-action")).clicked() {
                    self.settings = Settings::default();
                }
            });
        self.show_settings = open;
    }
}
