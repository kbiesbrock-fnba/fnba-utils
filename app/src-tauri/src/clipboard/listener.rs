//! Win32 clipboard listener.
//!
//! Spawns one dedicated OS thread that owns a hidden message-only window
//! registered as a clipboard listener via `AddClipboardFormatListener`.
//! Whenever the OS fires `WM_CLIPBOARDUPDATE`, the thread reads the clipboard
//! contents (text / HTML / image), detects sensitivity markers, captures the
//! owning process name, hashes the content, and sends a `NewClipboardEntry`
//! over a `tokio::sync::mpsc::UnboundedSender` for the main runtime to insert
//! into the SQLite store.
//!
//! On non-Windows platforms this module is a no-op so the crate still builds.

use crate::clipboard::pii;
use crate::state::clipboard_history::{ClipboardKind, NewClipboardEntry};
use crate::state::test_users::TestUser;
use std::sync::OnceLock;
use tokio::sync::mpsc::UnboundedSender;

pub type ClipboardEventSender = UnboundedSender<NewClipboardEntry>;

/// Callback the host process installs so the listener thread can pick a test
/// user without depending on Tauri State. Set once at startup by either
/// `clipd/src/main.rs` (daemon) or `lib.rs` (UI process).
type TestUserPicker = Box<dyn Fn() -> Option<TestUser> + Send + Sync>;
static TEST_USER_PICKER: OnceLock<TestUserPicker> = OnceLock::new();

/// Install the test-user picker used by the PII substitutor. Subsequent calls
/// are no-ops (first wins) — both `clipd` and `fnba-utils` may try to set this
/// when running the listener pipeline; either is fine.
pub fn install_test_user_picker<F>(f: F)
where
    F: Fn() -> Option<TestUser> + Send + Sync + 'static,
{
    let _ = TEST_USER_PICKER.set(Box::new(f));
}

#[allow(dead_code)] // Only called from the windows-only `read_clipboard_snapshot`.
fn current_test_user() -> Option<TestUser> {
    TEST_USER_PICKER.get().and_then(|p| p())
}

/// Cross-process self-write registry hook. The process that runs the listener
/// (the daemon) installs closures backed by the shared clipboard DB, so a
/// clipboard write performed in *either* process — PII substitution here, or a
/// paste from the manager in the UI process — is recognized as our own echo
/// and not re-captured. The DB is the channel because the listener and the
/// paste command live in different processes; an in-process marker (the old
/// approach) was invisible across that boundary.
struct SelfWriteStore {
    mark: Box<dyn Fn(&str) + Send + Sync>,
    is_self: Box<dyn Fn(&str) -> bool + Send + Sync>,
}
static SELF_WRITE_STORE: OnceLock<SelfWriteStore> = OnceLock::new();

/// Install the self-write store. First call wins (mirrors
/// [`install_test_user_picker`]).
pub fn install_self_write_store<M, T>(mark: M, is_self: T)
where
    M: Fn(&str) + Send + Sync + 'static,
    T: Fn(&str) -> bool + Send + Sync + 'static,
{
    let _ = SELF_WRITE_STORE.set(SelfWriteStore {
        mark: Box::new(mark),
        is_self: Box::new(is_self),
    });
}

/// Record a clipboard write we just performed so the listener ignores its echo.
/// No-op until a store is installed (the daemon does so at startup).
pub fn mark_self_write(hash: String) {
    if let Some(store) = SELF_WRITE_STORE.get() {
        (store.mark)(&hash);
    }
}

/// True if this clipboard update is an echo of a write we made (and so should
/// be ignored). Non-consuming — see `ClipboardHistoryState::is_self_write`.
#[allow(dead_code)] // Only called from the windows-only `read_clipboard_snapshot`.
fn is_self_write(hash: &str) -> bool {
    SELF_WRITE_STORE
        .get()
        .map(|store| (store.is_self)(hash))
        .unwrap_or(false)
}

