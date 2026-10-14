use super::super::App;
use earthcraft_core::language::Lang;
use eframe::egui::{self, Key, Rect};
impl App {
    pub(in crate::app) fn top_bar(&mut self, ctx: &egui::Context) {
        let lg = self.lang;
        let mut field_rect = Rect::NOTHING;
        const BAR_MARGIN_POINTS: egui::Margin = egui::Margin::symmetric(8.0, 4.0);
        egui::TopBottomPanel::top("top")
            .frame(egui::Frame::side_top_panel(&ctx.style()).inner_margin(BAR_MARGIN_POINTS))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let sidebar_hint = if self.sidebar_open {
                        lg.text("sidebar-hide-tooltip")
                    } else {
                        lg.text("sidebar-show-tooltip")
                    };
                    ui.toggle_value(&mut self.sidebar_open, "☰")
                        .on_hover_text(sidebar_hint);
                    ui.strong("🌍 EarthCraft");
                    if ui.button(lg.text("top-explore-action")).clicked() {
                        self.explorer.open = true;
                    }
                    for (name, body) in [
                        (lg.text("top-earth"), earthcraft_core::body::Body::Earth),
                        (lg.text("top-moon"), earthcraft_core::body::Body::Moon),
                        (
                            lg.text("top-custom-planet"),
                            earthcraft_core::body::Body::Custom,
                        ),
                    ] {
                        if ui.button(name).clicked() {
                            self.explorer.choose(body, ctx);
                            if body == earthcraft_core::body::Body::Custom {
                                self.explorer.open = true;
                            }
                            self.touch();
                        }
                    }
                    ui.separator();

                    // Keys for the suggestion list must be consumed before the TextEdit sees them.
                    let popup = self.sugg_open && !self.suggestions.is_empty();
                    let (mut down, mut up, mut esc) = (false, false, false);
                    if popup {
                        ctx.input_mut(|i| {
                            down = i.consume_key(egui::Modifiers::NONE, Key::ArrowDown);
                            up = i.consume_key(egui::Modifiers::NONE, Key::ArrowUp);
                            esc = i.consume_key(egui::Modifiers::NONE, Key::Escape);
                        });
                        let n = self.suggestions.len();
                        if down {
                            self.sugg_sel = Some(self.sugg_sel.map_or(0, |i| (i + 1) % n));
                        }
                        if up {
                            self.sugg_sel = Some(self.sugg_sel.map_or(n - 1, |i| (i + n - 1) % n));
                        }
                        if esc {
                            self.sugg_open = false;
                        }
                    }

                    let r = ui.add(
                        egui::TextEdit::singleline(&mut self.query)
                            .hint_text(lg.text("search-input-placeholder"))
                            .desired_width(340.0 * self.settings.ui_scale.max(1.0)),
                    );
                    field_rect = r.rect;
                    if r.changed() {
                        self.on_query_changed();
                    }
                    if r.gained_focus()
                        && !self.query.trim().is_empty()
                        && !self.suggestions.is_empty()
                    {
                        self.sugg_open = true;
                    }
                    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                    if enter {
                        match (self.sugg_open, self.sugg_sel) {
                            (true, Some(i)) => self.choose_suggestion(i),
                            _ => {
                                self.sugg_open = false;
                                self.start_search(ctx);
                            }
                        }
                    }
                    if ui.button(lg.text("search-submit-action")).clicked() {
                        self.sugg_open = false;
                        self.start_search(ctx);
                    }
                    ui.separator();
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button("⚙")
                            .on_hover_text(lg.text("settings-open-tooltip"))
                            .clicked()
                        {
                            self.show_settings = !self.show_settings;
                        }
                        let mut chosen = self.lang;
                        egui::ComboBox::from_id_source("lang_select")
                            .selected_text(format!("🌐 {}", chosen.label()))
                            .show_ui(ui, |ui| {
                                for l in Lang::ALL {
                                    ui.selectable_value(&mut chosen, l, l.label());
                                }
                            });
                        if chosen != self.lang {
                            self.set_lang(chosen);
                        }
                        let compact = match &self.status_file {
                            Some(file) if file.message == self.status => {
                                let full_path = file.path.to_string_lossy();
                                self.status.replace(
                                    full_path.as_ref(),
                                    &super::paths::short_folder(&full_path),
                                )
                            }
                            _ => self.status.clone(),
                        };
                        let available = ui.available_width().max(0.0);
                        let compact = super::paths::fit_tail(ui, &compact, available);
                        ui.add_sized(
                            [available, ui.spacing().interact_size.y],
                            egui::Label::new(compact).truncate(true),
                        )
                        .on_hover_text(&self.status);
                    });
                });
            });
        self.suggestion_popup(ctx, field_rect);
    }

    /// Status below the map.
    pub(in crate::app) fn bottom_bar(&mut self, ctx: &egui::Context) {
        let lg = self.lang;
        egui::TopBottomPanel::bottom("bottom").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.set_clip_rect(ui.max_rect().intersect(ui.clip_rect()));
                let sidebar_right =
                    egui::containers::panel::PanelState::load(ctx, egui::Id::new("side")).map_or(
                        ctx.screen_rect().left() + super::sidebar::DEFAULT_SIDEBAR_WIDTH_POINTS,
                        |state| state.rect.right(),
                    );
                // egui 0.27 paints the SidePanel separator one point inside its rect.
                const SIDEBAR_SEPARATOR_INSET_POINTS: f32 = 1.0;
                let sidebar_border_x = ui
                    .painter()
                    .round_to_pixel(sidebar_right - SIDEBAR_SEPARATOR_INSET_POINTS);
                cursor_readout(ui, self.cursor, sidebar_border_x);
                ui.monospace(lg.format(
                    "statusbar-camera-center",
                    &[
                        ("latitude", format!("{:.4}", self.cam.lat).into()),
                        ("longitude", format!("{:.4}", self.cam.lon).into()),
                    ],
                ));
                ui.separator();
                let altitude = lg.format(
                    "statusbar-camera-altitude",
                    &[(
                        "distance",
                        self.distance_units().altitude(self.cam.alt).into(),
                    )],
                );
                let altitude_response = ui
                    .add(
                        egui::Label::new(egui::RichText::new(altitude).monospace())
                            .sense(egui::Sense::click()),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(lg.text("altitude-edit-tooltip"));
                if altitude_response.clicked() {
                    self.altitude_editor = Some(super::altitude::AltitudeEditor::new(
                        self.cam.alt,
                        self.distance_units(),
                        altitude_response.rect.left_top(),
                    ));
                }
                ui.separator();
                let n = self.cache.inflight();
                if n > 0 {
                    ui.monospace(lg.format(
                        "statusbar-tiles-loading",
                        &[("count", n.to_string().into())],
                    ));
                } else {
                    ui.monospace("✔ tiles ok");
                }
            });
        });
    }

    // ───────── Globe / Globo ─────────
}

