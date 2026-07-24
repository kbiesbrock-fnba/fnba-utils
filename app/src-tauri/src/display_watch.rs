//! Detects display-topology changes — dock / undock, monitor add/remove, DPI
//! change, taskbar move/resize — and drives the app's recovery so the user
//! doesn't have to restart after re-docking.
//!
//! Windows broadcasts `WM_DISPLAYCHANGE` and `WM_SETTINGCHANGE(SPI_SETWORKAREA)`
//! to every top-level window, so we observe them by subclassing an existing,
//! always-alive app window (the docker-widget) via the comctl32
//! `SetWindowSubclass` API. Subclassing chains ahead of Tauri's own window
//! procedure without replacing it (unlike overwriting `GWLP_WNDPROC`), which is
//! the safe way to watch messages on a window whose wndproc we don't own.
//!
//! Dock transitions fire these messages in bursts, so the reaction is debounced
//! to run once ~700ms after the last message — off the UI thread, since the
//! wndproc itself must return fast.

#[cfg(windows)]
use tauri::{AppHandle, Emitter};

/// Install the display-topology watcher by subclassing the always-alive
/// docker-widget window. Best-effort: logs and returns on any failure so a
/// managed-machine quirk can't take down app setup.
#[cfg(windows)]
pub fn install(app: AppHandle) {
    win::install(app);
}

#[cfg(not(windows))]
pub fn install(_app: tauri::AppHandle) {}

/// Recovery run once per settled display change. Idempotent.
#[cfg(windows)]
fn react(app: &AppHandle) {
    // Re-pin the docker widget: its position was computed once at startup
    // against the old monitor layout / work-area rect.
    crate::position_docker_widget(app);

    // Let the frontend re-fetch anything it cached against the old layout
    // (e.g. the docker widget's taskbar anchor).
    let _ = app.emit("display-changed", ());
}

#[cfg(windows)]
mod win {
    use super::react;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::OnceLock;
    use tauri::{AppHandle, Manager};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
    use windows::Win32::UI::WindowsAndMessaging::{
        SPI_SETWORKAREA, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_SETTINGCHANGE,
    };

    static APP: OnceLock<AppHandle> = OnceLock::new();

    // Bumped on every relevant message; a scheduled reaction only fires if its
    // captured value is still current after the debounce window (i.e. no newer
    // message arrived). This collapses a dock-transition burst into one recovery.
    static GENERATION: AtomicU64 = AtomicU64::new(0);

    // Arbitrary, stable subclass id (only needs to be unique per HWND).
    const SUBCLASS_ID: usize = 0xFDBA;

    const DEBOUNCE_MS: u64 = 700;

    pub fn install(app: AppHandle) {
        let Some(window) = app.get_webview_window("docker-widget") else {
            eprintln!("display_watch: docker-widget window missing; watcher not installed");
            return;
        };
        // Tauri's HWND comes from a different `windows` crate version than the
        // one we link against, so round-trip through the raw pointer (same as
        // widget_focus.rs).
        let raw = match window.hwnd() {
            Ok(h) => h.0 as isize,
            Err(e) => {
                eprintln!("display_watch: hwnd() failed: {e}; watcher not installed");
                return;
            }
        };
        if APP.set(app).is_err() {
            return; // already installed
        }
        let hwnd = HWND(raw as *mut core::ffi::c_void);
        let ok = unsafe { SetWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID, 0) };
        if !ok.as_bool() {
            eprintln!("display_watch: SetWindowSubclass failed");
        }
    }

    unsafe extern "system" fn subclass_proc(
        hwnd: HWND,
        umsg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _uidsubclass: usize,
        _dwrefdata: usize,
    ) -> LRESULT {
        let relevant = match umsg {
            WM_DISPLAYCHANGE | WM_DPICHANGED => true,
            // wParam carries the SPI action; we only care about work-area moves
            // (taskbar relocated/resized), not every settings broadcast.
            WM_SETTINGCHANGE => wparam.0 as u32 == SPI_SETWORKAREA.0,
            _ => false,
        };
        if relevant {
            schedule_reaction();
        }
        DefSubclassProc(hwnd, umsg, wparam, lparam)
    }

    fn schedule_reaction() {
        let my_gen = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
        // Plain OS thread (not the tauri runtime): the reaction only calls
        // thread-safe Tauri window methods, and this keeps the wndproc's cost to
        // an atomic bump plus a spawn.
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(DEBOUNCE_MS));
            if GENERATION.load(Ordering::SeqCst) != my_gen {
                return; // superseded by a later message in the same burst
            }
            if let Some(app) = APP.get() {
                react(app);
            }
        });
    }
}
