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
    let our_hwnd: isize = w.hwnd().ok().map(|h| h.0 as isize).unwrap_or(0);
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
        let prior = unsafe { GetForegroundWindow() };
        if !prior.0.is_null() {
            // Don't overwrite a captured prior-HWND with our own window's
            // HWND if Win+V is re-pressed while we're already foreground.
            if prior.0 as isize != our_hwnd {
                if let Some(cap) = app.try_state::<ForegroundCapture>() {
                    cap.store(prior.0 as isize);
                }
            }
        }
    }
    // Only (re)size + (re)center when bringing the window up from hidden, so a
    // window the user has dragged/resized elsewhere isn't yanked back on every
    // chord. Width is half of *whichever monitor the window currently sits on*
    // (current_monitor() reflects wherever it was last positioned, including a
    // manual drag to a second display) rather than the fixed 540px from
    // tauri.conf.json, which read as cramped on anything wider than a laptop
    // panel. Height is left alone — only width was reported as too narrow.
    let was_visible = w.is_visible().unwrap_or(false);
    if !was_visible {
        if let Ok(Some(monitor)) = w.current_monitor() {
            let mon_size = monitor.size();
            let mon_pos = monitor.position();
            let win_height = w
                .outer_size()
                .map(|s| s.height)
                .unwrap_or(620);
            let target_width = (mon_size.width / 2).max(420); // clamp to minWidth
            let _ = w.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
                target_width,
                win_height,
            )));
            let x = mon_pos.x + (mon_size.width as i32 - target_width as i32) / 2;
            let y = mon_pos.y + 120;
            let _ = w.set_position(tauri::Position::Physical(
                tauri::PhysicalPosition::new(x, y),
            ));
        }
    }
    // Always show + unminimize + focus, even when `is_visible()` already
    // reports true. A prior show that never gained the foreground — e.g.
    // Win+Shift+V landing on an empty Pinned view the user never pasted from —
    // leaves the window marked visible; the old `if !visible { show }` guard
    // then skipped the show on the next chord and a bare `set_focus()` can't
    // pull a backgrounded window forward, wedging Win+V until some other
    // window (Win+Shift+F) reset the foreground. Forcing show+focus every
    // time makes each chord deterministically surface the window.
    let _ = w.unminimize();
    let _ = w.show();
    let _ = w.set_focus();
    // `set_focus()` alone is unreliable here: unlike the RegisterHotKey-driven
    // windows (command palette, Mission Control), Win+V is served by a raw
    // WH_KEYBOARD_LL hook (see `clipboard::hotkey`) that runs outside the normal
    // window-message flow Windows uses to decide a foreground-switch request is
    // "trusted". An explicit SetForegroundWindow on our own HWND — the same call
    // `paste::simulate_paste` already uses to hand focus back afterward — makes
    // the OS-level foreground switch (and therefore keyboard input actually
    // reaching the webview, not just DOM `document.activeElement`) deterministic.
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow;
        if our_hwnd != 0 {
            unsafe {
                let _ = SetForegroundWindow(HWND(our_hwnd as *mut _));
            }
        }
    }
    let _ = app.emit(
        "clipboard-window-shown",
        serde_json::json!({ "initialFilter": initial_filter }),
    );
}
