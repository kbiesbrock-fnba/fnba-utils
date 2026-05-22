//! SQLite-backed clipboard history.
//!
//! Persistence: `<wsl_home>/.claude/fnba-mc/clipboard.db` (same parent dir as
//! the other Mission Control state files), falling back to native Windows
//! app-data if no WSL home is reachable. Schema is migrated on first open.
//!
//! Images are stored inline as PNG BLOBs alongside a small thumbnail BLOB so
//! the list view can render previews without a second round-trip. Entries are
//! deduped by sha256 of their content — repeating a copy bumps `captured_at`
//! on the existing row instead of inserting a duplicate.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const DEFAULT_TEXT_CAP: u32 = 5_000;
pub const DEFAULT_IMAGE_CAP: u32 = 500;

fn epoch_ms_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ClipboardKind {
    Text,
    Html,
    Image,
}

impl ClipboardKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Html => "html",
            Self::Image => "image",
        }
    }
}

/// Lightweight row used in the list view — image payloads are excluded; only
/// the small thumbnail (base64 PNG) is sent so the list can render previews
/// without dragging full screenshots over the IPC bridge.
///
/// For sensitive entries, `text_preview` shows the **obfuscated** text so the
/// UI list never leaks the original. The original lives in `text_content` on
/// the full entry and only crosses the bridge when an explicit reveal token
/// is consumed by the paste path.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardEntrySummary {
    pub id: i64,
    pub kind: String,
    pub text_preview: Option<String>,
    pub thumb_base64: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub byte_size: i64,
    pub sensitive: bool,
    pub pii_kinds: Vec<String>,
    pub source_process: Option<String>,
    pub captured_at: i64,
    pub pinned: bool,
}

/// Full entry returned by `get_clipboard_entry`. Image bytes are base64-encoded
/// so the bridge stays uniform across text/html/image kinds.
///
/// `text_content` / `html_content` always hold the **original** captured text.
/// `obfuscated_text` holds the test-user-substituted version (or a keep-last-4
/// mask, if no test user was available at capture time). The paste path
/// chooses between them based on the explicit `pasteOriginal` flag — original
/// requires a reveal token round-trip, obfuscated does not.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardEntryFull {
    pub id: i64,
    pub kind: String,
    pub text_content: Option<String>,
    pub html_content: Option<String>,
    pub image_base64: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub byte_size: i64,
    pub sensitive: bool,
    pub obfuscated_text: Option<String>,
    pub test_user_id: Option<i64>,
    pub pii_kinds: Vec<String>,
    pub source_process: Option<String>,
    pub captured_at: i64,
    pub pinned: bool,
    pub content_hash: String,
}

/// Payload accepted from the clipboard listener.
pub struct NewClipboardEntry {
    pub kind: ClipboardKind,
    pub text_content: Option<String>,
    pub html_content: Option<String>,
    pub image_png: Option<Vec<u8>>,
    pub thumb_png: Option<Vec<u8>>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub byte_size: i64,
    pub sensitive: bool,
    pub obfuscated_text: Option<String>,
    pub test_user_id: Option<i64>,
    pub pii_kinds: Vec<String>,
    pub source_process: Option<String>,
    pub content_hash: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardSettings {
    pub text_cap: u32,
    pub image_cap: u32,
    pub capture_enabled: bool,
    pub ignored_processes: Vec<String>,
}

impl Default for ClipboardSettings {
    fn default() -> Self {
        Self {
            text_cap: DEFAULT_TEXT_CAP,
            image_cap: DEFAULT_IMAGE_CAP,
            capture_enabled: true,
            ignored_processes: Vec::new(),
        }
    }
}

pub struct ClipboardHistoryState {
    conn: Mutex<Connection>,
    #[allow(dead_code)] // kept for debugging / diagnostics
    db_path: PathBuf,
}

/// Result of `insert_or_touch`: tells the caller whether to fire a "new entry"
/// event vs. a "touched existing" event.
pub enum InsertOutcome {
    Inserted(i64),
    Touched(i64),
}

impl ClipboardHistoryState {
    pub fn load() -> Self {
        let db_path = resolve_db_path();
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(&db_path)
            .unwrap_or_else(|e| panic!("Failed to open clipboard DB at {}: {e}", db_path.display()));
        // Network shares (e.g. \\wsl.localhost\...) don't honor SQLite's
        // POSIX file locks reliably; keep the DB on the native Windows volume
        // and let SQLite wait briefly on transient locks instead of erroring.
        let _ = conn.busy_timeout(Duration::from_secs(5));
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        let _ = conn.pragma_update(None, "synchronous", "NORMAL");
        Self::migrate(&conn).expect("clipboard DB migration failed");
        Self {
            conn: Mutex::new(conn),
            db_path,
        }
    }

