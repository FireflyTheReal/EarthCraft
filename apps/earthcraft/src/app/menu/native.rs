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
    items: Vec<(Action, MenuItem)>,
    lang: Lang,
    screenshot_enabled: bool,
    pub(super) events: mpsc::Receiver<Action>,
}
impl NativeMenu {
    pub(super) fn new(cc: &eframe::CreationContext<'_>, lang: Lang) -> Result<Self, String> {
        let menu = Menu::new();
        let file = Submenu::new(lang.text("menu-file-title"), true);
        let mut items = Vec::new();
        for action in Action::ALL {
            let key = match action {
                Action::Open => Code::KeyO,
                Action::Save | Action::SaveAs => Code::KeyS,
                Action::Screenshot => Code::KeyE,
                Action::Quit => Code::KeyQ,
            };
            let command = Modifiers::SUPER;
            let modifiers = if matches!(action, Action::SaveAs | Action::Screenshot) {
                command | Modifiers::SHIFT
            } else {
                command
            };
            let item = MenuItem::new(
                action.label(lang),
                action != Action::Screenshot,
                Some(Accelerator::new(Some(modifiers), key)),
            );
            if action != Action::Quit {
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
                &items.last().unwrap().1,
            ])
            .map_err(|e| e.to_string())?;
            menu.append(&app).map_err(|e| e.to_string())?;
        }
        menu.append(&file).map_err(|e| e.to_string())?;
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
            items,
            lang,
            screenshot_enabled: false,
            events,
        })
    }
    pub(super) fn update(&mut self, lang: Lang, has_image: bool) {
        if lang != self.lang {
            self.file.set_text(lang.text("menu-file-title"));
            for (action, item) in &self.items {
                item.set_text(action.label(lang));
            }
            self.lang = lang;
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
