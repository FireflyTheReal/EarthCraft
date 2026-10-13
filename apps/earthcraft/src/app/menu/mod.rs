//! Native desktop commands; dialogs and app state are always handled by egui's thread.
use super::App;
use earthcraft_core::language::Lang;
use eframe::egui;
mod native;
pub(super) use native::NativeMenu;
#[cfg(target_os = "windows")]
mod windows;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    Open,
    Save,
    SaveAs,
    Screenshot,
    Quit,
}
impl Action {
    pub(super) const ALL: [Self; 5] = [
        Self::Open,
        Self::Save,
        Self::SaveAs,
        Self::Screenshot,
        Self::Quit,
    ];
    pub(super) fn label(self, lang: Lang) -> String {
        lang.text(match self {
            Self::Open => "menu-kml-open",
            Self::Save => "menu-kml-save",
            Self::SaveAs => "menu-kml-save-as",
            Self::Screenshot => "menu-screenshot-save",
            Self::Quit => "menu-quit-action",
        })
        .into()
    }
    #[cfg(target_os = "windows")]
    pub(super) fn shortcut(self) -> egui::KeyboardShortcut {
        use egui::{Key, Modifiers};
        let key = match self {
            Self::Open => Key::O,
            Self::Save | Self::SaveAs => Key::S,
            Self::Screenshot => Key::E,
            Self::Quit => Key::Q,
        };
        let modifiers = if matches!(self, Self::SaveAs | Self::Screenshot) {
            Modifiers::COMMAND | Modifiers::SHIFT
        } else {
            Modifiers::COMMAND
        };
        egui::KeyboardShortcut::new(modifiers, key)
    }
}
impl App {
    pub(super) fn with_native_menu(mut self, cc: &eframe::CreationContext<'_>) -> Self {
        match NativeMenu::new(cc, self.lang) {
            Ok(menu) => self.native_menu = Some(menu),
            Err(error) => self.status = format!("Native File menu initialization failed: {error}"),
        }
        self
    }
    pub(super) fn poll_native_menu(&mut self, ctx: &egui::Context) {
        let Some(menu) = &mut self.native_menu else {
            return;
        };
        menu.update(self.lang, self.image.is_some());
        let mut actions = Vec::new();
        while let Ok(action) = menu.events.try_recv() {
            actions.push(action);
        }
        // Windows' winit loop does not translate native menu accelerators. Consume
        // shortcuts here; macOS dispatches its registered accelerators via AppKit.
        #[cfg(target_os = "windows")]
        for action in [
            Action::SaveAs,
            Action::Screenshot,
            Action::Open,
            Action::Save,
            Action::Quit,
        ] {
            if ctx.input_mut(|input| input.consume_shortcut(&action.shortcut())) {
                actions.push(action);
            }
        }
        for action in actions {
            match action {
                Action::Open => self.pick_kml_open(),
                Action::Save => self.save_kml_to(self.resolve_kml_path(&self.kml_path)),
                Action::SaveAs => self.pick_kml_save_as(),
                Action::Screenshot if self.image.is_some() => self.save_screenshot(),
                Action::Screenshot => {}
                Action::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            }
        }
    }
}