/// Keep the coordinate slots and separator fixed at the remembered sidebar edge.
fn cursor_readout(ui: &mut egui::Ui, cursor: Option<(f64, f64)>, sidebar_border_x: f32) {
    const CURSOR_LAYOUT_SAMPLE: &str = "Cursor: -90.00000°, -180.00000°";
    const MIN_CURSOR_FONT_SCALE: f32 = 0.75;
    const CURSOR_FONT_STEP_POINTS: f32 = 0.25;
    let width = (sidebar_border_x - ui.next_widget_position().x - ui.spacing().item_spacing.x)
        .clamp(0.0, ui.available_width().max(0.0));
    let mut font = egui::TextStyle::Monospace.resolve(ui.style());
    let size = ui
        .painter()
        .layout_no_wrap(
            CURSOR_LAYOUT_SAMPLE.into(),
            font.clone(),
            ui.visuals().text_color(),
        )
        .size();
    // Fit the full coordinate slots first, with a readability floor for narrow
    // user-resized panels. Use the same sample for values and placeholders.
    let min_font_size = font.size * MIN_CURSOR_FONT_SCALE;
    font.size *= (width / size.x).clamp(MIN_CURSOR_FONT_SCALE, 1.0);
    // Glyph advances are rounded at the current DPI, so proportional scaling
    // alone can still leave the final character outside the slot.
    while font.size > min_font_size
        && ui
            .painter()
            .layout_no_wrap(
                CURSOR_LAYOUT_SAMPLE.into(),
                font.clone(),
                ui.visuals().text_color(),
            )
            .size()
            .x
            > width
    {
        font.size = (font.size - CURSOR_FONT_STEP_POINTS).max(min_font_size);
    }
    ui.allocate_ui_with_layout(
        egui::vec2(width, size.y),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_min_width(width);
            ui.set_max_width(width);
            let text = match cursor {
                Some((latitude, longitude)) => {
                    format!("Cursor: {latitude:>9.5}°, {longitude:>10.5}°")
                }
                None => format!("Cursor: {:>10}, {:>11}", "-", "-"),
            };
            ui.add(egui::Label::new(egui::RichText::new(&text).font(font)).truncate(true))
                .on_hover_text(text);
        },
    );
    ui.add(egui::Separator::default().vertical().spacing(0.0));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cursor_slot_scales_with_width_and_does_not_push_other_fields() {
        for border_x in [120.0, 220.0, 340.0] {
            for cursor in [None, Some((-90.0, -180.0))] {
                let ctx = egui::Context::default();
                let mut normal_font_size = 0.0;
                let output = ctx.run(egui::RawInput::default(), |ctx| {
                    egui::TopBottomPanel::bottom("test_status").show(ctx, |ui| {
                        normal_font_size = egui::TextStyle::Monospace.resolve(ui.style()).size;
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
                let text = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) if text.galley.job.text.starts_with("Cursor:") => {
                            Some(text)
                        }
                        _ => None,
                    })
                    .expect("cursor label is painted");
                let font_size = text.galley.job.sections[0].format.font_id.size;
                assert!(font_size >= normal_font_size * 0.75);
                assert!(font_size <= normal_font_size);
                if border_x == 120.0 {
                    assert!(text.galley.elided);
                } else {
                    assert!(
                        !text.galley.elided,
                        "width={border_x} cursor={cursor:?} font={font_size} actual={:?}",
                        text.galley.size()
                    );
                }
            }
        }
    }
}