    fn migrate(conn: &Connection) -> rusqlite::Result<()> {
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS entries (
                id               INTEGER PRIMARY KEY AUTOINCREMENT,
                kind             TEXT NOT NULL,
                text_content     TEXT,
                html_content     TEXT,
                image_png        BLOB,
                thumb_png        BLOB,
                width            INTEGER,
                height           INTEGER,
                byte_size        INTEGER NOT NULL,
                sensitive        INTEGER NOT NULL DEFAULT 0,
                obfuscated_text  TEXT,
                test_user_id     INTEGER,
                pii_kinds        TEXT,
                source_process   TEXT,
                captured_at      INTEGER NOT NULL,
                pinned           INTEGER NOT NULL DEFAULT 0,
                content_hash     TEXT NOT NULL UNIQUE
            );

            CREATE INDEX IF NOT EXISTS idx_entries_captured_at ON entries(captured_at DESC);
            CREATE INDEX IF NOT EXISTS idx_entries_kind        ON entries(kind);
            CREATE INDEX IF NOT EXISTS idx_entries_pinned      ON entries(pinned);

            CREATE TABLE IF NOT EXISTS settings (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            ",
        )?;

        // Add PII columns to pre-existing DBs (CREATE TABLE IF NOT EXISTS is a
        // no-op when the table already exists with the old schema).
        let existing_cols: Vec<String> = conn
            .prepare("PRAGMA table_info(entries)")?
            .query_map([], |r| r.get::<_, String>(1))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (col, ddl) in [
            ("obfuscated_text", "ALTER TABLE entries ADD COLUMN obfuscated_text TEXT"),
            ("test_user_id", "ALTER TABLE entries ADD COLUMN test_user_id INTEGER"),
            ("pii_kinds", "ALTER TABLE entries ADD COLUMN pii_kinds TEXT"),
        ] {
            if !existing_cols.iter().any(|c| c == col) {
                conn.execute_batch(ddl)?;
            }
        }
        Ok(())
    }

    /// Insert a new clipboard entry, or — if the content_hash already exists —
    /// bump its captured_at + source_process and return the existing id.
    ///
    /// On the Touched path we `COALESCE` the obfuscation fields so a re-copy
    /// of the same text doesn't churn substitution (and doesn't reassign the
    /// row's sticky test user, which would make the same record paste a
    /// different fake identity each time).
    pub fn insert_or_touch(&self, entry: NewClipboardEntry) -> Result<InsertOutcome, String> {
        let mut conn = self.conn.lock().map_err(|e| format!("clipboard lock poisoned: {e}"))?;
        let tx = conn.transaction().map_err(map_db)?;
        let existing: Option<i64> = tx
            .query_row(
                "SELECT id FROM entries WHERE content_hash = ?1",
                params![entry.content_hash],
                |r| r.get(0),
            )
            .optional()
            .map_err(map_db)?;
        let now = epoch_ms_now();
        let pii_kinds_csv = if entry.pii_kinds.is_empty() {
            None
        } else {
            Some(entry.pii_kinds.join(","))
        };
        let outcome = if let Some(id) = existing {
            tx.execute(
                "UPDATE entries
                    SET captured_at = ?1,
                        source_process = COALESCE(?2, source_process),
                        sensitive = ?3,
                        obfuscated_text = COALESCE(obfuscated_text, ?4),
                        test_user_id    = COALESCE(test_user_id, ?5),
                        pii_kinds       = COALESCE(pii_kinds, ?6)
                  WHERE id = ?7",
                params![
                    now,
                    entry.source_process,
                    entry.sensitive as i64,
                    entry.obfuscated_text,
                    entry.test_user_id,
                    pii_kinds_csv,
                    id,
                ],
            )
            .map_err(map_db)?;
            InsertOutcome::Touched(id)
        } else {
            tx.execute(
                "INSERT INTO entries
                    (kind, text_content, html_content, image_png, thumb_png,
                     width, height, byte_size, sensitive, obfuscated_text,
                     test_user_id, pii_kinds, source_process,
                     captured_at, pinned, content_hash)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, 0, ?15)",
                params![
                    entry.kind.as_str(),
                    entry.text_content,
                    entry.html_content,
                    entry.image_png,
                    entry.thumb_png,
                    entry.width,
                    entry.height,
                    entry.byte_size,
                    entry.sensitive as i64,
                    entry.obfuscated_text,
                    entry.test_user_id,
                    pii_kinds_csv,
                    entry.source_process,
                    now,
                    entry.content_hash,
                ],
            )
            .map_err(map_db)?;
            let id = tx.last_insert_rowid();
            InsertOutcome::Inserted(id)
        };
        tx.commit().map_err(map_db)?;
        Ok(outcome)
    }

