//! Low-level keyboard hook for `Win+V` and `Win+Shift+V`.
//!
//! We deliberately replace the native Windows clipboard history (also bound
//! to `Win+V`) with our own. `RegisterHotKey` can't take either chord — the
//! Windows shell already owns `Win+V`, and on managed corporate machines
//! `Win+Shift+V` is frequently claimed by DLP / EDR agents, both of which
//! cause `RegisterHotKey` to fail with `ERROR_HOTKEY_ALREADY_REGISTERED`.
//! `SetWindowsHookEx(WH_KEYBOARD_LL)` runs *before* hotkey dispatch, so our
//! hook sees the chord first, fires the show-clipboard handler, and returns
//! `LRESULT(1)` to swallow the keystroke before the shell (or any other
//! registered hotkey owner) ever sees it. This is the same approach Ditto
//! and other clipboard managers use to override system shortcuts.
//!
//! Both `Win+V` and `Win+Shift+V` are intercepted here. The Shift modifier
//! selects the initial filter (`None` = all entries, `Some("pinned")` =
//! pinned only) so a single hook handles both chords without races.
//!
//! Implementation notes:
//! - WH_KEYBOARD_LL does not require DLL injection; the hook callback fires
//!   on the thread that called `SetWindowsHookEx`. That thread MUST run a
//!   message pump or Windows will silently bypass the hook after ~300ms.
//! - The hook proc must return fast; we just check the chord and dispatch
//!   the actual window toggle onto the Tauri async runtime.
//! - On non-Windows builds the module is a no-op so the crate still compiles.

#[cfg(windows)]
pub fn spawn(app: tauri::AppHandle) {
    std::thread::Builder::new()
        .name("fnba-clipboard-hotkey".into())
        .spawn(move || win::run_hook_thread(app))
        .expect("failed to spawn clipboard hotkey thread");
}

#[cfg(not(windows))]
pub fn spawn(_app: tauri::AppHandle) {}

#[cfg(windows)]
mod win {
    use std::sync::OnceLock;
    use tauri::AppHandle;
    use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_LSHIFT, VK_LWIN, VK_RSHIFT, VK_RWIN, VK_V,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage,
        UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN,
    };

    static APP: OnceLock<AppHandle> = OnceLock::new();
    static HOOK: OnceLock<HhookWrapper> = OnceLock::new();

    // HHOOK is just a raw handle; wrap to make it Send + Sync (we only ever
    // read it from the hook proc thread anyway).
    struct HhookWrapper(HHOOK);
    unsafe impl Send for HhookWrapper {}
    unsafe impl Sync for HhookWrapper {}

    pub fn run_hook_thread(app: AppHandle) {
        let _ = APP.set(app);
        unsafe {
            let hook = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(ll_keyboard_proc), HINSTANCE::default(), 0) {
                Ok(h) => h,
                Err(e) => {
                    eprintln!("clipboard hotkey: SetWindowsHookExW failed: {e}");
                    return;
                }
            };
            let _ = HOOK.set(HhookWrapper(hook));

            // Pump messages so Windows continues to dispatch hook events to
            // this thread. Without a pump, the hook is silently bypassed.
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

    extern "system" fn ll_keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            let msg = wparam.0 as u32;
            if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
                let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
                if info.vkCode == VK_V.0 as u32 && win_held() {
                    let filter: Option<&'static str> = if shift_held() { Some("pinned") } else { None };
                    if let Some(app) = APP.get() {
                        let app = app.clone();
                        // Run the actual show off the hook thread so we
                        // return well under the LowLevelHooksTimeout (300ms).
                        //
                        // Use a plain OS thread, not `tauri::async_runtime::spawn`:
                        // the hook fires very early in app startup, before
                        // `.run()` has entered the tokio event loop, and futures
                        // scheduled in that window can sit un-polled until the
                        // user triggers some other code path that yields control
                        // to the runtime (e.g. pressing `Win+Shift+F`). Tauri's
                        // window methods are thread-safe and internally post to
                        // the UI thread, so a sync std thread is fine.
                        std::thread::spawn(move || {
                            crate::clipboard::show_clipboard_window(&app, filter);
                        });
                    }
                    // Swallow the chord so the Windows shell's native Win+V
                    // (clipboard history) and any other registered owner of
                    // Win+Shift+V (DLP agents etc.) don't also fire.
                    return LRESULT(1);
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

    fn win_held() -> bool {
        unsafe {
            (GetAsyncKeyState(VK_LWIN.0 as i32) as u16 & 0x8000) != 0
                || (GetAsyncKeyState(VK_RWIN.0 as i32) as u16 & 0x8000) != 0
        }
    }

    fn shift_held() -> bool {
        unsafe {
            (GetAsyncKeyState(VK_LSHIFT.0 as i32) as u16 & 0x8000) != 0
                || (GetAsyncKeyState(VK_RSHIFT.0 as i32) as u16 & 0x8000) != 0
        }
    }
}
