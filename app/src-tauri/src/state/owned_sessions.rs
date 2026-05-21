//! Persistent registry of Claude sessions launched by Mission Control.
//!
//! Mission Control now exclusively tracks sessions it spawned itself; external
//! claude processes (IntelliJ plugin, plain WSL terminals) are not shown. This
//! state is the source of truth for "what sessions exist."
//!
//! Persistence: `<wsl_home>/.claude/fnba-mc/owned-sessions.json` if available,
//! otherwise the native Windows `dirs::data_dir()`. Survives Tauri app
//! restart; sessions whose PIDs are no longer alive are dropped on load.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

fn epoch_ms_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() * 1000)
        .unwrap_or(0)
}

/// On-disk + in-memory representation of one MC-launched Claude session.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnedSession {
    pub session_id: String,
    pub cwd: String,
    pub pid: u32,
    pub started_at: u64,
    /// User-assigned friendly label (Feature #20).
    #[serde(default)]
    pub label: Option<String>,
    /// Resolved Claude home (e.g. `\\wsl.localhost\Ubuntu\home\<u>\.claude` or native).
    /// Stored so JSONL/projects paths can be resolved without re-scanning.
    pub claude_home: String,
    /// If this session was launched into a git worktree, the worktree path on disk.
    /// Used for cleanup on stop.
    #[serde(default)]
    pub worktree_path: Option<String>,
    /// Tmux session name (`claude-<uuid>`); cached so kill-session doesn't need to re-derive.
    pub tmux_session: String,
    /// Unix epoch ms when the session was observed to die. Only set on
    /// entries that have been moved to `history`.
    #[serde(default)]
    pub ended_at: Option<u64>,
}

/// Max retained dead sessions. Older entries are evicted from the back.
pub const HISTORY_CAP: usize = 200;

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OnDisk {
    #[serde(default)]
    entries: HashMap<String, OwnedSession>,
    /// Ordered newest-first.
    #[serde(default)]
    history: Vec<OwnedSession>,
}

pub struct OwnedSessionsState {
    inner: Mutex<OnDisk>,
    store_path: PathBuf,
}

impl OwnedSessionsState {
    /// Load state from disk. Falls back to empty on any parse/IO failure
    /// (never panics — corrupted state must not crash startup).
    pub fn load() -> Self {
        let store_path = resolve_store_path();
        let inner = match std::fs::read_to_string(&store_path) {
            Ok(s) => serde_json::from_str::<OnDisk>(&s).unwrap_or_default(),
            Err(_) => OnDisk::default(),
        };
        // Move dead entries to history on load. Tmux is the liveness source
        // of truth: our captured PID is the bash shell hosting `tmux attach`
        // which dies when the panel closes; the tmux session keeps running.
        let live_tmux = list_live_tmux_sessions();
        let mut pruned = OnDisk {
            entries: HashMap::new(),
            history: inner.history,
        };
        let now = epoch_ms_now();
        for (sid, mut session) in inner.entries {
            if live_tmux.contains(&session.tmux_session) {
                pruned.entries.insert(sid, session);
            } else {
                session.ended_at = Some(now);
                pruned.history.insert(0, session);
            }
        }
        if pruned.history.len() > HISTORY_CAP {
            pruned.history.truncate(HISTORY_CAP);
        }
        let state = Self {
            inner: Mutex::new(pruned),
            store_path,
        };
        // Persist the pruned set so the file reflects current reality.
        let _ = state.persist_locked(&state.inner.lock().expect("fresh mutex"));
        state
    }