/// Run PII scan + substitution on a captured text projection. Returns
/// `(is_pii, obfuscated_text, test_user_id, pii_kinds)`. When PII is detected
/// we **also replace the OS clipboard** with the obfuscated text — that way
/// an immediate Ctrl+V (without ever opening Win+V) pastes the safe version.
/// The original is still preserved in the DB and reachable via
/// `Ctrl+Shift+Enter` from the manager window.
#[allow(dead_code)] // Only called from the windows-only `read_clipboard_snapshot`.
fn scan_and_substitute(text: &str) -> (bool, Option<String>, Option<i64>, Vec<String>) {
    let res = pii::scan(text);
    if !res.is_sensitive() {
        return (false, None, None, Vec::new());
    }
    let kinds: Vec<String> = res.kinds().into_iter().map(|k| k.as_str().to_string()).collect();
    let user = current_test_user();
    let obfuscated = pii::substitute(text, &res.detections, user.as_ref());
    let user_id = user.and_then(|u| u.id);

    // Proactive protection: write the obfuscated text back to the OS clipboard
    // before returning. Mark our own write so the next WM_CLIPBOARDUPDATE
    // doesn't re-capture the obfuscation as a fresh entry.
    replace_clipboard_with_obfuscated(&obfuscated);

    (true, Some(obfuscated), user_id, kinds)
}

#[cfg(windows)]
fn replace_clipboard_with_obfuscated(obfuscated: &str) {
    let hash = win::compute_text_hash(obfuscated);
    mark_self_write(hash);
    if let Ok(mut cb) = arboard::Clipboard::new() {
        let _ = cb.set_text(obfuscated);
    }
}

#[cfg(not(windows))]
fn replace_clipboard_with_obfuscated(_obfuscated: &str) {}

#[cfg(windows)]
pub fn spawn(tx: ClipboardEventSender) {
    std::thread::Builder::new()
        .name("fnba-clipboard-listener".into())
        .spawn(move || win::run_listener_thread(tx))
        .expect("failed to spawn clipboard listener thread");
}

#[cfg(not(windows))]
pub fn spawn(_tx: ClipboardEventSender) {
    // Clipboard capture is Windows-only. The frontend still works against the
    // empty history; live capture just won't fire.
}

#[cfg(windows)]
mod win {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::sync::OnceLock;
    use windows::core::{w, PWSTR};
    use windows::Win32::Foundation::{
        CloseHandle, GetLastError, BOOL, HGLOBAL, HWND, LPARAM, LRESULT, WPARAM,
    };
    use windows::Win32::Graphics::Gdi::HBRUSH;
    use windows::Win32::System::DataExchange::{
        AddClipboardFormatListener, CloseClipboard, EnumClipboardFormats, GetClipboardData,
        GetClipboardOwner, OpenClipboard, RegisterClipboardFormatW,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetWindowThreadProcessId,
        RegisterClassExW, TranslateMessage, CW_USEDEFAULT, HCURSOR, HICON, HMENU, HWND_MESSAGE, MSG,
        WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLIPBOARDUPDATE, WNDCLASSEXW,
    };

    static SENDER: OnceLock<ClipboardEventSender> = OnceLock::new();

    pub fn run_listener_thread(tx: ClipboardEventSender) {
        let _ = SENDER.set(tx);

        unsafe {
            let h_instance = match GetModuleHandleW(None) {
                Ok(h) => h,
                Err(e) => {
                    eprintln!("clipboard listener: GetModuleHandleW failed: {e}");
                    return;
                }
            };

            let class_name = w!("FnbaClipboardListenerCls");
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(wnd_proc),
                hInstance: h_instance.into(),
                lpszClassName: class_name,
                hIcon: HICON::default(),
                hCursor: HCURSOR::default(),
                hbrBackground: HBRUSH::default(),
                hIconSm: HICON::default(),
                ..Default::default()
            };
            let atom = RegisterClassExW(&wc);
            if atom == 0 {
                eprintln!(
                    "clipboard listener: RegisterClassExW failed ({:?})",
                    GetLastError()
                );
                return;
            }

            let hwnd = match CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class_name,
                w!("FNBA Clipboard Listener"),
                WINDOW_STYLE::default(),
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                0,
                0,
                HWND_MESSAGE,
                HMENU::default(),
                h_instance,
                None,
            ) {
                Ok(h) => h,
                Err(e) => {
                    eprintln!("clipboard listener: CreateWindowExW failed: {e}");
                    return;
                }
            };