    /// List clipboard entries. When `query` is provided, results are ranked
    /// by fuzzy match score (skim/fzf-style scoring) so subsequence and typo
    /// matches surface; pinned entries still float to the top. When `query`
    /// is absent, ordering is plain pinned-first + captured_at DESC.
    ///
    /// To keep fuzzy ranking responsive at any history size, the candidate
    /// pool for fuzzy mode is capped at `FUZZY_CANDIDATE_POOL` most-recent
    /// rows — far beyond any practical visible-page count.
    pub fn list(
        &self,
        query: Option<&str>,
        kind_filter: Option<&str>,
        pinned_only: bool,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<ClipboardEntrySummary>, String> {
        const FUZZY_CANDIDATE_POOL: u32 = 5_000;
        let q = query.map(str::trim).filter(|s| !s.is_empty());
        let conn = self.conn.lock().map_err(|e| format!("clipboard lock poisoned: {e}"))?;

        let mut sql = String::from(
            "SELECT id, kind, text_content, thumb_png, width, height, byte_size,
                    sensitive, source_process, captured_at, pinned,
                    obfuscated_text, pii_kinds
               FROM entries WHERE 1=1",
        );
        if kind_filter.is_some() {
            sql.push_str(" AND kind = ?");
        }
        if pinned_only {
            sql.push_str(" AND pinned = 1");
        }
        sql.push_str(" ORDER BY pinned DESC, captured_at DESC LIMIT ?");
        // For non-fuzzy queries we paginate in SQL; for fuzzy we pull the
        // candidate pool and paginate after ranking.
        let row_cap: i64 = if q.is_some() {
            FUZZY_CANDIDATE_POOL as i64
        } else {
            sql.push_str(" OFFSET ?");
            limit as i64
        };

        let mut bind_params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        if let Some(k) = kind_filter {
            bind_params.push(Box::new(k.to_string()));
        }
        bind_params.push(Box::new(row_cap));
        if q.is_none() {
            bind_params.push(Box::new(offset as i64));
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db)?;
        let rows: Vec<ClipboardEntrySummary> = stmt
            .query_map(
                rusqlite::params_from_iter(bind_params.iter().map(|b| b.as_ref())),
                row_to_summary,
            )
            .map_err(map_db)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db)?;
        drop(stmt);
        drop(conn);

        let Some(needle) = q else {
            return Ok(rows);
        };

        // Fuzzy-rank the candidate pool. Image-only entries with no text
        // can't match a non-empty query, so they fall out.
        use fuzzy_matcher::skim::SkimMatcherV2;
        use fuzzy_matcher::FuzzyMatcher;
        let matcher = SkimMatcherV2::default();
        let mut scored: Vec<(i64, ClipboardEntrySummary)> = rows
            .into_iter()
            .filter_map(|row| {
                let haystack = row.text_preview.as_deref().unwrap_or("");
                if haystack.is_empty() {
                    return None;
                }
                matcher
                    .fuzzy_match(haystack, needle)
                    .map(|score| (score, row))
            })
            .collect();
        // Pinned first, then score desc, then recency desc as tiebreaker.
        scored.sort_by(|a, b| {
            b.1.pinned
                .cmp(&a.1.pinned)
                .then_with(|| b.0.cmp(&a.0))
                .then_with(|| b.1.captured_at.cmp(&a.1.captured_at))
        });
        let start = offset as usize;
        let end = (start + limit as usize).min(scored.len());
        if start >= scored.len() {
            return Ok(Vec::new());
        }
        Ok(scored[start..end].iter().map(|(_, r)| r.clone()).collect())
    }

