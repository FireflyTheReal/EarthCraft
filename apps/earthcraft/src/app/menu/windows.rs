//! The only workspace FFI exception: attach a menu to eframe's live Win32 window.
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

#[allow(unsafe_code)]
pub(super) fn attach(menu: &muda::Menu, cc: &eframe::CreationContext<'_>) -> Result<(), String> {
    let handle = cc.window_handle().map_err(|error| error.to_string())?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return Err("Expected a Win32 window handle".into());
    };
    // SAFETY: CreationContext borrows eframe's already-created, live native window.
    // This call runs synchronously on that window's event-loop thread. The owning
    // NativeMenu is !Send and retained by App; muda owns subclass teardown in Drop.
    // No HWND is cached or used by EarthCraft after this initialization call.
    unsafe { menu.init_for_hwnd(handle.hwnd.get()) }.map_err(|error| error.to_string())
}