            if let Err(e) = AddClipboardFormatListener(hwnd) {
                eprintln!("clipboard listener: AddClipboardFormatListener failed: {e}");
                return;
            }

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, HWND::default(), 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if msg == WM_CLIPBOARDUPDATE {
            if let Some(tx) = SENDER.get() {
                // Self-write filtering happens inside read_clipboard_snapshot,
                // per content kind, BEFORE the PII scan — so the echo of our
                // own obfuscation can't be re-scanned and re-substituted in a
                // loop. A snapshot that's our own echo comes back as None.
                if let Some(entry) = read_clipboard_snapshot() {
                    let _ = tx.send(entry);
                }
            }
            return LRESULT(0);
        }
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }

    const CF_TEXT: u32 = 1;
    const CF_BITMAP: u32 = 2;
    const CF_UNICODETEXT: u32 = 13;
    const CF_DIB: u32 = 8;
    const CF_DIBV5: u32 = 17;

    fn read_clipboard_snapshot() -> Option<NewClipboardEntry> {
        unsafe {
            if OpenClipboard(HWND::default()).is_err() {
                return None;
            }

            let formats = enumerate_formats();
            let os_sensitive = detect_sensitive(&formats);
            let source_process = read_source_process();

            let html_format_id = RegisterClipboardFormatW(w!("HTML Format"));

            let has_text = formats.contains(&CF_UNICODETEXT) || formats.contains(&CF_TEXT);
            let has_html = html_format_id != 0 && formats.contains(&html_format_id);
            let has_image = formats.contains(&CF_DIBV5)
                || formats.contains(&CF_DIB)
                || formats.contains(&CF_BITMAP);

            let text = if has_text { read_unicode_text() } else { None };
            let html = if has_html { read_html(html_format_id) } else { None };

            // Done with raw Win32 reads — close before handing off to arboard
            // (which opens the clipboard itself for image fetches).
            let _ = CloseClipboard();

            // Priority: image > html > text. Images carry their own preview
            // (thumbnail) and shouldn't be downgraded to "text" just because
            // some apps also stuff CF_UNICODETEXT in alongside.
            if has_image {
                if let Some((png, thumb, w, h, byte_size)) = read_image_via_arboard() {
                    let hash = hash_bytes(&[b"img:", &png]);
                    // Ignore the echo of an image we wrote ourselves (e.g. a
                    // paste-back from the manager).
                    if super::is_self_write(&hash) {
                        return None;
                    }
                    return Some(NewClipboardEntry {
                        kind: ClipboardKind::Image,
                        text_content: None,
                        html_content: None,
                        image_png: Some(png),
                        thumb_png: Some(thumb),
                        width: Some(w),
                        height: Some(h),
                        byte_size,
                        sensitive: os_sensitive,
                        obfuscated_text: None,
                        test_user_id: None,
                        pii_kinds: Vec::new(),
                        source_process,
                        content_hash: hash,
                    });
                }
            }

            if let Some(html_raw) = html {
                let hash = hash_bytes(&[b"html:", html_raw.as_bytes()]);
                // Ignore the echo of our own clipboard write before scanning
                // (see the text branch below for why the ordering matters).
                if super::is_self_write(&hash) {
                    return None;
                }
                let plain = text.clone().unwrap_or_else(|| strip_html(&html_raw));
                let (has_pii, obfuscated, test_user_id, pii_kinds) =
                    super::scan_and_substitute(&plain);
                let byte_size = html_raw.len() as i64;
                return Some(NewClipboardEntry {
                    kind: ClipboardKind::Html,
                    text_content: Some(plain),
                    html_content: Some(html_raw),
                    image_png: None,
                    thumb_png: None,
                    width: None,
                    height: None,
                    byte_size,
                    sensitive: os_sensitive || has_pii,
                    obfuscated_text: obfuscated,
                    test_user_id,
                    pii_kinds,
                    source_process,
                    content_hash: hash,
                });
            }

            if let Some(t) = text {
                if t.trim().is_empty() {
                    return None;
                }
                let hash = hash_bytes(&[b"txt:", t.as_bytes()]);
                // Ignore the echo of our own clipboard write. This MUST run
                // before scan_and_substitute: the substituted "safe" value can
                // itself match the PII detectors (a test SSN is still
                // ###-##-####), so re-scanning it would re-substitute and
                // re-write the clipboard in an infinite loop — each iteration
                // also firing a "Clipboard protected" toast. Bailing here also
                // keeps an explicit paste-of-original from being re-obfuscated.
                if super::is_self_write(&hash) {
                    return None;
                }
                let (has_pii, obfuscated, test_user_id, pii_kinds) =
                    super::scan_and_substitute(&t);
                let byte_size = t.len() as i64;
                return Some(NewClipboardEntry {
                    kind: ClipboardKind::Text,
                    text_content: Some(t),
                    html_content: None,
                    image_png: None,
                    thumb_png: None,
                    width: None,
                    height: None,
                    byte_size,
                    sensitive: os_sensitive || has_pii,
                    obfuscated_text: obfuscated,
                    test_user_id,
                    pii_kinds,
                    source_process,
                    content_hash: hash,
                });
            }

            None
        }
    }

    unsafe fn enumerate_formats() -> Vec<u32> {
        let mut out = Vec::new();
        let mut fmt = EnumClipboardFormats(0);
        while fmt != 0 {
            out.push(fmt);
            fmt = EnumClipboardFormats(fmt);
        }
        out
    }

    unsafe fn detect_sensitive(formats: &[u32]) -> bool {
        let markers = [
            w!("ExcludeClipboardContentFromMonitoring"),
            w!("CanIncludeInClipboardHistory"),
            w!("CanUploadToCloudClipboard"),
        ];
        for marker in markers {
            let id = RegisterClipboardFormatW(marker);
            if id != 0 && formats.contains(&id) {
                return true;
            }
        }
        false
    }

    unsafe fn read_source_process() -> Option<String> {
        let owner = GetClipboardOwner().ok()?;
        if owner.0.is_null() {
            return None;
        }
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(owner, Some(&mut pid));
        if pid == 0 {
            return None;
        }
        let h_proc = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, BOOL(0), pid).ok()?;
        let mut buf = vec![0u16; 1024];
        let mut size = buf.len() as u32;
        let res = QueryFullProcessImageNameW(
            h_proc,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buf.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(h_proc);
        if res.is_err() || size == 0 {
            return None;
        }
        let path = String::from_utf16_lossy(&buf[..size as usize]);
        Some(
            path.rsplit('\\')
                .next()
                .unwrap_or(&path)
                .to_string(),
        )
    }

    unsafe fn read_unicode_text() -> Option<String> {
        let handle = GetClipboardData(CF_UNICODETEXT).ok()?;
        let hglobal = HGLOBAL(handle.0 as *mut _);
        let ptr = GlobalLock(hglobal) as *const u16;
        if ptr.is_null() {
            return None;
        }
        let mut len = 0usize;
        while *ptr.add(len) != 0 {
            len += 1;
            if len > 16 * 1024 * 1024 {
                break;
            }
        }
        let slice = std::slice::from_raw_parts(ptr, len);
        let s = String::from_utf16_lossy(slice);
        let _ = GlobalUnlock(hglobal);
        Some(s)
    }

    unsafe fn read_html(format_id: u32) -> Option<String> {
        let handle = GetClipboardData(format_id).ok()?;
        let hglobal = HGLOBAL(handle.0 as *mut _);
        let size = GlobalSize(hglobal);
        if size == 0 {
            return None;
        }
        let ptr = GlobalLock(hglobal) as *const u8;
        if ptr.is_null() {
            return None;
        }
        let slice = std::slice::from_raw_parts(ptr, size);
        // Trim trailing NUL if present.
        let trimmed = match slice.iter().position(|&b| b == 0) {
            Some(n) => &slice[..n],
            None => slice,
        };
        let s = String::from_utf8_lossy(trimmed).to_string();
        let _ = GlobalUnlock(hglobal);
        // The CF_HTML format starts with a header block of "Key:Value" lines
        // up to StartFragment. Extract the fragment body for cleaner storage.
        Some(extract_html_fragment(&s).unwrap_or(s))
    }

    fn extract_html_fragment(raw: &str) -> Option<String> {
        let start_idx = raw.find("StartFragment:")?;
        let end_idx = raw.find("EndFragment:")?;
        let start_byte: usize = raw[start_idx + "StartFragment:".len()..]
            .lines()
            .next()?
            .trim()
            .parse()
            .ok()?;
        let end_byte: usize = raw[end_idx + "EndFragment:".len()..]
            .lines()
            .next()?
            .trim()
            .parse()
            .ok()?;
        if start_byte >= end_byte || end_byte > raw.len() {
            return None;
        }
        Some(raw[start_byte..end_byte].to_string())
    }

    fn strip_html(raw: &str) -> String {
        // Cheap-and-cheerful tag stripper for the plain-text fallback. We're
        // not trying to render HTML; we just need a searchable preview.
        let mut out = String::with_capacity(raw.len());
        let mut in_tag = false;
        for ch in raw.chars() {
            match ch {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ if !in_tag => out.push(ch),
                _ => {}
            }
        }
        out.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn read_image_via_arboard() -> Option<(Vec<u8>, Vec<u8>, u32, u32, i64)> {
        // arboard opens the clipboard itself; safe to call after we've closed
        // our raw read. Returns RGBA bytes which we encode to PNG.
        let mut cb = arboard::Clipboard::new().ok()?;
        let img = cb.get_image().ok()?;
        let w = img.width as u32;
        let h = img.height as u32;
        if w == 0 || h == 0 {
            return None;
        }
        let rgba = img.bytes.into_owned();
        let png = encode_png_rgba(&rgba, w, h)?;
        let thumb = make_thumbnail_png(&rgba, w, h, 256)?;
        let byte_size = png.len() as i64;
        Some((png, thumb, w, h, byte_size))
    }

    fn encode_png_rgba(rgba: &[u8], w: u32, h: u32) -> Option<Vec<u8>> {
        use image::codecs::png::PngEncoder;
        use image::ExtendedColorType;
        use image::ImageEncoder;
        let mut out = Vec::with_capacity(rgba.len() / 4);
        PngEncoder::new(&mut out)
            .write_image(rgba, w, h, ExtendedColorType::Rgba8)
            .ok()?;
        Some(out)
    }

    fn make_thumbnail_png(rgba: &[u8], w: u32, h: u32, max_dim: u32) -> Option<Vec<u8>> {
        use image::imageops::FilterType;
        use image::{ImageBuffer, Rgba};
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_raw(w, h, rgba.to_vec())?;
        let scale = (max_dim as f32 / w.max(h) as f32).min(1.0);
        let new_w = ((w as f32) * scale).max(1.0) as u32;
        let new_h = ((h as f32) * scale).max(1.0) as u32;
        let thumb = image::imageops::resize(&img, new_w, new_h, FilterType::Triangle);
        encode_png_rgba(thumb.as_raw(), new_w, new_h)
    }

    /// Public counterpart of `hash_bytes` for callers in `clipboard::paste`
    /// that need to mark a clipboard self-write before putting obfuscated
    /// text on the clipboard. Must use the exact same prefix the listener
    /// uses for plain-text captures so the dedupe hash matches.
    pub fn compute_text_hash(text: &str) -> String {
        hash_bytes(&[b"txt:", text.as_bytes()])
    }

    fn hash_bytes(parts: &[&[u8]]) -> String {
        let mut h = Sha256::new();
        for p in parts {
            h.update(p);
        }
        let digest = h.finalize();
        let mut s = String::with_capacity(digest.len() * 2);
        for b in digest {
            use std::fmt::Write;
            let _ = write!(s, "{:02x}", b);
        }
        s
    }
}

#[cfg(windows)]
pub use win::compute_text_hash;

#[cfg(not(windows))]
pub fn compute_text_hash(_text: &str) -> String {
    String::new()
}
