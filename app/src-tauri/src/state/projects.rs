//! Persistent registry of working directories the user launches Claude
//! sessions into. Replaces the localStorage MRU and adds pinned favorites.
//!
//! Persistence: `%LOCALAPPDATA%\fnba-utils\projects.json`. Same store-path
//! strategy as `owned_sessions.rs` so backups can grab both files at once.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    /// Working directory in WSL form (e.g. `/mnt/c/dev/fnba-utils`). Acts as
    /// the primary key — duplicates are not allowed.
    pub cwd: String,
    /// Friendly name shown in the launcher and palette. Defaults to the cwd
    /// basename if the user doesn't override.
    pub display_name: String,
    /// Pinned projects appear above unpinned MRU entries in the launcher.
    #[serde(default)]
    pub pinned: bool,
    /// Unix epoch milliseconds of the most recent launch into this project.
    /// Used to sort the unpinned half of the list.
    #[serde(default)]
    pub last_used_at: u64,
    /// Optional free-form note (planned but not surfaced in UI yet).
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OnDisk {
    #[serde(default)]
    entries: HashMap<String, Project>,
}

pub struct ProjectsState {
    inner: Mutex<OnDisk>,
    store_path: PathBuf,
}

impl ProjectsState {
    pub fn load() -> Self {
        let store_path = resolve_store_path();
        let inner = std::fs::read_to_string(&store_path)
            .ok()
            .and_then(|s| serde_json::from_str::<OnDisk>(&s).ok())
            .unwrap_or_default();
        let state = Self {
            inner: Mutex::new(inner),
            store_path,
        };
        let _ = state.persist_locked(&state.inner.lock().expect("fresh mutex"));
        state
    }

    /// Snapshot of every registered project. Frontend handles sorting (pinned
    /// first, then by `last_used_at` desc).
    pub fn list(&self) -> Vec<Project> {
        self.inner
            .lock()
            .ok()
            .map(|g| g.entries.values().cloned().collect())
            .unwrap_or_default()
    }

    pub fn most_recent_used(&self) -> Option<Project> {
        let guard = self.inner.lock().ok()?;
        guard
            .entries
            .values()
            .max_by_key(|p| p.last_used_at)
            .cloned()
    }

    /// Insert if absent, otherwise update display_name / pinned / notes
    /// (touching last_used_at is `record_used`'s job). Returns true if the
    /// entry was newly created.
    pub fn upsert(
        &self,
        cwd: String,
        display_name: Option<String>,
        pinned: Option<bool>,
        notes: Option<String>,
    ) -> Result<bool, String> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| format!("ProjectsState lock poisoned: {e}"))?;
        let created = !guard.entries.contains_key(&cwd);
        let entry = guard
            .entries
            .entry(cwd.clone())
            .or_insert_with(|| Project {
                cwd: cwd.clone(),
                display_name: default_display_name(&cwd),
                pinned: false,
                last_used_at: 0,
                notes: None,
            });
        if let Some(dn) = display_name {
            if !dn.trim().is_empty() {
                entry.display_name = dn;
            }
        }
        if let Some(p) = pinned {
            entry.pinned = p;
        }
        if let Some(n) = notes {
            entry.notes = if n.trim().is_empty() { None } else { Some(n) };
        }
        self.persist_locked(&guard)?;
        Ok(created)
    }

    pub fn remove(&self, cwd: &str) -> Result<bool, String> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| format!("ProjectsState lock poisoned: {e}"))?;
        let removed = guard.entries.remove(cwd).is_some();
        if removed {
            self.persist_locked(&guard)?;
        }
        Ok(removed)
    }

    /// Touch `last_used_at` for `cwd`, auto-creating the entry if missing.
    /// Called from `start_new_claude_session` after a successful spawn.
    pub fn record_used(&self, cwd: &str) -> Result<(), String> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| format!("ProjectsState lock poisoned: {e}"))?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() * 1000)
            .unwrap_or(0);
        let entry = guard.entries.entry(cwd.to_string()).or_insert_with(|| Project {
            cwd: cwd.to_string(),
            display_name: default_display_name(cwd),
            pinned: false,
            last_used_at: 0,
            notes: None,
        });
        entry.last_used_at = now;
        self.persist_locked(&guard)
    }

    fn persist_locked(
        &self,
        guard: &std::sync::MutexGuard<'_, OnDisk>,
    ) -> Result<(), String> {
        if let Some(parent) = self.store_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                return Err(format!("Failed to create state dir {}: {e}", parent.display()));
            }
        }
        let json = serde_json::to_string_pretty(&**guard)
            .map_err(|e| format!("Failed to serialize projects: {e}"))?;
        std::fs::write(&self.store_path, json)
            .map_err(|e| format!("Failed to write {}: {e}", self.store_path.display()))
    }
}

fn default_display_name(cwd: &str) -> String {
    cwd.rsplit_once('/')
        .map(|(_, tail)| tail.to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| cwd.to_string())
}

fn resolve_store_path() -> PathBuf {
    crate::state::paths::data_file("projects.json")
}
