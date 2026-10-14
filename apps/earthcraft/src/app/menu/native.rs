//! Native menu ownership stays on the event-loop thread for its full lifetime.
use super::Action;
use earthcraft_core::language::Lang;
use muda::{
    accelerator::{Accelerator, Code, Modifiers},
    Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu,
};
use std::sync::mpsc;
pub(in crate::app) struct NativeMenu {
    _menu: Menu,
    file: Submenu,
    edit: Submenu,
    history_enabled: (bool, bool),
    items: Vec<(Action, MenuItem)>,
    lang: Lang,
    screenshot_enabled: bool,
    pub(super) events: mpsc::Receiver<Action>,
}
impl NativeMenu {
    pub(super) fn new(cc: &eframe::CreationContext<'_>, lang: Lang) -> Result<Self, String> {
        let menu = Menu::new();
        let file = Submenu::new(lang.text("menu-file-title"), true);
        let edit = Submenu::new(lang.text("menu-edit-title"), true);
        let mut items = Vec::new();
        for action in Action::ALL {
            let key = match action {
                Action::Open => Code::KeyO,
                Action::Save | Action::SaveAs => Code::KeyS,
                Action::Screenshot => Code::KeyE,
                Action::Quit => Code::KeyQ,
                Action::Undo | Action::Redo => Code::KeyZ,
            };
            let command = Modifiers::META;
            let modifiers = if matches!(action, Action::SaveAs | Action::Screenshot | Action::Redo)
            {
                command | Modifiers::SHIFT
            } else {
                command
            };
            let item = MenuItem::new(
                action.label(lang),
                !matches!(action, Action::Screenshot | Action::Undo | Action::Redo),
                Some(Accelerator::new(modifiers, key)),
            );
            if matches!(action, Action::Undo | Action::Redo) {
                edit.append(&item).map_err(|e| e.to_string())?;
            } else if action != Action::Quit {
                if action == Action::Screenshot {
                    file.append(&PredefinedMenuItem::separator())
                        .map_err(|e| e.to_string())?;
                }
                file.append(&item).map_err(|e| e.to_string())?;
            }
            items.push((action, item));
        }
        {
            let app = Submenu::new("EarthCraft", true);
            app.append_items(&[
                &PredefinedMenuItem::hide(None),
                &PredefinedMenuItem::hide_others(None),
                &PredefinedMenuItem::show_all(None),
                &PredefinedMenuItem::separator(),
                &items
                    .iter()
                    .find(|(action, _)| *action == Action::Quit)
                    .unwrap()
                    .1,
            ])
            .map_err(|e| e.to_string())?;
            menu.append(&app).map_err(|e| e.to_string())?;
        }
        menu.append(&file).map_err(|e| e.to_string())?;
        menu.append(&edit).map_err(|e| e.to_string())?;
        menu.init_for_nsapp();
        let ids: Vec<_> = items
            .iter()
            .map(|(action, item)| (item.id().clone(), *action))
            .collect();
        let (send, events) = mpsc::channel();
        let ctx = cc.egui_ctx.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if let Some((_, action)) = ids.iter().find(|(id, _)| *id == event.id) {
                if send.send(*action).is_ok() {
                    ctx.request_repaint();
                }
            }
        }));
        Ok(Self {
            _menu: menu,
            file,
            edit,
            history_enabled: (false, false),
            items,
            lang,
            screenshot_enabled: false,
            events,
        })
    }
    pub(super) fn update(&mut self, lang: Lang, has_image: bool, can_undo: bool, can_redo: bool) {
        if lang != self.lang {
            self.file.set_text(lang.text("menu-file-title"));
            self.edit.set_text(lang.text("menu-edit-title"));
            for (action, item) in &self.items {
                item.set_text(action.label(lang));
            }
            self.lang = lang;
        }
        if self.history_enabled != (can_undo, can_redo) {
            for (action, item) in &self.items {
                match action {
                    Action::Undo => item.set_enabled(can_undo),
                    Action::Redo => item.set_enabled(can_redo),
                    _ => {}
                }
            }
            self.history_enabled = (can_undo, can_redo);
        }
        if has_image != self.screenshot_enabled {
            for (action, item) in &self.items {
                if *action == Action::Screenshot {
                    item.set_enabled(has_image);
                }
            }
            self.screenshot_enabled = has_image;
        }
    }
}
impl Drop for NativeMenu {
    fn drop(&mut self) {
        MenuEvent::set_event_handler(None::<fn(MenuEvent)>);
        self._menu.remove_for_nsapp();
    }
}
