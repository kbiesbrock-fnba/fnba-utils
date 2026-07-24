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
use tauri::{AppHandle, Emitter, Manager};

use std::sync::atomic::{AtomicBool, Ordering};

/// Set true by the debounced display-change handler and consumed (swapped back
/// to false) by the palette show-path, which then kicks its own surface once.
/// The palette is hidden during a dock transition, so it isn't in the
/// visible-window sweep and needs the kick on its next show instead.
static DISPLAY_CHANGED_SINCE_SHOW: AtomicBool = AtomicBool::new(false);

/// Consume the "a display change happened since the last palette show" flag.
pub fn take_display_changed_flag() -> bool {
    DISPLAY_CHANGED_SINCE_SHOW.swap(false, Ordering::SeqCst)
}

/// Nudge a window's outer size by 1px and restore it, forcing WebView2 to
/// recomposite. Clears the surface stall that can leave a window painting its
/// last pre-topology-change frame (e.g. the palette backdrop with no card)
/// after a dock/undock.
pub fn kick_window(window: &tauri::WebviewWindow) {
    if let Ok(size) = window.outer_size() {
        let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
            size.width + 1,
            size.height + 1,
        )));
        let _ = window.set_size(tauri::Size::Physical(size));
    }
}

/// Install the display-topology watcher by subclassing the always-alive
/// docker-widget window. Best-effort: logs and returns on any failure so a
/// managed-machine quirk can't take down app setup.
#[cfg(windows)]
pub fn install(app: AppHandle) {
    win::install(app);
}

#[cfg(not(windows))]
pub fn install(_app: tauri::AppHandle) {}

/// First recovery reaction after a settled display-change burst. Returns
/// whether the widget reposition validated (landed on a live monitor); the
/// caller uses `false` to decide whether to start the retry chain. Idempotent.
#[cfg(windows)]
fn react(app: &AppHandle) -> bool {
    // Re-pin the docker widget: its position was computed once at startup
    // against the old monitor layout / work-area rect. May fail if Windows is
    // still reporting stale monitor/work-area data (retried by the caller).
    let positioned = crate::position_docker_widget(app);

    // Let the frontend re-fetch anything it cached against the old layout
    // (e.g. the docker widget's taskbar anchor). The frontend keeps its
    // last-known-good anchor if this fires before the work area has settled.
    let _ = app.emit("display-changed", ());

    // Kick every currently-visible window's WebView2 surface out of a possible
    // compositor stall. Hidden windows (e.g. the closed palette) are handled on
    // their next show via take_display_changed_flag(). Kicks run once, on this
    // first reaction — the retry chain only re-pins position.
    for (_label, win) in app.webview_windows() {
        if win.is_visible().unwrap_or(false) {
            kick_window(&win);
        }
    }
    DISPLAY_CHANGED_SINCE_SHOW.store(true, Ordering::SeqCst);
    positioned
}

#[cfg(windows)]
mod win {
    use super::react;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::OnceLock;
    use tauri::{AppHandle, Emitter, Manager};
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

    // Reposition retry checkpoints (ms), measured from the first post-debounce
    // reaction. Windows can take a couple of seconds after the last WM burst to
    // publish settled monitor/work-area data, so if the first attempt lands
    // off-screen we re-attempt on a widening schedule before giving up.
    const RETRY_SCHEDULE_MS: [u64; 3] = [1000, 2500, 5000];

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
        // an atomic bump plus a spawn. The same thread also drives the retry
        // chain (it only sleeps), so no extra spawn is needed.
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(DEBOUNCE_MS));
            if GENERATION.load(Ordering::SeqCst) != my_gen {
                return; // superseded by a later message in the same burst
            }
            let Some(app) = APP.get() else {
                return;
            };
            // First reaction: reposition + emit + compositor kicks.
            if react(app) {
                return; // validated on the first try — topology already settled
            }
            // The reposition landed off-screen (stale topology). Retry on a
            // widening schedule until it settles or a newer burst supersedes us.
            run_retry_chain(app, my_gen);
        });
    }

    /// Re-attempt the validating reposition at [`RETRY_SCHEDULE_MS`] checkpoints.
    /// Bails immediately if a newer WM burst bumped the generation. On the first
    /// successful reposition, re-emits `display-changed` so the frontend
    /// re-fetches the now-settled anchor. If every checkpoint fails but a primary
    /// monitor still exists, force-centres there as a terminal fallback.
    fn run_retry_chain(app: &AppHandle, my_gen: u64) {
        let mut elapsed = 0u64;
        for target in RETRY_SCHEDULE_MS {
            std::thread::sleep(std::time::Duration::from_millis(target - elapsed));
            elapsed = target;
            if GENERATION.load(Ordering::SeqCst) != my_gen {
                return; // superseded by a newer display-change burst
            }
            if crate::position_docker_widget(app) {
                // Settled — let the frontend re-fetch the correct anchor.
                let _ = app.emit("display-changed", ());
                return;
            }
        }
        // Retries exhausted and still off-screen. Force-centre on the primary
        // monitor (if one exists) rather than leave the widget stranded.
        if GENERATION.load(Ordering::SeqCst) != my_gen {
            return;
        }
        if crate::force_center_docker_widget(app) {
            let _ = app.emit("display-changed", ());
        }
    }
}
