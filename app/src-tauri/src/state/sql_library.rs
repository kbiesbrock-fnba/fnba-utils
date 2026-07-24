//! Config for the filesystem-backed SQL query library.
//!
//! Persistence: `%LOCALAPPDATA%\fnba-utils\sql-library.json`:
//! `{ "root": string|null, "exportedAt": number|null }`.
//!
//! `root` is a Windows-reachable path (drive path or `\\wsl$\…` / `\\wsl.localhost\…`
//! UNC) that `std::fs` operates on directly. `exportedAt` is a millisecond epoch
//! stamp set the FIRST time any root is configured, marking that the one-time
//! export of the legacy saved-query store has run. It is never cleared, so
//! changing the root later never re-exports (which would duplicate files).
//!
//! The config file lives on local disk (`%LOCALAPPDATA%`), so the temp-write +
//! rename atomic-persist is reliable here — unlike the library root itself, which
//! may be a slow/unreliable 9p share.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Clone, Serialize, Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SqlLibraryConfig {
    /// Windows-reachable root directory, or `None` if not yet configured.
    #[serde(default)]
    pub root: Option<String>,
    /// Epoch-ms stamp of the one-time legacy export, or `None` if it never ran.
    #[serde(default)]
    pub exported_at: Option<i64>,
}

pub struct SqlLibraryState {
    inner: Mutex<SqlLibraryConfig>,
    store_path: PathBuf,
}

impl SqlLibraryState {
    pub fn load() -> Self {
        let store_path = resolve_store_path();
        let inner = std::fs::read_to_string(&store_path)
            .ok()
            .and_then(|s| serde_json::from_str::<SqlLibraryConfig>(&s).ok())
            .unwrap_or_default();
        Self {
            inner: Mutex::new(inner),
            store_path,
        }
    }

    /// Snapshot of the current config.
    pub fn get(&self) -> SqlLibraryConfig {
        self.inner
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    /// The configured root, if any.
    pub fn root(&self) -> Option<String> {
        self.inner.lock().ok().and_then(|g| g.root.clone())
    }

    /// Whether the one-time export has already been stamped.
    pub fn has_exported(&self) -> bool {
        self.inner
            .lock()
            .map(|g| g.exported_at.is_some())
            .unwrap_or(true)
    }

    /// Set (or change) the root and persist. Leaves `exported_at` untouched.
    pub fn set_root(&self, root: String) -> Result<SqlLibraryConfig, String> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| format!("SqlLibraryState lock poisoned: {e}"))?;
        guard.root = Some(root);
        self.persist_locked(&guard)?;
        Ok(guard.clone())
    }

    /// Stamp `exported_at` (the one-time export ran) and persist. Idempotent —
    /// a prior stamp is preserved so this never overwrites the original time.
    pub fn mark_exported(&self, ts: i64) -> Result<SqlLibraryConfig, String> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| format!("SqlLibraryState lock poisoned: {e}"))?;
        if guard.exported_at.is_none() {
            guard.exported_at = Some(ts);
            self.persist_locked(&guard)?;
        }
        Ok(guard.clone())
    }

    fn persist_locked(
        &self,
        guard: &std::sync::MutexGuard<'_, SqlLibraryConfig>,
    ) -> Result<(), String> {
        if let Some(parent) = self.store_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                return Err(format!("Failed to create state dir {}: {e}", parent.display()));
            }
        }
        let json = serde_json::to_string_pretty(&**guard)
            .map_err(|e| format!("Failed to serialize sql-library config: {e}"))?;
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
