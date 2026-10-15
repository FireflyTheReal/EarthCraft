//! Menu objects stay on the event-loop thread; callbacks only enqueue commands.
use super::{
    Action,
    commands::{MenuState, Section},
};
use earthcraft_core::language::Lang;
use muda::{
    CheckMenuItem, IsMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu,
    accelerator::{Accelerator, Code, Modifiers},
};
use std::sync::mpsc;

enum Item {
    Plain(MenuItem),
    Toggle(CheckMenuItem),
}
impl Item {
    fn menu_item(&self) -> &dyn IsMenuItem {
        match self {
            Self::Plain(item) => item,
            Self::Toggle(item) => item,
        }
    }
    fn id(&self) -> &MenuId {
        self.menu_item().id()
    }
    fn update(&self, label: &str, enabled: bool, checked: Option<bool>) {
        match self {
            Self::Plain(item) => {
                item.set_text(label);
                item.set_enabled(enabled);
            }
            Self::Toggle(item) => {
                item.set_text(label);
                item.set_enabled(enabled);
                if let Some(checked) = checked {
                    item.set_checked(checked);
                }
            }
        }
    }
}

pub(in crate::app) struct NativeMenu {
    menu: Menu,
    sections: Vec<(Section, Submenu)>,
    items: Vec<(Action, Item)>,
    lang: Lang,
    state: MenuState,
    pub(super) events: mpsc::Receiver<Action>,
}
impl NativeMenu {
    pub(super) fn new(cc: &eframe::CreationContext<'_>, lang: Lang) -> Result<Self, String> {
        let menu = Menu::new();
        let state = MenuState::default();
        let mut items = Vec::new();
        let mut sections = Vec::new();
        for section in Section::ALL {
            let submenu = Submenu::new(section.label(lang), true);
            for action in Action::ALL
                .into_iter()
                .filter(|action| action.section() == section)
            {
                if action == Action::Quit {
                    submenu
                        .append_items(&[
                            &PredefinedMenuItem::separator(),
                            &PredefinedMenuItem::hide(None),
                            &PredefinedMenuItem::hide_others(None),
                            &PredefinedMenuItem::show_all(None),
                            &PredefinedMenuItem::separator(),
                        ])
                        .map_err(|error| error.to_string())?;
                }
                let accelerator = accelerator(action);
                let item = if let Some(checked) = state.checked(action) {
                    Item::Toggle(CheckMenuItem::new(
                        action.label(lang),
                        state.enabled(action),
                        checked,
                        accelerator,
                    ))
                } else {
                    Item::Plain(MenuItem::new(
                        action.label(lang),
                        state.enabled(action),
                        accelerator,
                    ))
                };
                submenu
                    .append(item.menu_item())
                    .map_err(|error| error.to_string())?;
                items.push((action, item));
            }
            menu.append(&submenu).map_err(|error| error.to_string())?;
            if section == Section::Window {
                submenu.set_as_windows_menu_for_nsapp();
            }
            if section == Section::Help {
                submenu.set_as_help_menu_for_nsapp();
            }
            sections.push((section, submenu));
        }
        menu.init_for_nsapp();
        let ids: Vec<_> = items
            .iter()
            .map(|(action, item)| (item.id().clone(), *action))
            .collect();
        let (send, events) = mpsc::channel();
        let ctx = cc.egui_ctx.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if let Some((_, action)) = ids.iter().find(|(id, _)| *id == event.id)
                && send.send(*action).is_ok()
            {
                ctx.request_repaint();
            }
        }));
        Ok(Self {
            menu,
            sections,
            items,
            lang,
            state,
            events,
        })
    }
    pub(super) fn update(&mut self, lang: Lang, state: MenuState) {
        if lang != self.lang {
            for (section, submenu) in &self.sections {
                submenu.set_text(section.label(lang));
            }
        }
        if lang != self.lang || state != self.state {
            for (action, item) in &self.items {
                item.update(
                    action.label(lang),
                    state.enabled(*action),
                    state.checked(*action),
                );
            }
            self.lang = lang;
            self.state = state;
        }
    }
}
fn accelerator(action: Action) -> Option<Accelerator> {
    let shortcut = action.shortcut()?;
    let key = match shortcut.logical_key {
        eframe::egui::Key::O => Code::KeyO,
        eframe::egui::Key::S => Code::KeyS,
        eframe::egui::Key::E => Code::KeyE,
        eframe::egui::Key::Q => Code::KeyQ,
        eframe::egui::Key::Z => Code::KeyZ,
        eframe::egui::Key::Comma => Code::Comma,
        eframe::egui::Key::P => Code::KeyP,
        eframe::egui::Key::N => Code::KeyN,
        eframe::egui::Key::B => Code::KeyB,
        _ => return None,
    };
    let mut modifiers = Modifiers::META;
    if shortcut.modifiers.shift {
        modifiers |= Modifiers::SHIFT;
    }
    if shortcut.modifiers.alt {
        modifiers |= Modifiers::ALT;
    }
    Some(Accelerator::new(modifiers, key))
}
impl Drop for NativeMenu {
    fn drop(&mut self) {
        MenuEvent::set_event_handler(None::<fn(MenuEvent)>);
        self.menu.remove_for_nsapp();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_catalog_shortcut_has_a_native_accelerator() {
        for action in Action::ALL {
            assert_eq!(
                accelerator(action).is_some(),
                action.shortcut().is_some(),
                "{action:?}"
            );
        }
    }
}