    /// Insert (or replace) a session and persist.
    pub fn insert(&self, session: OwnedSession) -> Result<(), String> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| format!("OwnedSessionsState lock poisoned: {e}"))?;
        guard.entries.insert(session.session_id.clone(), session);
        self.persist_locked(&guard)
    }

    /// Remove a session by id and persist. Returns the removed entry, if any.
    pub fn remove(&self, session_id: &str) -> Result<Option<OwnedSession>, String> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| format!("OwnedSessionsState lock poisoned: {e}"))?;
        let removed = guard.entries.remove(session_id);
        self.persist_locked(&guard)?;
        Ok(removed)
    }

    /// Read-only snapshot of one entry.
    pub fn get(&self, session_id: &str) -> Option<OwnedSession> {
        self.inner.lock().ok()?.entries.get(session_id).cloned()
    }

    /// Set or clear the label for a session. Returns true if the session was found.
    pub fn set_label(&self, session_id: &str, label: Option<String>) -> Result<bool, String> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| format!("OwnedSessionsState lock poisoned: {e}"))?;
        let found = if let Some(entry) = guard.entries.get_mut(session_id) {
            entry.label = label;
            true
        } else {
            false
        };
        if found {
            self.persist_locked(&guard)?;
        }
        Ok(found)
    }

    /// Snapshot of all entries whose tmux sessions are still alive. Dead
    /// entries move to `history` (newest-first) so the user can find and
    /// resume them later.
    pub fn list_alive(&self) -> Vec<OwnedSession> {
        let live_tmux = list_live_tmux_sessions();
        let mut guard = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => return Vec::new(),
        };
        let now = epoch_ms_now();
        let mut archived = 0usize;
        // drain_filter isn't stable yet; collect dead keys then mutate.
        let dead_ids: Vec<String> = guard
            .entries
            .iter()
            .filter(|(_, s)| !live_tmux.contains(&s.tmux_session))
            .map(|(k, _)| k.clone())
            .collect();
        for sid in dead_ids {
            if let Some(mut dead) = guard.entries.remove(&sid) {
                dead.ended_at = Some(now);
                guard.history.insert(0, dead);
                archived += 1;
            }
        }
        if guard.history.len() > HISTORY_CAP {
            guard.history.truncate(HISTORY_CAP);
        }
        if archived > 0 {
            let _ = self.persist_locked(&guard);
        }
        guard.entries.values().cloned().collect()
    }

    /// Snapshot of historical (dead) sessions, newest-first, bounded by `limit`.
    pub fn list_history(&self, limit: usize) -> Vec<OwnedSession> {
        let guard = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => return Vec::new(),
        };
        guard.history.iter().take(limit).cloned().collect()
    }

    /// Remove a session from history by id. Used by the resume flow to
    /// reclaim an entry before inserting a fresh live one under the same
    /// session_id. Returns the previous historical record if found.
    pub fn pop_history(&self, session_id: &str) -> Result<Option<OwnedSession>, String> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| format!("OwnedSessionsState lock poisoned: {e}"))?;
        let pos = guard.history.iter().position(|s| s.session_id == session_id);
        let popped = pos.map(|i| guard.history.remove(i));
        if popped.is_some() {
            self.persist_locked(&guard)?;
        }
        Ok(popped)
    }

    /// Permanently remove a session from history (the "Forget" action).
    pub fn forget_history(&self, session_id: &str) -> Result<bool, String> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| format!("OwnedSessionsState lock poisoned: {e}"))?;
        let before = guard.history.len();
        guard.history.retain(|s| s.session_id != session_id);
        let removed = guard.history.len() != before;
        if removed {
            self.persist_locked(&guard)?;
        }
        Ok(removed)
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
            .map_err(|e| format!("Failed to serialize owned sessions: {e}"))?;
        std::fs::write(&self.store_path, json)
            .map_err(|e| format!("Failed to write {}: {e}", self.store_path.display()))
    }
}

fn resolve_store_path() -> PathBuf {
    // Prefer WSL's user home — that's where claude actually lives, and keeping
    // the state file next to `~/.claude/` makes it easy to inspect / back up.
    // Multi-user hosts are uncommon for this app; pick the first home we find.
    let wsl_root = PathBuf::from(r"\\wsl.localhost\Ubuntu\home");
    if let Ok(entries) = std::fs::read_dir(&wsl_root) {
        for entry in entries.flatten() {
            let candidate = entry.path().join(".claude").join("fnba-mc").join("owned-sessions.json");
            // Use this even if it doesn't exist yet — we'll create it on first persist.
            if entry.path().join(".claude").is_dir() {
                return candidate;
            }
        }
    }
    // Fallback: native Windows app-data.
    if let Some(data) = dirs::data_dir() {
        return data.join("fnba-utils").join("owned-sessions.json");
    }
    PathBuf::from("owned-sessions.json")
}

/// Snapshot of currently-running tmux session names, served from the shared
/// `tmux_sessions` cache. Tmux is the liveness source of truth: our captured
/// PID is the bash shell hosting `tmux attach` and dies when the panel
/// closes, but the tmux session keeps running.
///
/// Sharing the cache means `get_claude_sessions` in MC pays for the underlying
/// probe at most once per refresh (whichever caller arrives first primes the
/// cache; the other reads it).
fn list_live_tmux_sessions() -> HashSet<String> {
    super::tmux_sessions::list_live_session_names()
}

