use super::super::App;
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Vec2};
impl App {
    pub(in crate::app) fn suggestion_popup(&mut self, ctx: &egui::Context, field: Rect) {
        if !self.sugg_open || self.suggestions.is_empty() || field == Rect::NOTHING {
            return;
        }
        let mut picked: Option<usize> = None;
        let q = self.query.trim().to_lowercase();
        let ui_scale = self.settings.ui_scale;
        let width = field.width().max(320.0);
        let area = egui::Area::new(egui::Id::new("search_suggestions"))
            .order(egui::Order::Foreground)
            .fixed_pos(field.left_bottom() + Vec2::new(0.0, 3.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .rounding(14.0)
                    .inner_margin(egui::Margin::symmetric(4.0, 6.0))
                    .show(ui, |ui| {
                        ui.set_width(width);
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for (i, sg) in self.suggestions.iter().enumerate() {
                            let row_h = 28.0 * ui_scale;
                            let (rect, resp) =
                                ui.allocate_exact_size(Vec2::new(width, row_h), Sense::click());
                            if resp.hovered() {
                                self.sugg_sel = Some(i);
                            }
                            if self.sugg_sel == Some(i) {
                                ui.painter().rect_filled(
                                    rect,
                                    8.0,
                                    ui.visuals().widgets.hovered.bg_fill,
                                );
                            }
                            ui.painter().text(
                                Pos2::new(rect.left() + 14.0, rect.center().y),
                                Align2::LEFT_CENTER,
                                "🔍",
                                FontId::proportional(13.0 * ui_scale),
                                Color32::from_gray(150),
                            );
                            // Typed part in normal weight, completion in bold (like Google).
                            let font = FontId::proportional(14.0 * ui_scale);
                            let (normal, strong) =
                                (ui.visuals().text_color(), ui.visuals().strong_text_color());
                            let mut job = egui::text::LayoutJob::default();
                            job.wrap.max_width = width - 48.0;
                            job.wrap.max_rows = 1;
                            job.wrap.break_anywhere = true;
                            let lower = sg.label.to_lowercase();
                            let split = if !q.is_empty()
                                && lower.starts_with(&q)
                                && sg.label.is_char_boundary(q.len())
                            {
                                q.len()
                            } else {
                                0
                            };
                            if split > 0 {
                                job.append(
                                    &sg.label[..split],
                                    0.0,
                                    egui::TextFormat::simple(font.clone(), normal),
                                );
                            }
                            job.append(
                                &sg.label[split..],
                                0.0,
                                egui::TextFormat::simple(
                                    font,
                                    if split > 0 { strong } else { normal },
                                ),
                            );
                            let galley = ui.fonts(|f| f.layout_job(job));
                            let pos = Pos2::new(
                                rect.left() + 40.0,
                                rect.center().y - galley.size().y / 2.0,
                            );
                            ui.painter().galley(pos, galley, normal);
                            if resp.clicked() {
                                picked = Some(i);
                            }
                        }
                    });
            });
        let popup_rect = area.response.rect;
        if let Some(i) = picked {
            self.choose_suggestion(i);
            return;
        }
        // click anywhere else closes the list
        let clicked_away = ctx.input(|i| {
            i.pointer.any_pressed()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|p| !field.contains(p) && !popup_rect.contains(p))
        });
        if clicked_away {
            self.sugg_open = false;
        }
    }
}
