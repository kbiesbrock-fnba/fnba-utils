//! Paste-back helpers.
//!
//! `set_clipboard` writes a clipboard entry back to the OS clipboard (text,
//! HTML, or PNG image). `simulate_paste` returns focus to the window that was
//! foregrounded before our launcher took it, then synthesizes Ctrl+V so the
//! pick lands in the original app.

use crate::state::clipboard_history::ClipboardEntryFull;
use windows::core::w;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::DataExchange::RegisterClipboardFormatW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VIRTUAL_KEY, VK_CONTROL, VK_V,
};
use windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow;

pub fn set_clipboard(entry: &ClipboardEntryFull) -> Result<(), String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| format!("clipboard open: {e}"))?;
    match entry.kind.as_str() {
        "image" => {
            let b64 = entry
                .image_base64
                .as_deref()
                .ok_or_else(|| "image entry missing payload".to_string())?;
            use base64::Engine;
            let png = base64::engine::general_purpose::STANDARD
                .decode(b64)
                .map_err(|e| format!("base64 decode: {e}"))?;
            let img = image::load_from_memory_with_format(&png, image::ImageFormat::Png)
                .map_err(|e| format!("decode png: {e}"))?
                .to_rgba8();
            let (w, h) = img.dimensions();
            let bytes = img.into_raw();
            cb.set_image(arboard::ImageData {
                width: w as usize,
                height: h as usize,
                bytes: bytes.into(),
            })
            .map_err(|e| format!("set image: {e}"))?;
        }
        "html" => {
            // arboard 3.4+ exposes set_html which writes both HTML and a plain
            // fallback so non-HTML-aware targets still get something useful.
            let html = entry
                .html_content
                .as_deref()
                .ok_or_else(|| "html entry missing payload".to_string())?;
            let alt = entry.text_content.as_deref();
            cb.set_html(html, alt).map_err(|e| format!("set html: {e}"))?;
        }
        _ => {
            let txt = entry
                .text_content
                .as_deref()
                .ok_or_else(|| "text entry missing payload".to_string())?;
            cb.set_text(txt).map_err(|e| format!("set text: {e}"))?;
        }
    }
    Ok(())
}

pub fn simulate_paste(prior_hwnd: Option<isize>) -> Result<(), String> {
    if let Some(raw) = prior_hwnd {
        let hwnd = HWND(raw as *mut _);
        unsafe {
            let _ = SetForegroundWindow(hwnd);
        }
        // Give the OS a moment to actually move focus before we synthesize keys.
        std::thread::sleep(std::time::Duration::from_millis(80));
    }

    let inputs = [
        keyboard_input(VK_CONTROL, false),
        keyboard_input(VK_V, false),
        keyboard_input(VK_V, true),
        keyboard_input(VK_CONTROL, true),
    ];
    let sent =
        unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent as usize != inputs.len() {
        return Err(format!(
            "SendInput sent {} of {} events",
            sent,
            inputs.len()
        ));
    }
    Ok(())
}

fn keyboard_input(key: VIRTUAL_KEY, key_up: bool) -> INPUT {
    let flags = if key_up {
        KEYEVENTF_KEYUP
    } else {
        KEYBD_EVENT_FLAGS(0)
    };
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// Force-register the HTML format ID. arboard already does this, but having
/// our own reference keeps the symbol used in cfg(windows) builds.
#[allow(dead_code)]
pub fn html_format_id() -> u32 {
    unsafe { RegisterClipboardFormatW(w!("HTML Format")) }
}
