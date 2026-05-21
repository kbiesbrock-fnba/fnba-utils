//! Batched tmux session probe shared across Mission Control.
//!
//! Two `wsl.exe -e tmux` calls per refresh — one for `list-sessions`, one for
//! `list-panes -a` — merged into per-session info that includes the active
//! pane's current command and path. A short TTL cache keeps the 3-second MC
//! poll from forking tmux twice every tick.

use serde::Serialize;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// One tmux session as seen by `tmux list-sessions` + `list-panes -a`.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TmuxSessionInfo {
    pub name: String,
    /// Unix epoch seconds; 0 if tmux didn't supply it.
    pub created_at: i64,
    /// True if any client is currently attached.
    pub attached: bool,
    /// Number of windows in the session.
    pub window_count: u32,
    /// `pane_current_command` for the active pane (e.g. "bash", "claude",
    /// "vim", "node"). None if no pane data was returned for this session.
    pub current_command: Option<String>,
    /// `pane_current_path` for the active pane.
    pub current_path: Option<String>,
    /// Active pane's process id (the foreground process tmux sees).
    pub pane_pid: Option<i32>,
}

type TmuxCache = OnceLock<Mutex<Option<(Vec<TmuxSessionInfo>, Instant)>>>;
static CACHE: TmuxCache = OnceLock::new();

fn cache() -> &'static Mutex<Option<(Vec<TmuxSessionInfo>, Instant)>> {
    CACHE.get_or_init(|| Mutex::new(None))
}

/// Probe all tmux sessions on the host. Cached with a 2-second TTL so the MC
/// 3s poll doesn't fork+exec twice every tick. Returns empty Vec when tmux
/// has no server running (exit 1) or isn't installed.
pub fn list_all_tmux_sessions() -> Vec<TmuxSessionInfo> {
    const TTL: Duration = Duration::from_millis(2000);
    if let Ok(guard) = cache().lock() {
        if let Some((ref sessions, when)) = *guard {
            if when.elapsed() < TTL {
                return sessions.clone();
            }
        }
    }
    let fresh = fetch_all_tmux_sessions();
    if let Ok(mut guard) = cache().lock() {
        *guard = Some((fresh.clone(), Instant::now()));
    }
    fresh
}

/// Drop the cached tmux snapshot so the next `list_all_tmux_sessions()` call
/// forks `wsl.exe tmux` fresh. Used by the manual Mission Control refresh.
pub fn invalidate_cache() {
    if let Ok(mut guard) = cache().lock() {
        *guard = None;
    }
}

fn fetch_all_tmux_sessions() -> Vec<TmuxSessionInfo> {
    let mut sessions = match run_list_sessions() {
        Some(s) => s,
        None => return Vec::new(),
    };
    if sessions.is_empty() {
        return sessions;
    }
    let panes = run_list_panes();
    for s in sessions.iter_mut() {
        if let Some(p) = panes.get(&s.name) {
            s.current_command = Some(p.command.clone());
            s.current_path = Some(p.path.clone());
            s.pane_pid = p.pid;
        }
    }
    sessions
}

fn run_list_sessions() -> Option<Vec<TmuxSessionInfo>> {
    // Format fields are tab-separated to survive directory names with spaces.
    let output = std::process::Command::new("wsl.exe")
        .args([
            "-e",
            "tmux",
            "list-sessions",
            "-F",
            "#{session_name}\t#{session_created}\t#{session_attached}\t#{session_windows}",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        // exit 1 = "no server running", which is the empty case, not an error.
        return Some(Vec::new());
    }
    let mut out = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let mut parts = line.split('\t');
        let name = parts.next()?.trim().to_string();
        if name.is_empty() {
            continue;
        }
        let created_at = parts
            .next()
            .and_then(|s| s.trim().parse::<i64>().ok())
            .unwrap_or(0);
        let attached_raw = parts.next().unwrap_or("0").trim();
        let attached = attached_raw.parse::<i32>().map(|n| n > 0).unwrap_or(false);
        let window_count = parts
            .next()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(0);
        out.push(TmuxSessionInfo {
            name,
            created_at,
            attached,
            window_count,
            current_command: None,
            current_path: None,
            pane_pid: None,
        });
    }
    Some(out)
}

struct PaneInfo {
    command: String,
    path: String,
    pid: Option<i32>,
}

fn run_list_panes() -> HashMap<String, PaneInfo> {
    let output = std::process::Command::new("wsl.exe")
        .args([
            "-e",
            "tmux",
            "list-panes",
            "-a",
            "-F",
            "#{session_name}\t#{?pane_active,1,0}\t#{pane_current_command}\t#{pane_current_path}\t#{pane_pid}",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();
    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return HashMap::new(),
    };
    let mut map: HashMap<String, PaneInfo> = HashMap::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let mut parts = line.split('\t');
        let name = match parts.next() {
            Some(n) if !n.trim().is_empty() => n.trim().to_string(),
            _ => continue,
        };
        let active = parts.next().unwrap_or("0").trim() == "1";
        let command = parts.next().unwrap_or("").trim().to_string();
        let path = parts.next().unwrap_or("").trim().to_string();
        let pid = parts.next().and_then(|s| s.trim().parse::<i32>().ok());
        // Only keep the active pane per session; if list-panes returns multiple
        // active flags (e.g. one per window), the last one wins which is fine.
        if active || !map.contains_key(&name) {
            map.insert(
                name,
                PaneInfo {
                    command,
                    path,
                    pid,
                },
            );
        }
    }
    map
}

/// Best-effort follow-up probe: for panes whose `pane_current_command` is a
/// generic interpreter (`node`, `python`, `bash`), inspect the full argv via
/// `ps -o args= -p <pid>` to detect a Claude CLI invocation. Called only for
/// the small set of candidate pids — we don't ps every pane.
pub fn ps_contains_claude(pid: i32) -> bool {
    let output = std::process::Command::new("wsl.exe")
        .args(["-e", "ps", "-o", "args=", "-p", &pid.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();
    let Ok(output) = output else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let line = String::from_utf8_lossy(&output.stdout);
    line.contains("claude")
}
