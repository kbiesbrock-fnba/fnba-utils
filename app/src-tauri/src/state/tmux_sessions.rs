//! Batched tmux session probe shared across Mission Control.
//!
//! All tmux/ps state is gathered in a single round-trip to the persistent
//! [`wsl_helper`](super::wsl_helper) bash subprocess: one script runs
//! `tmux list-sessions`, `tmux list-panes -a`, and a single `ps` over every
//! pane pid. That collapses what used to be 2–N separate `wsl.exe` cold
//! starts into one inside-WSL command sequence. A short TTL cache absorbs the
//! 3-second Mission Control poll.

use super::wsl_helper;
use serde::Serialize;
use std::collections::HashMap;
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
    /// Full `ps -o args=` line for the pane pid, used to detect claude wrapped
    /// in `node`/`bash`/`python`. None if the ps lookup failed.
    pub pane_args: Option<String>,
}

type TmuxCache = OnceLock<Mutex<Option<(Vec<TmuxSessionInfo>, Instant)>>>;
static CACHE: TmuxCache = OnceLock::new();

fn cache() -> &'static Mutex<Option<(Vec<TmuxSessionInfo>, Instant)>> {
    CACHE.get_or_init(|| Mutex::new(None))
}

/// Probe all tmux sessions on the host. Cached with a 2-second TTL so the MC
/// 3s poll doesn't re-run the script every tick. Returns empty Vec when tmux
/// has no server running or isn't installed.
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

/// Drop the cached tmux snapshot so the next [`list_all_tmux_sessions`] call
/// re-runs the helper script. Used by the manual Mission Control refresh.
pub fn invalidate_cache() {
    if let Ok(mut guard) = cache().lock() {
        *guard = None;
    }
}

/// Backwards-compatible helper for callers that only need the set of live
/// tmux session names (e.g. `OwnedSessions::list_alive`). Shares the same
/// cache as [`list_all_tmux_sessions`], so two callers in the same MC tick
/// pay for only one probe.
pub fn list_live_session_names() -> std::collections::HashSet<String> {
    list_all_tmux_sessions()
        .into_iter()
        .map(|s| s.name)
        .collect()
}

// Section delimiters in the helper script output. Picked so they cannot
// collide with tmux session names (no '#' allowed in session names) or ps
// argv lines.
const SECTION_SESSIONS: &str = "###FNBA_SESSIONS###";
const SECTION_PANES: &str = "###FNBA_PANES###";
const SECTION_PS: &str = "###FNBA_PS###";

fn fetch_all_tmux_sessions() -> Vec<TmuxSessionInfo> {
    // One bash script, three sections. We capture panes into a variable so we
    // can derive the ps pid list without re-running tmux. `|| true` keeps the
    // pipeline going when tmux has no server (exit 1) so the section markers
    // still get printed and the parser stays simple.
    let script = format!(
        "echo '{ss}'
tmux list-sessions -F '#{{session_name}}\t#{{session_created}}\t#{{session_attached}}\t#{{session_windows}}' 2>/dev/null || true
echo '{sp}'
_PANES=$(tmux list-panes -a -F '#{{session_name}}\t#{{?pane_active,1,0}}\t#{{pane_current_command}}\t#{{pane_current_path}}\t#{{pane_pid}}' 2>/dev/null || true)
printf '%s\n' \"$_PANES\"
echo '{sx}'
_PIDS=$(printf '%s\n' \"$_PANES\" | awk -F '\t' 'NF>=5 && $5 ~ /^[0-9]+$/ {{print $5}}' | sort -u | tr '\n' ' ')
if [ -n \"$_PIDS\" ]; then ps -o pid=,args= -p $_PIDS 2>/dev/null || true; fi
",
        ss = SECTION_SESSIONS,
        sp = SECTION_PANES,
        sx = SECTION_PS,
    );

    let output = match wsl_helper::run_script(&script) {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    let (sessions_block, panes_block, ps_block) = match split_sections(&output) {
        Some(parts) => parts,
        None => return Vec::new(),
    };

    let ps_by_pid = parse_ps(ps_block);
    let panes = parse_panes(panes_block, &ps_by_pid);
    let mut sessions = parse_sessions(sessions_block);
    for s in sessions.iter_mut() {
        if let Some(p) = panes.get(&s.name) {
            s.current_command = Some(p.command.clone());
            s.current_path = Some(p.path.clone());
            s.pane_pid = p.pid;
            s.pane_args = p.args.clone();
        }
    }
    sessions
}

fn split_sections(output: &str) -> Option<(&str, &str, &str)> {
    // Find the three markers in order; everything between them is the section
    // body. Anything before the first marker (e.g. shell init noise on first
    // call) is discarded.
    let s1 = output.find(SECTION_SESSIONS)? + SECTION_SESSIONS.len();
    let s2 = output[s1..].find(SECTION_PANES)?;
    let s2_abs = s1 + s2;
    let s3 = output[s2_abs + SECTION_PANES.len()..].find(SECTION_PS)?;
    let s3_abs = s2_abs + SECTION_PANES.len() + s3;
    let sessions_block = output[s1..s2_abs].trim_matches('\n');
    let panes_block = output[s2_abs + SECTION_PANES.len()..s3_abs].trim_matches('\n');
    let ps_block = output[s3_abs + SECTION_PS.len()..].trim_matches('\n');
    Some((sessions_block, panes_block, ps_block))
}

fn parse_sessions(block: &str) -> Vec<TmuxSessionInfo> {
    let mut out = Vec::new();
    for line in block.lines() {
        let mut parts = line.split('\t');
        let name = match parts.next() {
            Some(n) if !n.trim().is_empty() => n.trim().to_string(),
            _ => continue,
        };
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
            pane_args: None,
        });
    }
    out
}

struct PaneInfo {
    command: String,
    path: String,
    pid: Option<i32>,
    args: Option<String>,
}

fn parse_panes(block: &str, ps_by_pid: &HashMap<i32, String>) -> HashMap<String, PaneInfo> {
    let mut map: HashMap<String, PaneInfo> = HashMap::new();
    for line in block.lines() {
        let mut parts = line.split('\t');
        let name = match parts.next() {
            Some(n) if !n.trim().is_empty() => n.trim().to_string(),
            _ => continue,
        };
        let active = parts.next().unwrap_or("0").trim() == "1";
        let command = parts.next().unwrap_or("").trim().to_string();
        let path = parts.next().unwrap_or("").trim().to_string();
        let pid = parts.next().and_then(|s| s.trim().parse::<i32>().ok());
        let args = pid.and_then(|p| ps_by_pid.get(&p).cloned());
        // Keep the active pane; otherwise the first one we saw. tmux usually
        // emits the active pane last per session, so a later `active=1`
        // wins.
        if active || !map.contains_key(&name) {
            map.insert(
                name,
                PaneInfo {
                    command,
                    path,
                    pid,
                    args,
                },
            );
        }
    }
    map
}

fn parse_ps(block: &str) -> HashMap<i32, String> {
    let mut out = HashMap::new();
    for line in block.lines() {
        let line = line.trim_start();
        if line.is_empty() {
            continue;
        }
        // `ps -o pid=,args=` formats lines as "  1234 /usr/bin/some args..."
        // (leading whitespace from the pid field width). Split on the first
        // whitespace block.
        let mut split = line.splitn(2, char::is_whitespace);
        let pid = match split.next().and_then(|s| s.trim().parse::<i32>().ok()) {
            Some(p) => p,
            None => continue,
        };
        let args = split.next().unwrap_or("").trim().to_string();
        if !args.is_empty() {
            out.insert(pid, args);
        }
    }
    out
}
