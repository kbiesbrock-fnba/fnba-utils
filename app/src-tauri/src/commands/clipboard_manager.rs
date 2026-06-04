//! Tauri command surface for the Clipboard Manager.
//!
//! Wraps `ClipboardHistoryState` (SQLite) and the `clipboard::paste` helpers.
//! Sensitive entries require an explicit reveal token before they can be
//! pasted; the token is short-lived and bound to the entry id.

use crate::clipboard::ForegroundCapture;
use crate::state::clipboard_history::{
    ClipboardEntryFull, ClipboardEntrySummary, ClipboardHistoryState, ClipboardSettings,
    UpdateContentOutcome,
};
use crate::state::test_users::{TestUser, TestUsersState};
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
    /// When true on a sensitive entry, paste the original captured text and
    /// require a valid reveal_token. When false (default), paste the stored
    /// obfuscated/test-user-substituted text — no token needed. Non-sensitive
    /// entries ignore this flag.
    #[serde(default)]
    pub paste_original: bool,
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

    // Decide what bytes actually go on the OS clipboard. For sensitive
    // entries, the default (`paste_original = false`) writes the obfuscated
    // text — no reveal token needed. Original requires a valid token. If a
    // sensitive entry has no stored obfuscation (image flagged only by an
    // OS-marker), we fall through to requiring the token even on the default
    // path, because there's no safe substitute to paste.
    let use_obfuscated = entry.sensitive
        && !options.paste_original
        && entry.obfuscated_text.as_deref().is_some();

    if entry.sensitive && !use_obfuscated {
        let ok = options
            .reveal_token
            .as_deref()
            .map(|t| reveal_tokens.consume(id, t))
            .unwrap_or(false);
        if !ok {
            return Err(
                "sensitive entry: pass paste_original=false to paste the safe version, or \
                 request_sensitive_reveal first to paste the original"
                    .into(),
            );
        }
    }

    #[cfg(windows)]
    {
        if use_obfuscated {
            // SAFETY: use_obfuscated only true when obfuscated_text is Some.
            let obfuscated = entry.obfuscated_text.as_deref().unwrap_or("");
            // Mark our own write so the daemon's listener doesn't re-capture
            // (or re-scan) this as a fresh entry. Recorded in the shared DB
            // because the listener runs in the daemon process, not here. Hash
            // uses the same `txt:` prefix the listener uses for plain-text.
            state.mark_self_write(&crate::clipboard::listener::compute_text_hash(obfuscated));
            let mut cb = arboard::Clipboard::new()
                .map_err(|e| format!("clipboard open: {e}"))?;
            cb.set_text(obfuscated)
                .map_err(|e| format!("set obfuscated text: {e}"))?;
        } else {
            state.mark_self_write(&entry.content_hash);
            crate::clipboard::paste::set_clipboard(&entry)?;
        }
        if options.simulate_paste {
            let prior = foreground.take();
            crate::clipboard::paste::simulate_paste(prior)?;
        }
    }
    #[cfg(not(windows))]
    {
        let _ = foreground.take();
        let mut cb = arboard::Clipboard::new().map_err(|e| format!("clipboard open: {e}"))?;
        if use_obfuscated {
            let obfuscated = entry.obfuscated_text.as_deref().unwrap_or("");
            cb.set_text(obfuscated).map_err(|e| format!("set obfuscated text: {e}"))?;
        } else if let Some(t) = entry.text_content.as_deref() {
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
pub async fn set_clipboard_entry_label(
    state: State<'_, ClipboardHistoryState>,
    app: AppHandle,
    id: i64,
    label: Option<String>,
) -> Result<(), String> {
    let trimmed = label.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let ok = state.set_label(id, trimmed)?;
    if !ok {
        return Err(format!("clipboard entry {id} not found"));
    }
    let _ = app.emit("clipboard-entry-updated", id);
    Ok(())
}

#[tauri::command]
pub async fn update_clipboard_entry_content(
    state: State<'_, ClipboardHistoryState>,
    app: AppHandle,
    id: i64,
    content: String,
) -> Result<(), String> {
    let new_hash = crate::clipboard::listener::compute_text_hash(&content);
    let byte_size = content.len() as i64;
    match state.update_text_content(id, &content, &new_hash, byte_size)? {
        UpdateContentOutcome::Updated => {
            // Mark the new hash as a self-write so an OS-clipboard echo of
            // the edited value (e.g. immediately pasting it) isn't captured
            // as a fresh entry by the daemon's listener.
            state.mark_self_write(&new_hash);
            let _ = app.emit("clipboard-entry-updated", id);
            Ok(())
        }
        UpdateContentOutcome::Blocked => Err(
            "cannot edit a sensitive or image entry; clear the sensitive flag first".into(),
        ),
        UpdateContentOutcome::Duplicate => Err(
            "another clipboard entry already has this exact content".into(),
        ),
        UpdateContentOutcome::NotFound => Err(format!("clipboard entry {id} not found")),
    }
}

#[tauri::command]
pub async fn set_clipboard_entry_sensitivity(
    state: State<'_, ClipboardHistoryState>,
    test_users: State<'_, TestUsersState>,
    app: AppHandle,
    id: i64,
    sensitive: bool,
) -> Result<(), String> {
    let ok = if sensitive {
        let entry = state
            .get(id)?
            .ok_or_else(|| format!("clipboard entry {id} not found"))?;
        let plain = entry.text_content.unwrap_or_default();
        let res = crate::clipboard::pii::scan(&plain);
        let kinds: Vec<String> = res
            .kinds()
            .into_iter()
            .map(|k| k.as_str().to_string())
            .collect();
        let user = test_users.pick_random_enabled().ok().flatten();
        // When PII detection found something, substitute against the test
        // user (or mask-fallback). When it found nothing but the user is
        // explicitly tagging this as sensitive, replace the whole text with
        // an "***" mask so the obfuscated paste path has something safe.
        let obfuscated = if res.detections.is_empty() {
            if plain.is_empty() { String::new() } else { "***".to_string() }
        } else {
            crate::clipboard::pii::substitute(&plain, &res.detections, user.as_ref())
        };
        let user_id = user.and_then(|u| u.id);
        state.set_sensitivity(id, true, Some(obfuscated.as_str()), user_id, &kinds)?
    } else {
        state.set_sensitivity(id, false, None, None, &[])?
    };
    if !ok {
        return Err(format!("clipboard entry {id} not found"));
    }
    let _ = app.emit("clipboard-entry-updated", id);
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

// --- Test Users (PII substitution pool) ---

#[tauri::command]
pub async fn list_test_users(
    state: State<'_, TestUsersState>,
) -> Result<Vec<TestUser>, String> {
    state.list_all()
}

#[tauri::command]
pub async fn upsert_test_user(
    state: State<'_, TestUsersState>,
    user: TestUser,
) -> Result<i64, String> {
    state.upsert(&user)
}

#[tauri::command]
pub async fn delete_test_user(
    state: State<'_, TestUsersState>,
    id: i64,
) -> Result<(), String> {
    state.delete(id)?;
    Ok(())
}

#[tauri::command]
pub async fn set_test_user_enabled(
    state: State<'_, TestUsersState>,
    id: i64,
    enabled: bool,
) -> Result<(), String> {
    state.set_enabled(id, enabled)
}
