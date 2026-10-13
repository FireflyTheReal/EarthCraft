use super::super::App;
use earthcraft_data::settings::Settings;
use earthcraft_data::settings::{MAX_MAP_SCALE, MIN_MAP_SCALE, MIN_UI_SCALE};
use eframe::egui;
const MAX_UI_SLIDER_SCALE: f32 = 2.0;

impl App {
    pub(in crate::app) fn settings_window(&mut self, ctx: &egui::Context) {
        let lg = self.lang;
        let mut open = self.show_settings;
        egui::Window::new(lg.pick("⚙ Settings", "⚙ Configurações"))
            .open(&mut open)
            .resizable(false)
            .default_width(420.0)
            .show(ctx, |ui| {
                ui.strong(lg.pick("Text size", "Tamanho do texto"));
                egui::Grid::new("settings_text")
                    .num_columns(2)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        ui.label(lg.pick("Buttons and panels", "Botões e painéis"));
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
                        ui.label(lg.pick(
                            "Map text (names, ruler, scale)",
                            "Texto do mapa (nomes, régua, escala)",
                        ));
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
                ui.strong(lg.pick("Where to save files", "Onde salvar os arquivos"));
                let folder_row =
                    |ui: &mut egui::Ui, label: &str, value: &mut String, title: &str| {
                        ui.label(label);
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(value).desired_width(300.0));
                            if ui.button(lg.pick("📁 Browse…", "📁 Procurar…")).clicked() {
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
                    lg.pick("Screenshots (PNG):", "Capturas de tela (PNG):"),
                    &mut self.settings.screenshot_dir,
                    lg.pick("Screenshots folder", "Pasta das capturas"),
                );
                folder_row(
                    ui,
                    lg.pick("KML files:", "Arquivos KML:"),
                    &mut self.settings.kml_dir,
                    lg.pick("KML folder", "Pasta dos KML"),
                );
                ui.weak(lg.pick(
                    "Folders are created automatically when saving.",
                    "As pastas são criadas automaticamente ao salvar.",
                ));
                ui.separator();
                if ui
                    .button(lg.pick("Restore defaults", "Restaurar padrões"))
                    .clicked()
                {
                    self.settings = Settings::default();
                }
            });
        self.show_settings = open;
    }
}
