//! Click-outside detection for the Docker widget.
//!
//! The widget window is intentionally non-focusable (it must never steal focus
//! / worsen the Win+V hook bug), so it never holds the foreground window and
//! receives no OS blur event. To support "click off to dismiss", we install a
//! `WH_MOUSE_LL` hook (same approach as the clipboard Win+V hook) and, on any
//! mouse-button-down that lands OUTSIDE the widget's window rect, emit a
//! `docker-widget-defocus` event the frontend listens for to collapse.
//!
//! The hook never swallows events (always calls `CallNextHookEx`), so clicks
//! continue to reach whatever window the user actually clicked.

#[cfg(windows)]
use std::sync::atomic::{AtomicIsize, Ordering};

/// Raw HWND of the docker-widget window, published by `track_window` once the
/// window exists. `0` means "unknown" — the hook then does nothing.
#[cfg(windows)]
static WIDGET_HWND: AtomicIsize = AtomicIsize::new(0);

/// Record the widget's native window handle so the hook can hit-test clicks.
#[cfg(windows)]
pub fn track_window(window: &tauri::WebviewWindow) {
    if let Ok(hwnd) = window.hwnd() {
        WIDGET_HWND.store(hwnd.0 as isize, Ordering::Relaxed);
    }
}

#[cfg(not(windows))]
pub fn track_window(_window: &tauri::WebviewWindow) {}

/// Spawn the low-level mouse hook thread.
#[cfg(windows)]
pub fn spawn(app: tauri::AppHandle) {
    std::thread::Builder::new()
        .name("fnba-docker-widget-focus".into())
        .spawn(move || win::run_hook_thread(app))
        .expect("failed to spawn docker widget focus thread");
}

#[cfg(not(windows))]
pub fn spawn(_app: tauri::AppHandle) {}

#[cfg(windows)]
mod win {
    use super::WIDGET_HWND;
    use std::sync::atomic::Ordering;
    use std::sync::OnceLock;
    use tauri::{AppHandle, Emitter};
    use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
    use windows::Win32::Graphics::Gdi::PtInRect;
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetMessageW, GetWindowRect, SetWindowsHookExW,
        TranslateMessage, UnhookWindowsHookEx, HHOOK, MSG, MSLLHOOKSTRUCT, WH_MOUSE_LL,
        WM_LBUTTONDOWN, WM_MBUTTONDOWN, WM_RBUTTONDOWN,
    };

    static APP: OnceLock<AppHandle> = OnceLock::new();
    static HOOK: OnceLock<HhookWrapper> = OnceLock::new();

    // HHOOK is a raw handle; wrap to make it Send + Sync (only read on the hook
    // thread anyway).
    struct HhookWrapper(HHOOK);
    unsafe impl Send for HhookWrapper {}
    unsafe impl Sync for HhookWrapper {}

    pub fn run_hook_thread(app: AppHandle) {
        let _ = APP.set(app);
        unsafe {
            let hook =
                match SetWindowsHookExW(WH_MOUSE_LL, Some(ll_mouse_proc), HINSTANCE::default(), 0) {
                    Ok(h) => h,
                    Err(e) => {
                        eprintln!("docker widget focus: SetWindowsHookExW failed: {e}");
                        return;
                    }
                };
            let _ = HOOK.set(HhookWrapper(hook));

            // A message pump is required for the hook to keep receiving events.
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            if let Some(HhookWrapper(h)) = HOOK.get() {
                let _ = UnhookWindowsHookEx(*h);
            }
        }
    }

    extern "system" fn ll_mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            let msg = wparam.0 as u32;
            if msg == WM_LBUTTONDOWN || msg == WM_RBUTTONDOWN || msg == WM_MBUTTONDOWN {
                let raw = WIDGET_HWND.load(Ordering::Relaxed);
                if raw != 0 {
                    let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
                    let pt: POINT = info.pt;
                    let hwnd = HWND(raw as *mut core::ffi::c_void);
                    let mut rect = RECT::default();
                    let inside = unsafe { GetWindowRect(hwnd, &mut rect) }.is_ok()
                        && unsafe { PtInRect(&rect, pt) }.as_bool();
                    if !inside {
                        if let Some(app) = APP.get() {
                            let _ = app.emit("docker-widget-defocus", ());
                        }
                    }
                }
            }
        }
        unsafe {
            CallNextHookEx(
                HOOK.get().map(|w| w.0).unwrap_or_default(),
                code,
                wparam,
                lparam,
            )
        }
    }
}
