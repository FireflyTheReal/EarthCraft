//! Native desktop commands; dialogs and app state are always handled by egui's thread.
use super::App;
#[cfg(target_os = "macos")]
use earthcraft_core::language::Lang;
use eframe::egui;
#[cfg(target_os = "macos")]
mod native;
#[cfg(target_os = "macos")]
pub(super) use native::NativeMenu;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    Open,
    Save,
    SaveAs,
    Screenshot,
    Quit,
    Undo,
    Redo,
}
impl Action {
    #[cfg(target_os = "macos")]
    pub(super) const ALL: [Self; 7] = [
        Self::Open,
        Self::Save,
        Self::SaveAs,
        Self::Screenshot,
        Self::Undo,
        Self::Redo,
        Self::Quit,
    ];
    #[cfg(target_os = "macos")]
    pub(super) fn label(self, lang: Lang) -> String {
        lang.text(match self {
            Self::Open => "menu-kml-open",
            Self::Save => "menu-kml-save",
            Self::SaveAs => "menu-kml-save-as",
            Self::Screenshot => "menu-screenshot-save",
            Self::Quit => "menu-quit-action",
            Self::Undo => "edit-undo-action",
            Self::Redo => "edit-redo-action",
        })
        .into()
    }
    #[cfg(not(target_os = "macos"))]
    pub(super) fn shortcut(self) -> egui::KeyboardShortcut {
        use egui::{Key, Modifiers};
        let key = match self {
            Self::Open => Key::O,
            Self::Save | Self::SaveAs => Key::S,
            Self::Screenshot => Key::E,
            Self::Quit => Key::Q,
            Self::Undo | Self::Redo => Key::Z,
        };
        let modifiers = if matches!(self, Self::SaveAs | Self::Screenshot | Self::Redo) {
            Modifiers::COMMAND | Modifiers::SHIFT
        } else {
            Modifiers::COMMAND
        };
        egui::KeyboardShortcut::new(modifiers, key)
    }
}
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
        if let Some(menu) = &mut self.native_menu {
            menu.update(
                self.lang,
                self.image.is_some(),
                ctx.wants_keyboard_input() || self.ruler_paths.can_undo(),
                ctx.wants_keyboard_input() || self.ruler_paths.can_redo(),
            );
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
        ] {
            if ctx.input_mut(|input| input.consume_shortcut(&action.shortcut())) {
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
                if ctx.wants_keyboard_input() {
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
        }
    }
}