    /// Max captured_at across all entries; used by the UI to poll for new
    /// captures inserted by the daemon. Returns 0 when the table is empty.
    pub fn max_captured_at(&self) -> Result<i64, String> {
        let conn = self.conn.lock().map_err(|e| format!("clipboard lock poisoned: {e}"))?;
        let value: Option<i64> = conn
            .query_row(
                "SELECT MAX(captured_at) FROM entries",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(map_db)?;
        Ok(value.unwrap_or(0))
    }

    pub fn get(&self, id: i64) -> Result<Option<ClipboardEntryFull>, String> {
        let conn = self.conn.lock().map_err(|e| format!("clipboard lock poisoned: {e}"))?;
        let row = conn
            .query_row(
                "SELECT id, kind, text_content, html_content, image_png, width, height,
                        byte_size, sensitive, source_process, captured_at, pinned, content_hash,
                        obfuscated_text, test_user_id, pii_kinds
                   FROM entries WHERE id = ?1",
                params![id],
                |r| {
                    let kind: String = r.get(1)?;
                    let image_bytes: Option<Vec<u8>> = r.get(4)?;
                    let image_base64 = image_bytes.map(|b| {
                        use base64::Engine;
                        base64::engine::general_purpose::STANDARD.encode(b)
                    });
                    let sensitive: i64 = r.get(8)?;
                    let pinned: i64 = r.get(11)?;
                    let pii_kinds_raw: Option<String> = r.get(15)?;
                    Ok(ClipboardEntryFull {
                        id: r.get(0)?,
                        kind,
                        text_content: r.get(2)?,
                        html_content: r.get(3)?,
                        image_base64,
                        width: r.get::<_, Option<i64>>(5)?.map(|n| n as u32),
                        height: r.get::<_, Option<i64>>(6)?.map(|n| n as u32),
                        byte_size: r.get(7)?,
                        sensitive: sensitive != 0,
                        obfuscated_text: r.get(13)?,
                        test_user_id: r.get(14)?,
                        pii_kinds: split_kinds(pii_kinds_raw),
                        source_process: r.get(9)?,
                        captured_at: r.get(10)?,
                        pinned: pinned != 0,
                        content_hash: r.get(12)?,
                    })
                },
            )
            .optional()
            .map_err(map_db)?;
        Ok(row)
    }

    pub fn delete(&self, id: i64) -> Result<bool, String> {
        let conn = self.conn.lock().map_err(|e| format!("clipboard lock poisoned: {e}"))?;
        let n = conn
            .execute("DELETE FROM entries WHERE id = ?1", params![id])
            .map_err(map_db)?;
        Ok(n > 0)
    }

    pub fn set_pinned(&self, id: i64, pinned: bool) -> Result<bool, String> {
        let conn = self.conn.lock().map_err(|e| format!("clipboard lock poisoned: {e}"))?;
        let n = conn
            .execute(
                "UPDATE entries SET pinned = ?1 WHERE id = ?2",
                params![pinned as i64, id],
            )
            .map_err(map_db)?;
        Ok(n > 0)
    }

    pub fn clear(&self, include_pinned: bool) -> Result<usize, String> {
        let conn = self.conn.lock().map_err(|e| format!("clipboard lock poisoned: {e}"))?;
        let n = if include_pinned {
            conn.execute("DELETE FROM entries", [])
        } else {
            conn.execute("DELETE FROM entries WHERE pinned = 0", [])
        }
        .map_err(map_db)?;
        Ok(n)
    }

    /// Drop non-pinned rows above the per-kind cap. Called after each insert.
    pub fn prune(&self, settings: &ClipboardSettings) -> Result<usize, String> {
        let conn = self.conn.lock().map_err(|e| format!("clipboard lock poisoned: {e}"))?;
        let mut total = 0usize;
        for (kind, cap) in [
            ("text", settings.text_cap),
            ("html", settings.text_cap),
            ("image", settings.image_cap),
        ] {
            // Keep the newest `cap` non-pinned rows; delete the rest.
            let n = conn
                .execute(
                    "DELETE FROM entries
                      WHERE kind = ?1
                        AND pinned = 0
                        AND id NOT IN (
                            SELECT id FROM entries
                             WHERE kind = ?1 AND pinned = 0
                             ORDER BY captured_at DESC
                             LIMIT ?2
                        )",
                    params![kind, cap as i64],
                )
                .map_err(map_db)?;
            total += n;
        }
        Ok(total)
    }

    pub fn get_settings(&self) -> Result<ClipboardSettings, String> {
        let conn = self.conn.lock().map_err(|e| format!("clipboard lock poisoned: {e}"))?;
        let blob: Option<String> = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'config'",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(map_db)?;
        let cfg = blob
            .and_then(|s| serde_json::from_str::<ClipboardSettings>(&s).ok())
            .unwrap_or_default();
        Ok(cfg)
    }

    pub fn set_settings(&self, settings: &ClipboardSettings) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("clipboard lock poisoned: {e}"))?;
        let json = serde_json::to_string(settings)
            .map_err(|e| format!("failed to serialize settings: {e}"))?;
        conn.execute(
            "INSERT INTO settings(key, value) VALUES('config', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![json],
        )
        .map_err(map_db)?;
        Ok(())
    }
}

