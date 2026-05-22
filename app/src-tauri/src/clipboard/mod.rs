//! Clipboard capture pipeline.
//!
//! - `listener` registers a hidden message-only window that receives
//!   `WM_CLIPBOARDUPDATE` events from the OS and reads the clipboard contents
//!   into a `NewClipboardEntry`.
//! - `paste` handles re-emitting an entry back to the OS clipboard and
//!   simulating Ctrl+V into the previously-focused window.
//! - `ForegroundCapture` is a tiny `Mutex<Option<isize>>` Tauri state holding
//!   the HWND that was foregrounded right before we showed our window, so
//!   `paste_clipboard_entry` can return focus to it.

pub mod daemon;
pub mod hotkey;
pub mod listener;
#[cfg(windows)]
pub mod paste;
pub mod pii;

use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Default)]
pub struct ForegroundCapture {
    pub hwnd: Mutex<Option<isize>>,
}

impl ForegroundCapture {
    pub fn store(&self, hwnd: isize) {
        if let Ok(mut g) = self.hwnd.lock() {
            *g = Some(hwnd);
        }
    }

    pub fn take(&self) -> Option<isize> {
        self.hwnd.lock().ok().and_then(|mut g| g.take())
    }
}

/// Always-show entry point for the clipboard-manager window.
///
/// Hotkey semantics (Win+V) are deliberately *show + focus*, never toggle:
/// pressing the chord while the window is already visible should still
/// reaffirm focus and put the caret back in the search field. Hiding is the
/// job of the window's own Esc / close-button handlers.
///
/// Captures the prior foreground HWND first so the paste path can return
/// focus + synthesize Ctrl+V into whichever app the user was typing in.
///
/// `initial_filter` is forwarded to the frontend via the emitted
/// `clipboard-window-shown` event payload. `None` keeps the existing
/// behavior (reset to "all"); `Some("pinned")` is used by Win+Shift+V to
/// land directly on the pinned filter.
pub fn show_clipboard_window(app: &AppHandle, initial_filter: Option<&str>) {
    let Some(w) = app.get_webview_window("clipboard-manager") else {
        return;
    };
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
        let prior = unsafe { GetForegroundWindow() };
        if !prior.0.is_null() {
            // Don't overwrite a captured prior-HWND with our own window's
            // HWND if Win+V is re-pressed while we're already foreground.
            let our_hwnd = w
                .hwnd()
                .ok()
                .map(|h| h.0 as isize)
                .unwrap_or(0);
            if prior.0 as isize != our_hwnd {
                if let Some(cap) = app.try_state::<ForegroundCapture>() {
                    cap.store(prior.0 as isize);
                }
            }
        }
    }
    if !w.is_visible().unwrap_or(false) {
        if let Ok(Some(monitor)) = w.current_monitor() {
            let mon_size = monitor.size();
            let mon_pos = monitor.position();
            let win_size = w
                .outer_size()
                .unwrap_or(tauri::PhysicalSize::new(520, 600));
            let x = mon_pos.x + (mon_size.width as i32 - win_size.width as i32) / 2;
            let y = mon_pos.y + 120;
            let _ = w.set_position(tauri::Position::Physical(
                tauri::PhysicalPosition::new(x, y),
            ));
        }
        let _ = w.show();
    }
    let _ = w.set_focus();
    let _ = app.emit(
        "clipboard-window-shown",
        serde_json::json!({ "initialFilter": initial_filter }),
    );
}
