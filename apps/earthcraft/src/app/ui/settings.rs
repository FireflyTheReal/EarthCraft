//! Searchable settings shell; category widgets are separate from persistence.
use super::super::App;
use earthcraft_core::language::{Lang, fold_name};
use earthcraft_data::settings::Settings;
use eframe::egui;
mod sections;
pub(super) const MAX_UI_SLIDER_SCALE: f32 = 2.0;
const WINDOW_SIZE_POINTS: egui::Vec2 = egui::vec2(680.0, 560.0);
const WINDOW_MIN_SIZE_POINTS: egui::Vec2 = egui::vec2(520.0, 360.0);
const MIN_BODY_HEIGHT_POINTS: f32 = 160.0;
const FOOTER_HEIGHT_POINTS: f32 = 40.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Category {
    #[default]
    General,
    Appearance,
    Navigation,
    Globe,
    Graphics,
    Cache,
    Files,
}
impl Category {
    const ALL: [Self; 7] = [
        Self::General,
        Self::Appearance,
        Self::Navigation,
        Self::Globe,
        Self::Graphics,
        Self::Cache,
        Self::Files,
    ];
    fn name_id(self) -> &'static str {
        match self {
            Self::General => "settings-category-general",
            Self::Appearance => "settings-category-appearance",
            Self::Navigation => "settings-category-navigation",
            Self::Globe => "settings-category-globe",
            Self::Graphics => "settings-category-graphics",
            Self::Cache => "settings-category-cache",
            Self::Files => "settings-category-files",
        }
    }
    fn keywords(self) -> &'static [&'static str] {
        match self {
            Self::General => &["settings-language-label"],
            Self::Appearance => &[
                "settings-theme-label",
                "settings-ui-font-label",
                "settings-map-font-label",
                "settings-distance-units-label",
                "settings-ui-scale-label",
                "settings-map-scale-label",
                "coordinates-format-label",
                "coordinates-format-dd",
                "coordinates-format-dms",
                "coordinates-format-ddm",
                "coordinates-format-utm",
                "coordinates-format-mgrs",
                "coordinates-input-help",
            ],
            Self::Navigation => &[
                "settings-hide-navigation-controls-toggle",
                "settings-pan-sensitivity",
                "settings-rotation-sensitivity",
                "settings-zoom-sensitivity",
                "settings-invert-scroll",
                "settings-startup-view-label",
                "settings-startup-help",
            ],
            Self::Globe => &[
                "settings-earth-model-label",
                "earth-model-wgs84",
                "earth-model-sphere",
            ],
            Self::Graphics => &["settings-uncap-fps-toggle", "settings-frame-sync-help"],
            Self::Cache => &[
                "settings-cache-section-title",
                "settings-cache-clear-help",
                "settings-cache-decoded-label",
                "settings-cache-disk-label",
                "settings-cache-restart-help",
            ],
            Self::Files => &[
                "settings-screenshot-folder-label",
                "settings-kml-folder-label",
                "settings-folder-creation-help",
            ],
        }
    }
    fn matches(self, query: &str, lang: Lang) -> bool {
        query.is_empty()
            || std::iter::once(self.name_id())
                .chain(self.keywords().iter().copied())
                .any(|id| fold_name(lang.text(id)).contains(query))
    }
}
#[derive(Default)]
pub(in crate::app) struct SettingsUi {
    category: Category,
    query: String,
    confirm_reset: bool,
    confirm_cache: bool,
}
impl App {
    pub(in crate::app) fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }
        self.refresh_disk_usage(ctx);
        let lang = self.lang;
        let previous_model = self.settings.earth_model;
        let mut open = self.show_settings;
        let mut close_requested = false;
        let mut language_preference = self.language_preference;
        let mut state = std::mem::take(&mut self.settings_ui);
        egui::Window::new(lang.text("settings-title"))
            .id(egui::Id::new("settings-window"))
            .open(&mut open)
            .resizable(true)
            .default_size(WINDOW_SIZE_POINTS)
            .min_size(WINDOW_MIN_SIZE_POINTS)
            .show(ctx, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut state.query)
                        .hint_text(lang.text("settings-search-placeholder"))
                        .desired_width(f32::INFINITY),
                );
                ui.horizontal_wrapped(|ui| {
                    for category in Category::ALL {
                        ui.selectable_value(
                            &mut state.category,
                            category,
                            lang.text(category.name_id()),
                        );
                    }
                });
                ui.separator();
                let query = fold_name(&state.query);
                let categories: Vec<_> = Category::ALL
                    .into_iter()
                    .filter(|category| {
                        if query.is_empty() {
                            *category == state.category
                        } else {
                            category.matches(&query, lang)
                        }
                    })
                    .collect();
                let height =
                    (ui.available_height() - FOOTER_HEIGHT_POINTS).max(MIN_BODY_HEIGHT_POINTS);
                egui::ScrollArea::vertical()
                    .id_salt("settings-body")
                    .max_height(height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if categories.is_empty() {
                            ui.label(lang.format(
                                "settings-search-empty",
                                &[("query", state.query.as_str().into())],
                            ));
                        }
                        for category in categories {
                            ui.push_id(category as u8, |ui| {
                                ui.heading(lang.text(category.name_id()));
                                match category {
                                    Category::General => {
                                        self.settings_general(ui, &mut language_preference)
                                    }
                                    Category::Appearance => self.settings_appearance(ui),
                                    Category::Navigation => self.settings_navigation(ui),
                                    Category::Globe => self.settings_globe(ui),
                                    Category::Graphics => self.settings_graphics(ui),
                                    Category::Cache => {
                                        self.settings_cache(ui, &mut state.confirm_cache)
                                    }
                                    Category::Files => self.settings_files(ui),
                                }
                                ui.add_space(ui.spacing().item_spacing.y);
                            });
                        }
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(lang.text("settings-reset-action")).clicked() {
                        state.confirm_reset = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(lang.text("settings-close-action")).clicked() {
                            close_requested = true;
                        }
                    });
                });
                if state.confirm_reset {
                    ui.label(lang.text("settings-reset-confirm"));
                    ui.horizontal(|ui| {
                        if ui.button(lang.text("settings-confirm-action")).clicked() {
                            self.settings = Settings::default();
                            self.ui_scale_edit = self.settings.ui_scale;
                            self.sidebar_open = self.settings.sidebar.open;
                            state.confirm_reset = false;
                        }
                        if ui.button(lang.text("settings-cancel-action")).clicked() {
                            state.confirm_reset = false;
                        }
                    });
                }
            });
        if language_preference != self.language_preference {
            self.set_language_preference(language_preference);
            ctx.request_repaint();
        }
        if previous_model != self.settings.earth_model
            && self.explorer.active == earthcraft_core::body::Body::Earth
        {
            self.fly = None;
            self.ruler_paths.end_move();
            self.ruler_drag = None;
        }
        self.settings_ui = state;
        self.show_settings = open && !close_requested;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_matches_localized_labels_and_help() {
        assert!(Category::Appearance.matches(&fold_name("MGRS"), Lang::En));
        assert!(Category::Appearance.matches(&fold_name("distância"), Lang::PtBr));
        assert!(Category::Navigation.matches(&fold_name("sensibilidade"), Lang::PtBr));
        assert!(!Category::Files.matches("mgrs", Lang::En));
        for lang in [Lang::En, Lang::PtBr] {
            for category in Category::ALL {
                assert!(category.matches(&fold_name(lang.text(category.name_id())), lang));
            }
        }
    }
}
