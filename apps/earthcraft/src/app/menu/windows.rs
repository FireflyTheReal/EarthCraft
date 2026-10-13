//! Win32 menu construction and event binding through safe library APIs.
use super::Action;
use earthcraft_core::language::Lang;
use native_windows_gui as nwg;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::sync::mpsc;
pub(in crate::app) struct NativeMenu {
    // Drop items before their parent submenu. The parent HWND is borrowed, never destroyed.
    contents: FileMenu,
    handler: nwg::EventHandler,
    parent: nwg::ControlHandle,
    lang: Lang,
    screenshot_enabled: bool,
    ctx: eframe::egui::Context,
    send: mpsc::Sender<Action>,
    pub(super) events: mpsc::Receiver<Action>,
}
impl NativeMenu {
    pub(super) fn new(cc: &eframe::CreationContext<'_>, lang: Lang) -> Result<Self, String> {
        let handle = cc.window_handle().map_err(|e| e.to_string())?;
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return Err("Expected a Win32 window handle".into());
        };
        let parent = nwg::ControlHandle::Hwnd(handle.hwnd.get() as _);
        let (send, events) = mpsc::channel();
        let contents = build(parent, lang)?;
        let ctx = cc.egui_ctx.clone();
        let handler = bind(parent, &contents.items, send.clone(), ctx.clone());
        Ok(Self {
            contents,
            handler,
            parent,
            lang,
            screenshot_enabled: false,
            ctx,
            send,
            events,
        })
    }
    pub(super) fn update(&mut self, lang: Lang, has_image: bool) -> Result<(), String> {
        if lang != self.lang {
            // Build a complete replacement before releasing the current commands.
            let contents = build(self.parent, lang)?;
            nwg::unbind_event_handler(&self.handler);
            self.contents = contents;
            self.handler = bind(
                self.parent,
                &self.contents.items,
                self.send.clone(),
                self.ctx.clone(),
            );
            self.lang = lang;
            self.screenshot_enabled = false;
        }
        if has_image != self.screenshot_enabled {
            for (action, item) in &self.contents.items {
                if *action == Action::Screenshot {
                    item.set_enabled(has_image);
                }
            }
            self.screenshot_enabled = has_image;
        }
        Ok(())
    }
}
struct FileMenu {
    items: Vec<(Action, nwg::MenuItem)>,
    _separator: nwg::MenuSeparator,
    _file: nwg::Menu,
}
fn build(parent: nwg::ControlHandle, lang: Lang) -> Result<FileMenu, String> {
    let mut file = nwg::Menu::default();
    nwg::Menu::builder()
        .text(&format!("&{}", lang.text("menu-file-title")))
        .parent(parent)
        .build(&mut file)
        .map_err(|e| e.to_string())?;
    let mut items = Vec::new();
    let mut separator = nwg::MenuSeparator::default();
    for action in Action::ALL {
        if action == Action::Screenshot {
            nwg::MenuSeparator::builder()
                .parent(&file)
                .build(&mut separator)
                .map_err(|e| e.to_string())?;
        }
        let mut item = nwg::MenuItem::default();
        let shortcut = match action {
            Action::Open => "Ctrl+O",
            Action::Save => "Ctrl+S",
            Action::SaveAs => "Ctrl+Shift+S",
            Action::Screenshot => "Ctrl+Shift+E",
            Action::Quit => "Ctrl+Q",
        };
        nwg::MenuItem::builder()
            .text(&format!("{}\t{shortcut}", action.label(lang)))
            .parent(&file)
            .disabled(action == Action::Screenshot)
            .build(&mut item)
            .map_err(|e| e.to_string())?;
        items.push((action, item));
    }
    Ok(FileMenu {
        items,
        _separator: separator,
        _file: file,
    })
}
fn bind(
    parent: nwg::ControlHandle,
    items: &[(Action, nwg::MenuItem)],
    send: mpsc::Sender<Action>,
    ctx: eframe::egui::Context,
) -> nwg::EventHandler {
    let handles: Vec<_> = items
        .iter()
        .map(|(action, item)| (*action, item.handle))
        .collect();
    nwg::full_bind_event_handler(&parent, move |event, _, handle| {
        if event == nwg::Event::OnMenuItemSelected {
            if let Some((action, _)) = handles.iter().find(|(_, item)| *item == handle) {
                if send.send(*action).is_ok() {
                    ctx.request_repaint();
                }
            }
        }
    })
}
impl Drop for NativeMenu {
    fn drop(&mut self) {
        nwg::unbind_event_handler(&self.handler);
    }
}
