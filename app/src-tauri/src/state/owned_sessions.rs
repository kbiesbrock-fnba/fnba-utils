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
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

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
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OnDisk {
    #[serde(default)]
    entries: HashMap<String, OwnedSession>,
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
        // Prune dead sessions. We use the *tmux session* as the liveness
        // indicator, not our bash shell's PID: the bash shell exits when the
        // user closes the chat panel, but the tmux session (and claude
        // inside it) keeps running. We resolve liveness via a single
        // `tmux list-sessions` call below.
        let live_tmux = list_live_tmux_sessions();
        let mut pruned = OnDisk::default();
        for (sid, session) in inner.entries {
            if live_tmux.contains(&session.tmux_session) {
                pruned.entries.insert(sid, session);
            }
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
    /// entries are removed from state as a side effect (and persisted).
    pub fn list_alive(&self) -> Vec<OwnedSession> {
        let live_tmux = list_live_tmux_sessions();
        let mut guard = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => return Vec::new(),
        };
        let before = guard.entries.len();
        guard.entries.retain(|_, s| live_tmux.contains(&s.tmux_session));
        if guard.entries.len() != before {
            let _ = self.persist_locked(&guard);
        }
        guard.entries.values().cloned().collect()
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

/// Snapshot of all currently-running tmux sessions visible to `wsl.exe -e tmux
/// list-sessions`, with a short TTL cache so the 3s Mission Control poll
/// doesn't fork+exec a process every tick. Tmux is the liveness source of
/// truth: our captured PID is the bash shell hosting `tmux attach` and dies
/// when the panel closes, but the tmux session keeps running.
fn list_live_tmux_sessions() -> HashSet<String> {
    const TTL: Duration = Duration::from_millis(2000);
    static CACHE: OnceLock<Mutex<Option<(HashSet<String>, Instant)>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = cache.lock() {
        if let Some((ref names, when)) = *guard {
            if when.elapsed() < TTL {
                return names.clone();
            }
        }
    }
    let fresh = fetch_live_tmux_sessions();
    if let Ok(mut guard) = cache.lock() {
        *guard = Some((fresh.clone(), Instant::now()));
    }
    fresh
}

fn fetch_live_tmux_sessions() -> HashSet<String> {
    let output = match std::process::Command::new("wsl.exe")
        .args(["-e", "tmux", "list-sessions", "-F", "#{session_name}"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
    {
        Ok(o) => o,
        Err(_) => return HashSet::new(),
    };
    if !output.status.success() {
        return HashSet::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

