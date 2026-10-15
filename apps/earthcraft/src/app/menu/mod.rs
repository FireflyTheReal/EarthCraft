//! Native desktop commands; dialogs and app state are always handled by egui's thread.
use super::App;
use eframe::egui;
#[cfg(target_os = "macos")]
mod native;
#[cfg(target_os = "macos")]
pub(super) use native::NativeMenu;

mod commands;
pub(super) use commands::Action;
use commands::MenuState;
#[cfg(test)]
mod tests;
mod windows;
const GLOBAL_VIEW_ALTITUDE_KM: f64 = 14_000.0;

impl App {
    #[cfg(target_os = "macos")]
    pub(super) fn with_native_menu(mut self, cc: &eframe::CreationContext<'_>) -> Self {
        match NativeMenu::new(cc, self.lang) {
            Ok(menu) => self.native_menu = Some(menu),
            Err(error) => self.status = format!("Native menu initialization failed: {error}"),
        }
        self
    }
    #[cfg(target_os = "macos")]
    pub(super) fn refresh_native_menu(&mut self, ctx: &egui::Context) {
        let state = self.menu_state(ctx);
        if let Some(menu) = &mut self.native_menu {
            menu.update(self.lang, state);
        }
    }
    #[cfg(target_os = "macos")]
    pub(super) fn poll_native_menu(&mut self, ctx: &egui::Context) {
        self.refresh_native_menu(ctx);
        let Some(menu) = &mut self.native_menu else {
            return;
        };
        let mut actions = Vec::new();
        while let Ok(action) = menu.events.try_recv() {
            actions.push(action);
        }
        for action in actions {
            self.run_menu_action(action, ctx);
        }
    }
    #[cfg(not(target_os = "macos"))]
    pub(super) fn file_shortcuts(&mut self, ctx: &egui::Context) {
        // Keep file shortcuts available on platforms without a native menu bar.
        for action in [
            Action::SaveAs,
            Action::Screenshot,
            Action::Open,
            Action::Save,
            Action::Quit,
            Action::Settings,
            Action::Placemark,
            Action::Folder,
            Action::Sidebar,
        ] {
            if let Some(shortcut) = action.shortcut()
                && ctx.input_mut(|input| input.consume_shortcut(&shortcut))
            {
                self.run_menu_action(action, ctx);
            }
        }
    }
    pub(super) fn run_menu_action(&mut self, action: Action, ctx: &egui::Context) {
        match action {
            Action::Undo | Action::Redo => {
                let command = if action == Action::Undo {
                    super::ruler_history::HistoryCommand::Undo
                } else {
                    super::ruler_history::HistoryCommand::Redo
                };
                if ctx.egui_wants_keyboard_input() {
                    // Native accelerators consume their key events. Hand text edits
                    // the corresponding egui event so their own undo history wins.
                    ctx.input_mut(|input| {
                        input
                            .events
                            .push(super::ruler_history::text_shortcut_event(command))
                    });
                } else {
                    self.apply_ruler_history(command, ctx);
                }
            }
            Action::Open => self.pick_kml_open(),
            Action::Save => self.save_kml_to(self.resolve_kml_path(&self.kml_path)),
            Action::SaveAs => self.pick_kml_save_as(),
            Action::Screenshot if self.image.is_some() => self.save_screenshot(),
            Action::Screenshot => {}
            Action::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Action::Settings => self.show_settings = true,
            Action::Sidebar => self.sidebar_open = !self.sidebar_open,
            Action::NavigationRail => {
                self.settings.sidebar.rail_visible = !self.settings.sidebar.rail_visible
            }
            Action::ShowExplore | Action::ShowLayers | Action::ShowPlaces => {
                use earthcraft_data::settings::sidebar::SidebarPanel;
                self.settings.sidebar.panel = match action {
                    Action::ShowLayers => SidebarPanel::Layers,
                    Action::ShowPlaces => SidebarPanel::Places,
                    _ => SidebarPanel::Explore,
                };
                self.sidebar_open = true;
            }
            Action::NavigationControls => {
                self.settings.hide_navigation_controls = !self.settings.hide_navigation_controls
            }
            Action::Grid => self.show_grid = !self.show_grid,
            Action::Atmosphere => self.show_atmo = !self.show_atmo,
            Action::Sunlight => self.show_sun = !self.show_sun,
            Action::North => self.cam.heading = 0.0,
            Action::GlobalView => self.fly_to(self.cam.lat, self.cam.lon, GLOBAL_VIEW_ALTITUDE_KM),
            Action::Placemark => {
                self.add_center_placemark();
                self.reveal_places();
            }
            Action::Folder => {
                self.add_places_folder();
                self.reveal_places();
            }
            Action::Ruler => {
                self.ruler = !self.ruler;
                self.reveal_ruler();
            }
            Action::Explorer => self.explorer.open = !self.explorer.open,
            Action::Fullscreen => ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(
                !self.menu_state(ctx).fullscreen,
            )),
            Action::Minimize => ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true)),
            Action::About => self.show_about = true,
            Action::Help => self.show_shortcuts = true,
            Action::Documentation => ctx.open_url(egui::OpenUrl::new_tab(
                "https://github.com/therichronicles/EarthCraft#readme",
            )),
            Action::ReportIssue => ctx.open_url(egui::OpenUrl::new_tab(
                "https://github.com/therichronicles/EarthCraft/issues/new/choose",
            )),
        }
    }
}

impl App {
    fn menu_state(&self, ctx: &egui::Context) -> MenuState {
        MenuState {
            image: self.image.is_some(),
            undo: ctx.egui_wants_keyboard_input() || self.ruler_paths.can_undo(),
            redo: ctx.egui_wants_keyboard_input() || self.ruler_paths.can_redo(),
            sidebar: self.sidebar_open,
            rail: self.settings.sidebar.rail_visible,
            navigation: !self.settings.hide_navigation_controls,
            grid: self.show_grid,
            atmosphere: self.show_atmo,
            sunlight: self.show_sun,
            ruler: self.ruler,
            explorer: self.explorer.open,
            fullscreen: ctx.input(|input| input.viewport().fullscreen.unwrap_or(false)),
        }
    }
    pub(super) fn reveal_places(&mut self) {
        self.sidebar_open = true;
        self.place_reveal_requested = true;
        self.ruler_reveal_requested = false;
    }
    pub(super) fn reveal_ruler(&mut self) {
        self.ruler_reveal_requested = self.ruler;
        if self.ruler {
            self.sidebar_open = true;
            self.place_reveal_requested = false;
        }
    }
    #[cfg(any(not(target_os = "macos"), test))]
    pub(super) fn in_window_menu(&mut self, ui: &mut egui::Ui) {
        let state = self.menu_state(ui);
        if let Some(action) = commands::draw_menu(ui, self.lang, state) {
            self.run_menu_action(action, ui);
        }
    }
}
