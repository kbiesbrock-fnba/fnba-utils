//! Persistent state for the Docker widget: pinned container names and the
//! widget window's last saved position.
//!
//! Persistence: `%LOCALAPPDATA%\fnba-utils\docker-widget.json`. Follows the
//! same Mutex<OnDisk> + load/persist_locked pattern as `state/projects.rs`.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OnDisk {
    #[serde(default)]
    pinned: Vec<String>, // container names (deduped, order preserved)
    #[serde(default)]
    window_x: Option<i32>,
    #[serde(default)]
    window_y: Option<i32>,
}

pub struct DockerWidgetState {
    inner: Mutex<OnDisk>,
    store_path: PathBuf,
}

impl DockerWidgetState {
    pub fn load() -> Self {
        let store_path = crate::state::paths::data_file("docker-widget.json");
        let inner = std::fs::read_to_string(&store_path)
            .ok()
            .and_then(|s| serde_json::from_str::<OnDisk>(&s).ok())
            .unwrap_or_default();
        let state = Self {
            inner: Mutex::new(inner),
            store_path,
        };
        // Ensure the file is written even on a fresh install so future reads
        // always find a valid JSON document.
        let _ = state.persist_locked(&state.inner.lock().expect("fresh mutex"));
        state
    }

    /// The set of pinned container names (for O(1) membership checks during
    /// `list_containers`).
    pub fn pinned_set(&self) -> HashSet<String> {
        self.inner
            .lock()
            .ok()
            .map(|g| g.pinned.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Ordered list of pinned container names (for the frontend list command).
    pub fn pinned_list(&self) -> Vec<String> {
        self.inner
            .lock()
            .ok()
            .map(|g| g.pinned.clone())
            .unwrap_or_default()
    }

    /// Add `name` to the pinned list (no-op if already present).
    pub fn pin(&self, name: String) -> Result<(), String> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| format!("DockerWidgetState lock poisoned: {e}"))?;
        if !guard.pinned.contains(&name) {
            guard.pinned.push(name);
            self.persist_locked(&guard)?;
        }
        Ok(())
    }

    /// Remove `name` from the pinned list (no-op if not present).
    pub fn unpin(&self, name: &str) -> Result<(), String> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| format!("DockerWidgetState lock poisoned: {e}"))?;
        let before = guard.pinned.len();
        guard.pinned.retain(|n| n != name);
        if guard.pinned.len() != before {
            self.persist_locked(&guard)?;
        }
        Ok(())
    }

    /// Persist a new window position.
    pub fn set_position(&self, x: i32, y: i32) -> Result<(), String> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| format!("DockerWidgetState lock poisoned: {e}"))?;
        guard.window_x = Some(x);
        guard.window_y = Some(y);
        self.persist_locked(&guard)
    }

    /// Return the last saved window position, or `None` if none was saved yet.
    pub fn position(&self) -> Option<(i32, i32)> {
        let guard = self.inner.lock().ok()?;
        match (guard.window_x, guard.window_y) {
            (Some(x), Some(y)) => Some((x, y)),
            _ => None,
        }
    }

    fn persist_locked(
        &self,
        guard: &std::sync::MutexGuard<'_, OnDisk>,
    ) -> Result<(), String> {
        if let Some(parent) = self.store_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                return Err(format!(
                    "Failed to create state dir {}: {e}",
                    parent.display()
                ));
            }
        }
        let json = serde_json::to_string_pretty(&**guard)
            .map_err(|e| format!("Failed to serialize docker-widget state: {e}"))?;
        std::fs::write(&self.store_path, json)
            .map_err(|e| format!("Failed to write {}: {e}", self.store_path.display()))
    }
}
