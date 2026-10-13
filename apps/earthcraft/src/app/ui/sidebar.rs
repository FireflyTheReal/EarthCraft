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
        egui::SidePanel::left("side").default_width(270.0).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::CollapsingHeader::new(lg.pick("Layers", "Camadas")).default_open(true).show(ui, |ui| {
                    for p in Provider::ALL {
                        ui.radio_value(&mut self.provider, p, p.name(lg));
                    }
                    if self.provider == Provider::Wayback {
                        ui.indent("wayback_ui", |ui| {
                            if self.wayback_list.is_empty() {
                                ui.weak(lg.pick("⏳ Loading available dates…", "⏳ Carregando datas disponíveis…"));
                            } else {
                                let n = self.wayback_list.len();
                                let mut idx = self.wayback_idx;
                                ui.horizontal(|ui| {
                                    if ui.small_button("◀").on_hover_text(lg.pick("Older", "Mais antiga")).clicked() && idx + 1 < n {
                                        idx += 1;
                                    }
                                    egui::ComboBox::from_id_source("wayback_date")
                                        .selected_text(self.wayback_list[idx].date.clone())
                                        .height(320.0)
                                        .show_ui(ui, |ui| {
                                            for (i, r) in self.wayback_list.iter().enumerate() {
                                                ui.selectable_value(&mut idx, i, &r.date);
                                            }
                                        });
                                    if ui.small_button("▶").on_hover_text(lg.pick("Newer", "Mais recente")).clicked() && idx > 0 {
                                        idx -= 1;
                                    }
                                });
                                ui.weak(format!("{} {}", n, lg.pick("snapshots available", "versões disponíveis")));
                                ui.weak(lg.pick(
                                    "Not every place has a snapshot on every date; missing areas fall back to the nearest available imagery.",
                                    "Nem todo lugar tem imagem em todas as datas; áreas sem imagem mostram a versão mais próxima disponível.",
                                ));
                                self.set_wayback(idx);
                            }
                            if let Some(e) = &self.wayback_err {
                                ui.colored_label(Color32::from_rgb(255, 120, 120), format!("✖ {e}"));
                            }
                        });
                    }
                    ui.separator();
                    ui.checkbox(&mut self.show_grid, lg.pick("Latitude/longitude grid", "Grade de latitude/longitude"));
                    ui.checkbox(&mut self.show_country_boundaries, lg.pick("Country borders (worldwide)", "Limites dos países (mundo todo)"));
                    ui.checkbox(&mut self.show_state_boundaries, lg.pick("States / provinces (worldwide)", "Estados / províncias (mundo todo)"));
                    ui.checkbox(&mut self.show_place_labels, lg.pick("Country, state and city names", "Nomes de países, estados e cidades"));
                    ui.checkbox(&mut self.show_city_boundaries, lg.pick("City / municipality borders (Brazil only)", "Limites de cidades / municípios (disponível apenas para o Brasil)"));
                    ui.checkbox(&mut self.show_metro_boundaries, lg.pick("Metropolitan regions (Brazil only)", "Regiões metropolitanas (disponível apenas para o Brasil)"));
                    if self.boundaries_rx.is_some() {
                        ui.weak(lg.pick("⏳ Loading borders and names…", "⏳ Carregando limites e nomes…"));
                    }
                    if let Ok(log) = self.boundary_log.lock() {
                        for line in log.iter() { ui.weak(egui::RichText::new(line).small()); }
                    }
                    ui.checkbox(&mut self.show_atmo, lg.pick("Atmosphere and stars", "Atmosfera e estrelas"));
                    ui.checkbox(&mut self.show_sun, lg.pick("Real-time sunlight (day/night)", "Luz do sol em tempo real (dia/noite)"));
                    ui.checkbox(&mut self.auto_rotate, lg.pick("Auto-rotate", "Girar automaticamente"));
                });

                egui::CollapsingHeader::new(lg.pick("Navigation", "Navegação")).default_open(true).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button(lg.pick("🧭 North up", "🧭 Norte para cima")).clicked() {
                            self.cam.heading = 0.0;
                        }
                        if ui.button(lg.pick("🌐 Global view", "🌐 Visão global")).clicked() {
                            self.fly_to(self.cam.lat, self.cam.lon, 14_000.0);
                        }
                    });
                    if !self.results.is_empty() {
                        ui.separator();
                        ui.label(lg.pick("Search results:", "Resultados da busca:"));
                        for r in &self.results {
                            let short: String = r.name.chars().take(48).collect();
                            if ui.button(short).on_hover_text(&r.name).clicked() {
                                goto = Some((r.lat, r.lon, r.alt));
                            }
                        }
                    }
                });

                egui::CollapsingHeader::new(lg.pick("Placemarks", "Locais (Placemarks)")).default_open(true).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.new_name).hint_text(lg.pick("Name", "Nome")).desired_width(120.0));
                        if ui.button(lg.pick("➕ At center", "➕ No centro")).clicked() {
                            let name =
                                if self.new_name.trim().is_empty() { format!("{} {}", lg.pick("Place", "Local"), self.placemarks.len() + 1) } else { self.new_name.trim().to_string() };
                            self.placemarks.push(Placemark { name, desc: String::new(), lat: self.cam.lat, lon: self.cam.lon });
                            self.new_name.clear();
                        }
                    });
                    for (i, p) in self.placemarks.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut p.name).desired_width(110.0));
                            if ui.small_button(lg.pick("Go", "Ir")).on_hover_text(format!("{:.5}, {:.5}", p.lat, p.lon)).clicked() {
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
                        (Color32::from_rgb(90, 170, 255), Color32::from_rgba_unmultiplied(60, 120, 220, 45))
                    } else {
                        (Color32::from_gray(90), Color32::from_black_alpha(40))
                    };
                    let zone = egui::Frame::none()
                        .fill(fill)
                        .stroke(Stroke::new(if hovering_file { 2.0_f32 } else { 1.0_f32 }, stroke_col))
                        .rounding(6.0)
                        .inner_margin(egui::Margin::same(8.0))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.vertical_centered(|ui| {
                                ui.label(lg.pick("📥 Drag a .kml file here", "📥 Arraste um arquivo .kml aqui"));
                            });
                            let r = ui.add(egui::TextEdit::singleline(&mut self.kml_path).desired_width(f32::INFINITY));
                            if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                                open_typed = true;
                            }
                        });
                    self.kml_drop_rect = zone.response.rect;
                    ui.horizontal_wrapped(|ui| {
                        if ui.button(lg.pick("💾 Save", "💾 Salvar")).on_hover_text(lg.pick("Saves into the KML folder (see Settings)", "Salva na pasta de KML (veja Configurações)")).clicked() {
                            save_now = true;
                        }
                        if ui.button(lg.pick("💾 Save as…", "💾 Salvar como…")).clicked() {
                            save_as = true;
                        }
                        if ui.button(lg.pick("📂 Open KML…", "📂 Abrir KML…")).clicked() {
                            open_dialog = true;
                        }
                    });
                    ui.weak(format!("{} {}", lg.pick("Folder:", "Pasta:"), self.settings.kml_dir));
                });

                egui::CollapsingHeader::new(lg.pick("Ruler", "Régua")).default_open(true).show(ui, |ui| {
                    ui.checkbox(&mut self.ruler, lg.pick("Measure distances (click on the globe)", "Medir distâncias (clique no globo)"));
                    let total: f64 = self.ruler_pts.windows(2).map(|w| haversine_km(w[0], w[1])).sum();
                    ui.label(format!("{}: {}   Total: {}", lg.pick("Points", "Pontos"), self.ruler_pts.len(), fmt_dist(total)));
                    if ui.button(lg.pick("Clear ruler", "Limpar régua")).clicked() {
                        self.ruler_pts.clear();
                    }
                });

                egui::CollapsingHeader::new(lg.pick("Export", "Exportar")).default_open(false).show(ui, |ui| {
                    if ui.button(lg.pick("📷 Save screenshot (PNG)", "📷 Salvar captura (PNG)")).clicked() {
                        shot = true;
                    }
                    ui.weak(format!("{} {}", lg.pick("Folder:", "Pasta:"), self.settings.screenshot_dir));
                    if ui.button(lg.pick("⚙ Change folders…", "⚙ Mudar pastas…")).clicked() {
                        self.show_settings = true;
                    }
                });

                ui.checkbox(&mut self.show_help, lg.pick("Show controls help", "Mostrar ajuda de controles"));
                if self.show_help {
                    ui.label(lg.pick(
                        "• Drag (left): rotate the globe\n\
                         • Right-drag / Shift+drag: rotate the view\n\
                         • Wheel / pinch or +/-: zoom\n\
                         • Double-click: fly to the point\n\
                         • Arrow keys: move   • N: north up\n\
                         • Right-click: context menu",
                        "• Arrastar (esq.): girar o globo\n\
                         • Botão direito/Shift+arrastar: girar a vista\n\
                         • Roda/pinça ou +/-: zoom\n\
                         • Duplo clique: voar até o ponto\n\
                         • Setas: mover   • N: norte para cima\n\
                         • Botão direito (clique): menu de contexto",
                    ));
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
