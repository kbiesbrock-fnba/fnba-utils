//! App STATE for the filesystem-backed SQL query library.
//!
//! Persistence: `%LOCALAPPDATA%\fnba-utils\sql-library.json`. This file holds
//! ONLY `{ "exportedAt": number|null }` — the millisecond-epoch stamp marking
//! that the one-time export of the legacy SQLite saved-query store has run. It
//! is never cleared, so the export never re-runs.
//!
//! The library ROOT is NOT stored here — it lives in the user-edited
//! `config.yaml` (`sql_library.root`) and is re-read on every library call.
//! Older builds wrote a `root` key into this file; serde ignores unknown fields
//! on read, and the next persist drops it.
//!
//! This file lives on local disk (`%LOCALAPPDATA%`), so the temp-write + rename
//! atomic-persist is reliable here.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Clone, Serialize, Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
struct StateData {
    /// Epoch-ms stamp of the one-time legacy export, or `None` if it never ran.
    #[serde(default)]
    exported_at: Option<i64>,
}

pub struct SqlLibraryState {
    inner: Mutex<StateData>,
    store_path: PathBuf,
}

impl SqlLibraryState {
    pub fn load() -> Self {
        let store_path = resolve_store_path();
        let inner = std::fs::read_to_string(&store_path)
            .ok()
            .and_then(|s| serde_json::from_str::<StateData>(&s).ok())
            .unwrap_or_default();
        Self {
            inner: Mutex::new(inner),
            store_path,
        }
    }

    /// The one-time export stamp, if the export has run.
    pub fn exported_at(&self) -> Option<i64> {
        self.inner.lock().ok().and_then(|g| g.exported_at)
    }

    /// Stamp `exported_at` (the one-time export ran) and persist. Idempotent —
    /// a prior stamp is preserved so this never overwrites the original time.
    pub fn mark_exported(&self, ts: i64) -> Result<(), String> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| format!("SqlLibraryState lock poisoned: {e}"))?;
        if guard.exported_at.is_none() {
            guard.exported_at = Some(ts);
            self.persist_locked(&guard)?;
        }
        Ok(())
    }

    fn persist_locked(
        &self,
        guard: &std::sync::MutexGuard<'_, StateData>,
    ) -> Result<(), String> {
        if let Some(parent) = self.store_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                return Err(format!("Failed to create state dir {}: {e}", parent.display()));
            }
        }
        let json = serde_json::to_string_pretty(&**guard)
            .map_err(|e| format!("Failed to serialize sql-library state: {e}"))?;
        // Atomic write: temp file in the same dir, then rename over the target.
        let tmp = self.store_path.with_extension("json.tmp");
        std::fs::write(&tmp, json)
            .map_err(|e| format!("Failed to write {}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, &self.store_path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("Failed to persist {}: {e}", self.store_path.display())
        })
    }
}

fn resolve_store_path() -> PathBuf {
    crate::state::paths::data_file("sql-library.json")
}
