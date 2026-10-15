use crate::app::App;
use crate::app::ui::paths::short_folder;
use eframe::egui::{self, Color32, Key, Stroke};
const PLACES_FOOTER_HEIGHT_POINTS: f32 = 40.0;
const PLACES_DETAILS_HEIGHT_POINTS: f32 = 135.0;
const PLACES_ADD_HEIGHT_POINTS: f32 = 90.0;
const MIN_TREE_HEIGHT_POINTS: f32 = 64.0;

impl App {
    pub(super) fn sidebar_places(&mut self, ui: &mut egui::Ui) {
        let lg = self.lang;
        let (mut save_now, mut save_as, mut open_dialog, mut open_typed) =
            (false, false, false, false);
        ui.horizontal_wrapped(|ui| {
            if ui
                .button(lg.text("kml-save-action"))
                .on_hover_text(lg.text("kml-save-tooltip"))
                .clicked()
            {
                save_now = true;
            }
            if ui.button(lg.text("kml-save-as-action")).clicked() {
                save_as = true;
            }
            if ui.button(lg.text("kml-open-action")).clicked() {
                open_dialog = true;
            }
        });
        let details_height = if self.place_selection.is_some() {
            PLACES_DETAILS_HEIGHT_POINTS
        } else {
            0.0
        };
        let tree_height = (ui.available_height()
            - PLACES_FOOTER_HEIGHT_POINTS
            - PLACES_ADD_HEIGHT_POINTS
            - details_height)
            .max(MIN_TREE_HEIGHT_POINTS);
        let goto = self.places_tree(ui, tree_height);
        self.place_reveal_requested = false;
        egui::CollapsingHeader::new(lg.text("sidebar-places-files-title"))
            .id_salt("places-files")
            .show(ui, |ui| {
                // Drop zone: drag a .kml file from the file manager onto this box.
                let hovering_file = ui.input(|i| !i.raw.hovered_files.is_empty());
                let (stroke_col, fill) = if hovering_file {
                    (
                        Color32::from_rgb(90, 170, 255),
                        Color32::from_rgba_unmultiplied(60, 120, 220, 45),
                    )
                } else {
                    (Color32::from_gray(90), Color32::from_black_alpha(40))
                };
                let zone = egui::Frame::NONE
                    .fill(fill)
                    .stroke(Stroke::new(
                        if hovering_file { 2.0_f32 } else { 1.0_f32 },
                        stroke_col,
                    ))
                    .corner_radius(6.0)
                    .inner_margin(egui::Margin::same(8))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.vertical_centered(|ui| {
                            ui.label(lg.text("kml-drop-zone-hint"));
                        });
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut self.kml_path)
                                .desired_width(f32::INFINITY),
                        );
                        if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                            open_typed = true;
                        }
                    });
                self.kml_drop_rect = zone.response.rect;
                ui.weak(lg.format(
                    "export-folder-label",
                    &[("path", short_folder(&self.settings.kml_dir).into())],
                ))
                .on_hover_text(&self.settings.kml_dir);
                if ui.button(lg.text("places-clear-all-action")).clicked() {
                    self.clear_places();
                }
            });
        if save_now {
            let path = self.resolve_kml_path(&self.kml_path.clone());
            self.save_kml_to(path);
        }
        if save_as {
            self.pick_kml_save_as();
        }
        if open_dialog {
            self.pick_kml_open();
        }
        if open_typed {
            let path = self.resolve_kml_path(&self.kml_path.clone());
            if path.is_file() {
                self.load_kml_from(&path);
            }
        }
        if let Some((lat, lon, alt)) = goto {
            self.fly_to(lat, lon, alt);
        }
    }
}