fn map_db(e: rusqlite::Error) -> String {
    format!("clipboard DB error: {e}")
}

/// Shared row → summary mapper used by `list`. The column order must match
/// the SELECT in that query.
///
/// For sensitive rows, `text_preview` is sourced from `obfuscated_text` when
/// available so the list view never exposes the original. Falls through to the
/// original only when no obfuscated form exists (e.g. an image flagged by an
/// OS-marker but never PII-scanned).
fn row_to_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<ClipboardEntrySummary> {
    let kind: String = row.get(1)?;
    let text: Option<String> = row.get(2)?;
    let thumb_bytes: Option<Vec<u8>> = row.get(3)?;
    let thumb_base64 = thumb_bytes.map(|b| {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(b)
    });
    let sensitive_raw: i64 = row.get(7)?;
    let sensitive = sensitive_raw != 0;
    let pinned: i64 = row.get(10)?;
    let obfuscated: Option<String> = row.get(11)?;
    let pii_kinds_raw: Option<String> = row.get(12)?;
    let preview_src = if sensitive {
        obfuscated.as_deref().or(text.as_deref())
    } else {
        text.as_deref()
    };
    Ok(ClipboardEntrySummary {
        id: row.get(0)?,
        kind,
        text_preview: preview_src.map(|t| preview(t, 240)),
        thumb_base64,
        width: row.get::<_, Option<i64>>(4)?.map(|n| n as u32),
        height: row.get::<_, Option<i64>>(5)?.map(|n| n as u32),
        byte_size: row.get(6)?,
        sensitive,
        pii_kinds: split_kinds(pii_kinds_raw),
        source_process: row.get(8)?,
        captured_at: row.get(9)?,
        pinned: pinned != 0,
    })
}

fn split_kinds(raw: Option<String>) -> Vec<String> {
    raw.map(|s| {
        s.split(',')
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect()
    })
    .unwrap_or_default()
}

fn preview(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let cut: String = trimmed.chars().take(max_chars).collect();
    format!("{cut}…")
}

fn resolve_db_path() -> PathBuf {
    // SQLite + SMB (\\wsl.localhost\...) is unreliable — locks aren't honored
    // and migrations fail with "database is locked". Keep the DB on the
    // native Windows volume; the JSON registries in `owned_sessions.rs` and
    // `projects.rs` can live in WSL because they don't rely on file locking.
    if let Some(data) = dirs::data_local_dir() {
        return data.join("fnba-utils").join("clipboard.db");
    }
    if let Some(data) = dirs::data_dir() {
        return data.join("fnba-utils").join("clipboard.db");
    }
    PathBuf::from("clipboard.db")
}
