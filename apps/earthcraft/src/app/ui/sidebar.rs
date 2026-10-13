use super::super::App;
use earthcraft_core::{
    math::{fmt_dist, haversine_km},
    placemark::Placemark,
};
use earthcraft_data::tiles::Provider;
use eframe::egui::{self, Color32, Key, Stroke};
impl App {
    pub(in crate::app) fn side_panel(&mut self, ctx: &egui::Context) {
        let lg = self.lang;
        let mut goto: Option<(f64, f64, f64)> = None;
        let mut del: Option<usize> = None;
        let (mut save_now, mut save_as, mut open_dialog, mut open_typed, mut shot) =
            (false, false, false, false, false);
        egui::SidePanel::left("side")
            .default_width(270.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    egui::CollapsingHeader::new(lg.text("sidebar-layers"))
                        .default_open(true)
                        .show(ui, |ui| {
                            for p in Provider::ALL {
                                ui.radio_value(&mut self.provider, p, p.name(lg));
                            }
                            if self.provider == Provider::Wayback {
                                ui.indent("wayback_ui", |ui| {
                                    if self.wayback_list.is_empty() {
                                        ui.weak(lg.text("sidebar-loading-available-dates"));
                                    } else {
                                        let n = self.wayback_list.len();
                                        let mut idx = self.wayback_idx;
                                        ui.horizontal(|ui| {
                                            if ui
                                                .small_button("◀")
                                                .on_hover_text(lg.text("sidebar-older"))
                                                .clicked()
                                                && idx + 1 < n
                                            {
                                                idx += 1;
                                            }
                                            egui::ComboBox::from_id_source("wayback_date")
                                                .selected_text(self.wayback_list[idx].date.clone())
                                                .height(320.0)
                                                .show_ui(ui, |ui| {
                                                    for (i, r) in
                                                        self.wayback_list.iter().enumerate()
                                                    {
                                                        ui.selectable_value(&mut idx, i, &r.date);
                                                    }
                                                });
                                            if ui
                                                .small_button("▶")
                                                .on_hover_text(lg.text("sidebar-newer"))
                                                .clicked()
                                                && idx > 0
                                            {
                                                idx -= 1;
                                            }
                                        });
                                        ui.weak(lg.format(
                                            "wayback-count",
                                            &[("count", n.to_string().into())],
                                        ));
                                        ui.weak(
                                            lg.text("sidebar-not-every-place-has-a-snapshot-on"),
                                        );
                                        self.set_wayback(idx);
                                    }
                                    if let Some(e) = &self.wayback_err {
                                        ui.colored_label(
                                            Color32::from_rgb(255, 120, 120),
                                            format!("✖ {e}"),
                                        );
                                    }
                                });
                            }
                            ui.separator();
                            ui.checkbox(
                                &mut self.show_grid,
                                lg.text("sidebar-latitude-longitude-grid"),
                            );
                            ui.checkbox(
                                &mut self.show_country_boundaries,
                                lg.text("sidebar-country-borders-worldwide"),
                            );
                            ui.checkbox(
                                &mut self.show_state_boundaries,
                                lg.text("sidebar-states-provinces-worldwide"),
                            );
                            ui.checkbox(
                                &mut self.show_place_labels,
                                lg.text("sidebar-country-state-and-city-names"),
                            );
                            ui.checkbox(
                                &mut self.show_city_boundaries,
                                lg.text("sidebar-city-municipality-borders-brazil-dataset"),
                            );
                            ui.checkbox(
                                &mut self.show_metro_boundaries,
                                lg.text("sidebar-metropolitan-regions-brazil-only"),
                            );
                            if self.boundary_loader.is_loading() {
                                ui.weak(lg.text("sidebar-loading-borders-and-names"));
                            }
                            if let Ok(log) = self.boundary_log.lock() {
                                for line in log.iter() {
                                    ui.weak(egui::RichText::new(line).small());
                                }
                            }
                            ui.checkbox(
                                &mut self.show_atmo,
                                lg.text("sidebar-atmosphere-and-stars"),
                            );
                            ui.checkbox(
                                &mut self.show_sun,
                                lg.text("sidebar-real-time-sunlight-day-night"),
                            );
                            ui.checkbox(&mut self.auto_rotate, lg.text("sidebar-auto-rotate"));
                        });

                    egui::CollapsingHeader::new(lg.text("sidebar-navigation"))
                        .default_open(true)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                if ui.button(lg.text("sidebar-north-up")).clicked() {
                                    self.cam.heading = 0.0;
                                }
                                if ui.button(lg.text("sidebar-global-view")).clicked() {
                                    self.fly_to(self.cam.lat, self.cam.lon, 14_000.0);
                                }
                            });
                            if !self.results.is_empty() {
                                ui.separator();
                                ui.label(lg.text("sidebar-search-results"));
                                for r in &self.results {
                                    let short: String = r.name.chars().take(48).collect();
                                    if ui.button(short).on_hover_text(&r.name).clicked() {
                                        goto = Some((r.lat, r.lon, r.alt));
                                    }
                                }
                            }
                        });

                    egui::CollapsingHeader::new(lg.text("sidebar-placemarks"))
                        .default_open(true)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.new_name)
                                        .hint_text(lg.text("sidebar-name"))
                                        .desired_width(120.0),
                                );
                                if ui.button(lg.text("sidebar-at-center")).clicked() {
                                    let name = if self.new_name.trim().is_empty() {
                                        lg.format(
                                            "placemark-default-name",
                                            &[(
                                                "number",
                                                (self.placemarks.len() + 1).to_string().into(),
                                            )],
                                        )
                                    } else {
                                        self.new_name.trim().to_string()
                                    };
                                    self.placemarks.push(Placemark {
                                        name,
                                        desc: String::new(),
                                        lat: self.cam.lat,
                                        lon: self.cam.lon,
                                    });
                                    self.new_name.clear();
                                }
                            });
                            for (i, p) in self.placemarks.iter_mut().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.add(
                                        egui::TextEdit::singleline(&mut p.name)
                                            .desired_width(110.0),
                                    );
                                    if ui
                                        .small_button(lg.text("sidebar-go"))
                                        .on_hover_text(format!("{:.5}, {:.5}", p.lat, p.lon))
                                        .clicked()
                                    {
                                        goto = Some((p.lat, p.lon, 3.0));
                                    }
                                    if ui.small_button("🗑").clicked() {
                                        del = Some(i);
                                    }
                                });
                            }
                            ui.separator();
                            // Drop zone: drag a .kml file from the file manager onto this box.
                            let hovering_file = ctx.input(|i| !i.raw.hovered_files.is_empty());
                            let (stroke_col, fill) = if hovering_file {
                                (
                                    Color32::from_rgb(90, 170, 255),
                                    Color32::from_rgba_unmultiplied(60, 120, 220, 45),
                                )
                            } else {
                                (Color32::from_gray(90), Color32::from_black_alpha(40))
                            };
                            let zone = egui::Frame::none()
                                .fill(fill)
                                .stroke(Stroke::new(
                                    if hovering_file { 2.0_f32 } else { 1.0_f32 },
                                    stroke_col,
                                ))
                                .rounding(6.0)
                                .inner_margin(egui::Margin::same(8.0))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.vertical_centered(|ui| {
                                        ui.label(lg.text("sidebar-drag-a-kml-file-here"));
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
                            ui.horizontal_wrapped(|ui| {
                                if ui
                                    .button(lg.text("sidebar-save"))
                                    .on_hover_text(
                                        lg.text("sidebar-saves-into-the-kml-folder-see-settings"),
                                    )
                                    .clicked()
                                {
                                    save_now = true;
                                }
                                if ui.button(lg.text("sidebar-save-as")).clicked() {
                                    save_as = true;
                                }
                                if ui.button(lg.text("sidebar-open-kml")).clicked() {
                                    open_dialog = true;
                                }
                            });
                            ui.weak(lg.format(
                                "files-folder",
                                &[("path", self.settings.kml_dir.as_str().into())],
                            ));
                        });

                    egui::CollapsingHeader::new(lg.text("sidebar-ruler"))
                        .default_open(true)
                        .show(ui, |ui| {
                            ui.checkbox(
                                &mut self.ruler,
                                lg.text("sidebar-measure-distances-click-on-the-globe"),
                            );
                            let total: f64 = self
                                .ruler_pts
                                .windows(2)
                                .map(|w| haversine_km(w[0], w[1]))
                                .sum();
                            ui.label(lg.format(
                                "ruler-summary",
                                &[
                                    ("count", self.ruler_pts.len().to_string().into()),
                                    ("distance", fmt_dist(total).into()),
                                ],
                            ));
                            if ui.button(lg.text("sidebar-clear-ruler")).clicked() {
                                self.ruler_pts.clear();
                            }
                        });

                    egui::CollapsingHeader::new(lg.text("sidebar-export"))
                        .default_open(false)
                        .show(ui, |ui| {
                            if ui.button(lg.text("sidebar-save-screenshot-png")).clicked() {
                                shot = true;
                            }
                            ui.weak(lg.format(
                                "files-folder",
                                &[("path", self.settings.screenshot_dir.as_str().into())],
                            ));
                            if ui.button(lg.text("sidebar-change-folders")).clicked() {
                                self.show_settings = true;
                            }
                        });

                    ui.checkbox(&mut self.show_help, lg.text("sidebar-show-controls-help"));
                    if self.show_help {
                        ui.label(lg.text("sidebar-controls-help"));
                    }
                });
            });
        if let Some(i) = del {
            self.placemarks.remove(i);
        }
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
        if shot {
            self.save_screenshot();
        }
        if let Some((la, lo, al)) = goto {
            self.fly_to(la, lo, al);
        }
    }
}
