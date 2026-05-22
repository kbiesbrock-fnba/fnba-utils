//! Low-level keyboard hook for `Win+V`.
//!
//! We deliberately replace the native Windows clipboard history (also bound
//! to `Win+V`) with our own. `RegisterHotKey` can't take the chord — the
//! Windows shell already owns it, and that API is per-process with no
//! force-overtake. `SetWindowsHookEx(WH_KEYBOARD_LL)` runs *before* hotkey
//! dispatch, so our hook sees `Win+V` first, fires our show-clipboard
//! handler, and returns `LRESULT(1)` to swallow the keystroke before the
//! shell ever sees it. This is the same approach Ditto and other clipboard
//! managers use to override the native shortcut.
//!
//! We only intercept `Win+V` with Shift *not* held, so `Win+Shift+V` (used by
//! PowerToys Advanced Paste, etc.) continues to work normally.
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
                if info.vkCode == VK_V.0 as u32 && win_held() && !shift_held() {
                    if let Some(app) = APP.get() {
                        let app = app.clone();
                        // Run the actual show off the hook thread so we
                        // return well under the LowLevelHooksTimeout (300ms).
                        tauri::async_runtime::spawn(async move {
                            crate::clipboard::show_clipboard_window(&app);
                        });
                    }
                    // Swallow the chord so the Windows shell's native Win+V
                    // (clipboard history) doesn't also fire.
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
