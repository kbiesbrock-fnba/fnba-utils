//! Tauri command surface for the Clipboard Manager.
//!
//! Wraps `ClipboardHistoryState` (SQLite) and the `clipboard::paste` helpers.
//! Sensitive entries require an explicit reveal token before they can be
//! pasted; the token is short-lived and bound to the entry id.

use crate::clipboard::ForegroundCapture;
use crate::state::clipboard_history::{
    ClipboardEntryFull, ClipboardEntrySummary, ClipboardHistoryState, ClipboardSettings,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, State};

const REVEAL_TTL: Duration = Duration::from_secs(15);

#[derive(Default)]
pub struct RevealTokens {
    inner: Mutex<HashMap<i64, (String, Instant)>>,
}

impl RevealTokens {
    fn issue(&self, id: i64) -> String {
        let token = uuid::Uuid::new_v4().to_string();
        if let Ok(mut g) = self.inner.lock() {
            g.retain(|_, (_, t)| t.elapsed() < REVEAL_TTL);
            g.insert(id, (token.clone(), Instant::now()));
        }
        token
    }

    fn consume(&self, id: i64, token: &str) -> bool {
        let Ok(mut g) = self.inner.lock() else {
            return false;
        };
        let Some((stored, when)) = g.get(&id) else {
            return false;
        };
        if when.elapsed() > REVEAL_TTL {
            g.remove(&id);
            return false;
        }
        if stored != token {
            return false;
        }
        g.remove(&id);
        true
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevealToken {
    pub id: i64,
    pub token: String,
    pub expires_in_ms: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasteOptions {
    pub simulate_paste: bool,
    #[serde(default)]
    pub reveal_token: Option<String>,
}

#[tauri::command]
pub async fn list_clipboard_entries(
    state: State<'_, ClipboardHistoryState>,
    query: Option<String>,
    kind: Option<String>,
    pinned_only: Option<bool>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<ClipboardEntrySummary>, String> {
    let q = query
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    state.list(
        q,
        kind.as_deref(),
        pinned_only.unwrap_or(false),
        limit.unwrap_or(100).min(500),
        offset.unwrap_or(0),
    )
}

#[tauri::command]
pub async fn get_clipboard_entry(
    state: State<'_, ClipboardHistoryState>,
    id: i64,
) -> Result<Option<ClipboardEntryFull>, String> {
    state.get(id)
}

#[tauri::command]
pub async fn paste_clipboard_entry(
    state: State<'_, ClipboardHistoryState>,
    foreground: State<'_, ForegroundCapture>,
    reveal_tokens: State<'_, RevealTokens>,
    id: i64,
    options: PasteOptions,
) -> Result<(), String> {
    let entry = state
        .get(id)?
        .ok_or_else(|| format!("clipboard entry {id} not found"))?;
    if entry.sensitive {
        let ok = options
            .reveal_token
            .as_deref()
            .map(|t| reveal_tokens.consume(id, t))
            .unwrap_or(false);
        if !ok {
            return Err("entry is marked sensitive; call request_sensitive_reveal first".into());
        }
    }
    #[cfg(windows)]
    {
        crate::clipboard::listener::mark_self_write(entry.content_hash.clone());
        crate::clipboard::paste::set_clipboard(&entry)?;
        if options.simulate_paste {
            let prior = foreground.take();
            crate::clipboard::paste::simulate_paste(prior)?;
        }
    }
    #[cfg(not(windows))]
    {
        // On non-Windows hosts we can still set the clipboard via arboard; we
        // just can't simulate paste. The frontend treats simulate_paste as
        // best-effort, so silently dropping it is acceptable.
        let _ = foreground.take();
        let _ = options;
        let mut cb = arboard::Clipboard::new().map_err(|e| format!("clipboard open: {e}"))?;
        if let Some(t) = entry.text_content.as_deref() {
            cb.set_text(t).map_err(|e| format!("set text: {e}"))?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn request_sensitive_reveal(
    state: State<'_, ClipboardHistoryState>,
    reveal_tokens: State<'_, RevealTokens>,
    id: i64,
) -> Result<RevealToken, String> {
    let entry = state
        .get(id)?
        .ok_or_else(|| format!("clipboard entry {id} not found"))?;
    if !entry.sensitive {
        return Err("entry is not marked sensitive".into());
    }
    let token = reveal_tokens.issue(id);
    Ok(RevealToken {
        id,
        token,
        expires_in_ms: REVEAL_TTL.as_millis() as u64,
    })
}

#[tauri::command]
pub async fn delete_clipboard_entry(
    state: State<'_, ClipboardHistoryState>,
    app: AppHandle,
    id: i64,
) -> Result<(), String> {
    state.delete(id)?;
    let _ = app.emit("clipboard-entry-removed", id);
    Ok(())
}

#[tauri::command]
pub async fn pin_clipboard_entry(
    state: State<'_, ClipboardHistoryState>,
    app: AppHandle,
    id: i64,
    pinned: bool,
) -> Result<(), String> {
    state.set_pinned(id, pinned)?;
    let _ = app.emit("clipboard-entry-pinned", (id, pinned));
    Ok(())
}

#[tauri::command]
pub async fn clear_clipboard_history(
    state: State<'_, ClipboardHistoryState>,
    app: AppHandle,
    include_pinned: bool,
) -> Result<usize, String> {
    let n = state.clear(include_pinned)?;
    let _ = app.emit("clipboard-history-cleared", n);
    Ok(n)
}

#[tauri::command]
pub async fn get_clipboard_settings(
    state: State<'_, ClipboardHistoryState>,
) -> Result<ClipboardSettings, String> {
    state.get_settings()
}

#[tauri::command]
pub async fn set_clipboard_settings(
    state: State<'_, ClipboardHistoryState>,
    settings: ClipboardSettings,
) -> Result<(), String> {
    state.set_settings(&settings)
}

#[tauri::command]
pub async fn hide_clipboard_window(window: tauri::WebviewWindow) {
    let _ = window.hide();
}

#[tauri::command]
pub async fn get_clipboard_max_captured_at(
    state: State<'_, ClipboardHistoryState>,
) -> Result<i64, String> {
    state.max_captured_at()
}
