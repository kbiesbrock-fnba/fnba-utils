//! Claude session I/O for Mission Control.
//!
//! Architecture: we spawn `claude --session-id <uuid>` inside a *tmux session*
//! named `claude-<uuid>`, running under our portable_pty. Why tmux?
//!
//!   1. **Multi-client attach.** The user can `tmux attach -t claude-<uuid>`
//!      from any WSL terminal (IntelliJ's embedded terminal, Windows Terminal,
//!      etc.) and see the live TUI. Anything they type there ALSO goes to
//!      claude. The chat panel and the IDE terminal are co-drivers.
//!
//!   2. **Robust send.** Instead of writing bracketed-paste keystrokes to the
//!      PTY (which only one tmux client sees if the user has the focus elsewhere),
//!      we use `tmux load-buffer` + `tmux paste-buffer`, which delivers to
//!      claude regardless of who is attached.
//!
//! Output is rendered by *tailing the session JSONL file*, not by parsing the
//! TUI output stream — JSONL records are already structured and avoid the
//! ANSI-parsing nightmare.

use crate::commands::mission_control::{cwd_to_project_hash, wsl_claude_dirs};
use crate::models::mission_control::{
    ClaudeIoSession, ClaudeIoState, NewSessionInfo, PTY_BUFFER_CAP,
};
use crate::state::owned_sessions::{OwnedSession, OwnedSessionsState};
use std::collections::VecDeque;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use tauri::{Emitter, Manager};

/// Shared type for the per-session PTY ring buffer.
pub(crate) type PtyBuffer = Arc<Mutex<VecDeque<u8>>>;

/// Monotonic counter for ClaudeIoSession.generation. See its docstring for
/// why we need it (disconnect → reattach race).
static NEXT_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn next_generation() -> u64 {
    NEXT_GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// Phase-1 pre-approved tool set. Pre-accepting these means the spawned claude
/// won't block on a permission prompt for the common operations. Phase 2 will
/// swap this for an MCP-based `--permission-prompt-tool` bridge.
const PRE_APPROVED_TOOLS: &str =
    "Read,Edit,Write,Bash,Glob,Grep,WebFetch,WebSearch,TodoWrite,NotebookEdit";

/// Maximum carry-over buffer size for the JSONL tail thread before we drop it.
/// A misbehaving writer that never emits `\n` would otherwise OOM the thread.
const TAIL_CARRY_CAP: usize = 1_048_576; // 1 MiB

/// Outcome of `ensure_workspace_trust`. Surfaced to the frontend via a system
/// `claude-event` with subtype `trust-warning` so the user knows whether the
/// first send will land cleanly or dismiss a trust dialog.
#[derive(serde::Serialize, Clone, Copy, PartialEq, Eq)]
pub enum TrustState {
    /// Trust was already granted for this cwd — nothing to do.
    AlreadyTrusted,
    /// We wrote `hasTrustDialogAccepted: true` for this cwd.
    Granted,
    /// We could not update `.claude.json`. The first send may dismiss a trust
    /// dialog instead of being received; the UI should warn the user.
    WriteFailed,
}

// =============================================================================
// Workspace trust
// =============================================================================

/// Mark `cwd` as trusted in `~/.claude.json` so the spawned claude doesn't show
/// the "Quick safety check: Is this a project you trust?" dialog at startup.
/// Without this, the dialog blocks input — and when `send_claude_message` fires
/// its first paste, the implicit Enter accepts the dialog while the paste
/// content is silently dropped.
pub(crate) fn ensure_workspace_trust(cwd: &str) -> TrustState {
    let wsl_user = cwd
        .strip_prefix("/home/")
        .and_then(|rest| rest.split('/').next())
        .map(str::to_string);

    let mut candidates: Vec<PathBuf> = Vec::new();
    for (claude_dir, _) in wsl_claude_dirs() {
        if let Some(home) = claude_dir.parent() {
            if let Some(ref user) = wsl_user {
                if home.file_name().and_then(|n| n.to_str()) != Some(user.as_str()) {
                    continue;
                }
            }
            candidates.push(home.join(".claude.json"));
        }
    }
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".claude.json"));
    }

    let mut any_attempted = false;
    for path in candidates {
        if !path.exists() {
            continue;
        }
        any_attempted = true;
        let contents = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let mut config: serde_json::Value = match serde_json::from_str(&contents) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let projects = match config.get_mut("projects").and_then(|v| v.as_object_mut()) {
            Some(p) => p,
            None => continue,
        };
        let project = projects
            .entry(cwd.to_string())
            .or_insert_with(|| serde_json::json!({}));
        let already_trusted = project
            .get("hasTrustDialogAccepted")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if already_trusted {
            return TrustState::AlreadyTrusted;
        }
        if let Some(obj) = project.as_object_mut() {
            obj.insert(
                "hasTrustDialogAccepted".to_string(),
                serde_json::Value::Bool(true),
            );
        }
        let new_contents = match serde_json::to_string_pretty(&config) {
            Ok(s) => s,
            Err(_) => return TrustState::WriteFailed,
        };
        return match std::fs::write(&path, new_contents) {
            Ok(()) => TrustState::Granted,
            Err(_) => TrustState::WriteFailed,
        };
    }
    if any_attempted {
        TrustState::WriteFailed
    } else {
        // No .claude.json anywhere — claude will create one with the dialog flag
        // on first run. Treat as "already" so we don't warn unnecessarily.
        TrustState::AlreadyTrusted
    }
}

// =============================================================================
// JSONL → claude-event mapping
// =============================================================================

/// Convert a JSONL conversation record into the envelope shape `ChatPane.vue`
/// already consumes. Returns `None` for noise records (custom-title, agent-name,
/// summary, etc.) that shouldn't surface in chat.
fn jsonl_to_event(record: &serde_json::Value) -> Option<serde_json::Value> {
    let record_type = record.get("type").and_then(|v| v.as_str())?;
    match record_type {
        "user" => {
            if record
                .get("isMeta")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                return None;
            }
            let message = record
                .get("message")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            Some(serde_json::json!({ "type": "user", "message": message }))
        }
        "assistant" => {
            let message = record
                .get("message")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            Some(serde_json::json!({ "type": "assistant", "message": message }))
        }
        "system" => Some(record.clone()),
        "summary" | "custom-title" | "agent-name" | "permission-mode" | "pr-link" => None,
        _ => None,
    }
}

// =============================================================================
// tmux helpers
// =============================================================================

fn tmux_session_name(session_id: &str) -> String {
    format!("claude-{session_id}")
}

/// Patterns that indicate claude's TUI is showing a decision prompt the user
/// must respond to (permission requests, plan-mode commits, slash-command
/// picks). Maintained by hand — update when claude's wording changes.
///
/// Strings here must be specific enough that they don't appear in normal
/// assistant output. We deliberately key on the rendered TUI strings (with
/// the "❯" arrow glyph and "1." prefix) rather than English phrasing alone,
/// so we don't false-positive on assistant text that happens to mention
/// "Do you want to" in a code snippet.
const PERMISSION_PROMPT_PATTERNS: &[&str] = &[
    "Do you want to make this edit",
    "Do you want to allow",
    "Do you want to proceed",
    "Do you want me to ",
    "Approve this change",
    "❯ 1. Yes",
    "❯ 1. Approve",
];

fn contains_permission_prompt(window: &str) -> bool {
    PERMISSION_PROMPT_PATTERNS
        .iter()
        .any(|p| window.contains(p))
}

/// Probe whether a tmux session is still alive. `tmux has-session -t <name>`
/// exits 0 when the session exists, non-zero otherwise.
pub(crate) fn tmux_session_alive(name: &str) -> bool {
    std::process::Command::new("wsl.exe")
        .args(["-e", "tmux", "has-session", "-t", name])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Spawn `wsl.exe -e tmux <args>` and wait for it. Returns stderr on failure.
fn run_tmux(args: &[&str]) -> Result<(), String> {
    let output = std::process::Command::new("wsl.exe")
        .arg("-e")
        .arg("tmux")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to spawn tmux: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(())
}

/// Send `content` to a tmux session via `load-buffer | paste-buffer`. This is
/// the multi-line-safe path: stdin avoids shell-escaping headaches and
/// paste-buffer delivers atomically with no risk of tmux interpreting key
/// names mid-string.
fn tmux_paste_into(tmux_session: &str, content: &str, submit: bool) -> Result<(), String> {
    let buf_name = format!("mc-{}", uuid::Uuid::new_v4().simple());
    // 1. load-buffer -b <name> -- - : read from stdin into the named buffer
    let mut load = std::process::Command::new("wsl.exe")
        .arg("-e")
        .arg("tmux")
        .args(["load-buffer", "-b", &buf_name, "--", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("tmux load-buffer spawn failed: {e}"))?;
    {
        let stdin = load
            .stdin
            .as_mut()
            .ok_or("tmux load-buffer stdin unavailable")?;
        stdin
            .write_all(content.as_bytes())
            .map_err(|e| format!("tmux load-buffer write failed: {e}"))?;
    }
    let load_status = load
        .wait()
        .map_err(|e| format!("tmux load-buffer wait failed: {e}"))?;
    if !load_status.success() {
        return Err(format!("tmux load-buffer exited {load_status}"));
    }

    // 2. paste-buffer -b <name> -t <session> -d  (delete buffer after)
    run_tmux(&[
        "paste-buffer",
        "-b",
        &buf_name,
        "-t",
        tmux_session,
        "-d",
    ])?;

    if submit {
        // Claude's input accepts a final Enter to submit.
        run_tmux(&["send-keys", "-t", tmux_session, "Enter"])?;
    }
    Ok(())
}

// =============================================================================
// JSONL path resolution
// =============================================================================

/// Compute the JSONL path for a session at `cwd` with `session_id`. Probes the
/// WSL `.claude` dirs first, then native Windows home. If no claude home exists
/// at all, falls back to the first WSL claude dir (the file will be created by
/// claude on first turn). Returns the path AND the claude_home root used.
fn resolve_jsonl_path(cwd: &str, session_id: &str) -> Result<(PathBuf, PathBuf), String> {
    let hash = cwd_to_project_hash(cwd);
    let filename = format!("{session_id}.jsonl");

    for (claude_dir, _) in wsl_claude_dirs() {
        let candidate = claude_dir.join("projects").join(&hash).join(&filename);
        if candidate.exists() {
            return Ok((candidate, claude_dir));
        }
    }
    if let Some(home) = dirs::home_dir() {
        let claude = home.join(".claude");
        let candidate = claude.join("projects").join(&hash).join(&filename);
        if candidate.exists() {
            return Ok((candidate, claude));
        }
    }
    // No existing file. Pick the first WSL claude_dir as the target.
    if let Some((claude_dir, _)) = wsl_claude_dirs().into_iter().next() {
        let candidate = claude_dir.join("projects").join(&hash).join(&filename);
        return Ok((candidate, claude_dir));
    }
    if let Some(home) = dirs::home_dir() {
        let claude = home.join(".claude");
        let candidate = claude.join("projects").join(&hash).join(&filename);
        return Ok((candidate, claude));
    }
    Err("No claude home directory found".into())
}

// =============================================================================
// Spawn worker setup (shared between new-session and reattach paths)
// =============================================================================

/// Start the JSONL tail thread + PTY drain thread for a session. Caller has
/// already inserted the `ClaudeIoSession` entry into state and passes the
/// session's buffer Arc so the drain thread can populate it. The `generation`
/// matches the just-inserted ClaudeIoSession; the drain thread's EOF cleanup
/// only touches the state map if the live entry's generation still matches —
/// preventing a stale drain thread from evicting a newer attach.
fn start_workers(
    app: tauri::AppHandle,
    session_id: String,
    jsonl_path: PathBuf,
    baseline_offset: u64,
    reader: Box<dyn std::io::Read + Send>,
    tail_stop_rx: std::sync::mpsc::Receiver<()>,
    pty_buffer: PtyBuffer,
    generation: u64,
) {
    // PTY drain: surface as debug `pty` events AND append to the ring buffer
    // so a late-subscribing frontend can replay. EOF = claude exited.
    {
        let app = app.clone();
        let sid = session_id.clone();
        std::thread::spawn(move || {
            let mut sink = [0u8; 4096];
            let mut r = reader;
            // Sliding window over recent PTY text for permission-prompt detection.
            // Capped at 8 KB — enough to span a multi-byte prompt rendered across
            // separate read chunks, but short enough that we re-scan cheaply.
            let mut prompt_scan = String::new();
            let mut prompt_active = false;
            loop {
                match std::io::Read::read(&mut r, &mut sink) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        // Append to ring buffer (evict from front past cap).
                        if let Ok(mut buf) = pty_buffer.lock() {
                            buf.extend(sink[..n].iter().copied());
                            while buf.len() > PTY_BUFFER_CAP {
                                buf.pop_front();
                            }
                        }
                        let text = String::from_utf8_lossy(&sink[..n]).into_owned();
                        // Pattern-scan a sliding window for permission prompts
                        // and emit a debounced event when one appears. We only
                        // fire the EDGE — once a prompt is active we suppress
                        // re-fires until claude redraws without it.
                        prompt_scan.push_str(&text);
                        if prompt_scan.len() > 8192 {
                            // Snap to next char boundary — claude's TUI uses
                            // multi-byte glyphs (e.g. "❯" is 3 bytes) and a
                            // raw byte offset can land mid-codepoint, which
                            // would panic `String::drain`. UTF-8 codepoints
                            // are at most 4 bytes so this loops <= 3 times.
                            let mut drain = prompt_scan.len() - 8192;
                            while drain < prompt_scan.len()
                                && !prompt_scan.is_char_boundary(drain)
                            {
                                drain += 1;
                            }
                            prompt_scan.drain(..drain);
                        }
                        let now_prompting = contains_permission_prompt(&prompt_scan);
                        if now_prompting && !prompt_active {
                            prompt_active = true;
                            let _ = app.emit(
                                "claude-event",
                                serde_json::json!({
                                    "sessionId": sid,
                                    "event": {
                                        "type": "system",
                                        "subtype": "permission-prompt",
                                    },
                                }),
                            );
                        } else if !now_prompting && prompt_active {
                            prompt_active = false;
                            let _ = app.emit(
                                "claude-event",
                                serde_json::json!({
                                    "sessionId": sid,
                                    "event": {
                                        "type": "system",
                                        "subtype": "permission-prompt-cleared",
                                    },
                                }),
                            );
                        }
                        let _ = app.emit(
                            "claude-event",
                            serde_json::json!({
                                "sessionId": sid,
                                "event": { "type": "pty", "text": text },
                            }),
                        );
                    }
                }
            }
            // EOF on the PTY means our `tmux attach` ended. Two possible
            // causes: (1) claude exited and tmux tore the session down, or
            // (2) we (Tauri) intentionally dropped the PTY for a disconnect
            // while claude is still alive. Distinguish by probing whether
            // the tmux session still exists. Only do FULL teardown (kill
            // tmux, remove from owned-sessions, clean worktree) when claude
            // is genuinely gone — otherwise just release our PTY entry and
            // leave the session running for resume.
            let tmux = tmux_session_name(&sid);
            let session_dead = !tmux_session_alive(&tmux);
            if session_dead {
                let _ = run_tmux(&["kill-session", "-t", &tmux]);
                if let Some(state) = app.try_state::<OwnedSessionsState>() {
                    if let Ok(Some(entry)) = state.remove(&sid) {
                        if let Some(wt) = &entry.worktree_path {
                            remove_worktree_best_effort(&app, &sid, wt);
                        }
                    }
                }
                let _ = app.emit(
                    "claude-session-closed",
                    serde_json::json!({ "sessionId": sid, "exitCode": 0 }),
                );
            }
            // Release the ClaudeIoState entry — but ONLY if the live entry's
            // generation still matches ours. If a rapid disconnect → reattach
            // inserted a new entry under the same session_id while we were
            // wrapping up, that entry is owned by a different drain thread
            // and must not be touched.
            if let Some(io_state) = app.try_state::<ClaudeIoState>() {
                if let Ok(mut sessions) = io_state.sessions.lock() {
                    let same_gen = sessions
                        .get(&sid)
                        .map(|s| s.generation == generation)
                        .unwrap_or(false);
                    if same_gen {
                        sessions.remove(&sid);
                    }
                }
            }
        });
    }

    // JSONL tail thread.
    {
        let app = app.clone();
        let sid = session_id.clone();
        let path = jsonl_path.clone();
        let mut offset = baseline_offset;
        std::thread::spawn(move || {
            let mut carry = String::new();
            loop {
                if matches!(
                    tail_stop_rx.recv_timeout(Duration::from_millis(200)),
                    Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected)
                ) {
                    break;
                }
                // The JSONL file may not exist yet (claude creates it on first
                // turn). Tolerate ENOENT silently.
                let len = match std::fs::metadata(&path) {
                    Ok(m) => m.len(),
                    Err(_) => continue,
                };
                if len <= offset {
                    continue;
                }
                let mut file = match std::fs::File::open(&path) {
                    Ok(f) => f,
                    Err(_) => continue,
                };
                use std::io::{Read, Seek, SeekFrom};
                if file.seek(SeekFrom::Start(offset)).is_err() {
                    continue;
                }
                let mut buf = Vec::with_capacity((len - offset) as usize);
                let read = match file.read_to_end(&mut buf) {
                    Ok(n) => n,
                    Err(_) => continue,
                };
                offset += read as u64;

                let chunk = String::from_utf8_lossy(&buf);
                carry.push_str(&chunk);

                // Defensive cap: if a writer never emits `\n`, carry grows
                // unbounded. Surface the dropped prefix and reset.
                if carry.len() > TAIL_CARRY_CAP {
                    let dropped: String = carry.chars().take(2048).collect();
                    let _ = app.emit(
                        "claude-event",
                        serde_json::json!({
                            "sessionId": sid,
                            "event": {
                                "type": "raw",
                                "text": format!("[tail buffer overflow; dropped {} bytes, prefix: {}]", carry.len(), dropped),
                            },
                        }),
                    );
                    carry.clear();
                    continue;
                }

                let mut lines: Vec<&str> = carry.split('\n').collect();
                // The last element is the carry-over (empty if chunk ended on `\n`).
                let tail = lines.pop().unwrap_or("").to_string();

                for line in lines {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    match serde_json::from_str::<serde_json::Value>(trimmed) {
                        Ok(record) => {
                            if let Some(event) = jsonl_to_event(&record) {
                                let _ = app.emit(
                                    "claude-event",
                                    serde_json::json!({ "sessionId": sid, "event": event }),
                                );
                            }
                        }
                        Err(_) => {
                            // Malformed JSONL — surface as raw so debug Diagnostics
                            // shows it instead of silently dropping.
                            let _ = app.emit(
                                "claude-event",
                                serde_json::json!({
                                    "sessionId": sid,
                                    "event": { "type": "raw", "text": trimmed },
                                }),
                            );
                        }
                    }
                }
                carry = tail;
            }
        });
    }
}

// =============================================================================
// Spawn helpers
// =============================================================================

/// Spawn the tmux+claude command in a portable_pty. Returns the writer, master,
/// reader, and child PID.
fn spawn_in_pty(
    cwd: &str,
    bash_cmd: &str,
) -> Result<
    (
        Box<dyn std::io::Write + Send>,
        Box<dyn portable_pty::MasterPty + Send>,
        Box<dyn std::io::Read + Send>,
        Option<u32>,
    ),
    String,
> {
    use portable_pty::{native_pty_system, CommandBuilder, PtySize};

    let pair = native_pty_system()
        .openpty(PtySize {
            rows: 50,
            cols: 220,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Failed to open PTY: {e}"))?;

    let mut cmd = CommandBuilder::new("wsl.exe");
    cmd.args(["--cd", cwd, "-e", "bash", "-ilc", bash_cmd]);

    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("Failed to spawn tmux+claude in PTY: {e}"))?;
    let pid = child.process_id();
    drop(pair.slave);

    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("Failed to clone PTY reader: {e}"))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|e| format!("Failed to take PTY writer: {e}"))?;

    Ok((writer, pair.master, reader, pid))
}

/// Build the `bash -ilc` command for spawning a session via tmux+claude.
/// `claude_flag` is either `"--session-id"` (fresh spawn) or `"--resume"`
/// (resume an existing JSONL).
///
/// Both `cd '<cwd>' &&` AND the wsl.exe `--cd` are required: bash -i runs the
/// user's interactive bashrc, which can chdir away. Claude resolves its
/// session by hashing process.cwd() — if it doesn't match the original,
/// claude silently starts a new session.
fn build_tmux_claude_cmd(cwd: &str, session_id: &str, claude_flag: &str) -> String {
    let cwd_esc = shell_quote(cwd);
    let sid_esc = shell_quote(session_id);
    let tmux = shell_quote(&tmux_session_name(session_id));
    format!(
        "cd {cwd_esc} && tmux new-session -d -x 220 -y 50 -s {tmux} \
         'claude {claude_flag} {sid_esc} --permission-mode acceptEdits --allowedTools {tools}' && \
         tmux set-option -t {tmux} -g aggressive-resize on >/dev/null 2>&1; \
         tmux attach -t {tmux}",
        tools = PRE_APPROVED_TOOLS,
    )
}

/// POSIX-safe single-quoted shell escape.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

// =============================================================================
// Worktree support (Feature #7)
// =============================================================================

/// If `worktree` is true, create a fresh worktree under `<cwd>/.worktrees/<short-uuid>`
/// and return its WSL path. Otherwise returns None.
fn maybe_create_worktree(cwd: &str, worktree: bool) -> Result<Option<String>, String> {
    if !worktree {
        return Ok(None);
    }
    let short = uuid::Uuid::new_v4().simple().to_string();
    let short = &short[..8];
    let worktree_path = format!("{}/.worktrees/{}", cwd.trim_end_matches('/'), short);

    // Ensure the worktrees dir exists. `git worktree add` will create the leaf.
    let mkdir_cmd = format!(
        "mkdir -p {parent} && git -C {cwd} worktree add {wt} HEAD",
        parent = shell_quote(&format!("{}/.worktrees", cwd.trim_end_matches('/'))),
        cwd = shell_quote(cwd),
        wt = shell_quote(&worktree_path),
    );
    let output = std::process::Command::new("wsl.exe")
        .args(["-e", "bash", "-lc", &mkdir_cmd])
        .output()
        .map_err(|e| format!("Failed to invoke git worktree add: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "git worktree add failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(Some(worktree_path))
}

// =============================================================================
// Tauri commands
// =============================================================================

/// Launch a brand-new Claude session from Mission Control.
#[tauri::command]
pub async fn start_new_claude_session(
    app: tauri::AppHandle,
    cwd: String,
    initial_prompt: Option<String>,
    worktree: Option<bool>,
    io_state: tauri::State<'_, ClaudeIoState>,
    owned_state: tauri::State<'_, OwnedSessionsState>,
    projects_state: tauri::State<'_, crate::state::projects::ProjectsState>,
) -> Result<NewSessionInfo, String> {
    let cwd = cwd.trim().to_string();
    if cwd.is_empty() {
        return Err("Working directory cannot be empty".into());
    }
    // Spike: verify tmux is installed. Hard fail if not — broadcast/attach is
    // part of the value prop and silently falling back to raw PTY would be
    // misleading.
    if std::process::Command::new("wsl.exe")
        .args(["-e", "which", "tmux"])
        .output()
        .map(|o| !o.status.success())
        .unwrap_or(true)
    {
        return Err(
            "tmux is not installed in WSL. Install it with: sudo apt-get install -y tmux".into(),
        );
    }

    // Worktree mode (Feature #7) — if requested, create worktree first and
    // re-point cwd at it. Failure here aborts the launch without spawning.
    let worktree_path = maybe_create_worktree(&cwd, worktree.unwrap_or(false))?;
    let effective_cwd = worktree_path.clone().unwrap_or_else(|| cwd.clone());

    let session_id = uuid::Uuid::new_v4().to_string();
    let trust = ensure_workspace_trust(&effective_cwd);
    if trust == TrustState::WriteFailed {
        let _ = app.emit(
            "claude-event",
            serde_json::json!({
                "sessionId": session_id,
                "event": {
                    "type": "system",
                    "subtype": "trust-warning",
                    "text": "Could not pre-accept workspace trust for this directory. The first send may dismiss a dialog instead of being received.",
                },
            }),
        );
    }

    let (jsonl_path, claude_home) = resolve_jsonl_path(&effective_cwd, &session_id)?;
    let baseline_offset = std::fs::metadata(&jsonl_path)
        .map(|m| m.len())
        .unwrap_or(0);

    let bash_cmd = build_tmux_claude_cmd(&effective_cwd, &session_id, "--session-id");
    let (writer, master, reader, pid) = spawn_in_pty(&effective_cwd, &bash_cmd)?;
    let pid = pid.unwrap_or(0);

    let (tail_stop_tx, tail_stop_rx) = std::sync::mpsc::channel::<()>();
    let pty_buffer = Arc::new(Mutex::new(VecDeque::with_capacity(PTY_BUFFER_CAP)));
    let generation = next_generation();
    io_state
        .sessions
        .lock()
        .map_err(|e| format!("State lock poisoned: {e}"))?
        .insert(
            session_id.clone(),
            ClaudeIoSession {
                writer,
                _master: master,
                _tail_stop: tail_stop_tx,
                pty_buffer: pty_buffer.clone(),
                generation,
            },
        );

    let started_at = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() * 1000)
        .unwrap_or(0);

    // Store effective_cwd (the worktree path if in worktree mode, else the
    // original cwd). Claude hashes its process.cwd() to locate the JSONL, so
    // downstream consumers (get_session_detail, the reattach path) MUST hash
    // the same value or they'd watch a non-existent path.
    owned_state.insert(OwnedSession {
        session_id: session_id.clone(),
        cwd: effective_cwd.clone(),
        pid,
        started_at,
        label: None,
        claude_home: claude_home.to_string_lossy().into_owned(),
        worktree_path: worktree_path.clone(),
        tmux_session: tmux_session_name(&session_id),
        ended_at: None,
    })?;

    start_workers(
        app.clone(),
        session_id.clone(),
        jsonl_path.clone(),
        baseline_offset,
        reader,
        tail_stop_rx,
        pty_buffer,
        generation,
    );

    // Initial prompt: send once claude is actually accepting input. The first
    // JSONL record signals readiness; if that doesn't arrive within 2s, give
    // up and send anyway (paste-buffer queues until claude reads it).
    if let Some(prompt) = initial_prompt.filter(|p| !p.trim().is_empty()) {
        let tmux = tmux_session_name(&session_id);
        let jsonl_path_for_wait = jsonl_path.clone();
        let app_clone = app.clone();
        let sid_for_emit = session_id.clone();
        std::thread::spawn(move || {
            wait_for_jsonl(&jsonl_path_for_wait, Duration::from_millis(2000));
            if let Err(e) = tmux_paste_into(&tmux, &prompt, true) {
                let _ = app_clone.emit(
                    "claude-event",
                    serde_json::json!({
                        "sessionId": sid_for_emit,
                        "event": {
                            "type": "system",
                            "subtype": "send-failed",
                            "text": format!("Initial prompt failed to send: {e}"),
                        },
                    }),
                );
            }
        });
    }

    // Record in the project registry under the original cwd (the user's
    // semantic project root) — NOT effective_cwd, which would be the ephemeral
    // worktree directory. Best-effort: registry failures don't block the
    // spawn since claude is already running.
    let _ = projects_state.record_used(&cwd);

    Ok(NewSessionInfo {
        session_id,
        pid,
        jsonl_path: jsonl_path.to_string_lossy().into_owned(),
        started_at,
        cwd: effective_cwd,
        worktree_path,
    })
}

/// Re-attach to a session that's already in `OwnedSessionsState` (e.g. after a
/// Tauri restart). Reuses all the spawn machinery but doesn't create a new
/// claude — it tmux-attaches to the still-running one. If the session has died,
/// returns an error.
#[tauri::command]
pub async fn start_claude_session(
    app: tauri::AppHandle,
    session_id: String,
    cwd: String,
    io_state: tauri::State<'_, ClaudeIoState>,
    owned_state: tauri::State<'_, OwnedSessionsState>,
) -> Result<(), String> {
    // Idempotent: if workers are already running for this session (e.g. we
    // just spawned it via start_new_claude_session in this same Tauri lifetime),
    // we still replay the PTY ring buffer so a late-subscribing frontend
    // (xterm in ChatPane mounting after spawn) catches up on claude's
    // initial render. Without this, the panel is blank until the user does
    // something that nudges claude to redraw.
    {
        let sessions = io_state
            .sessions
            .lock()
            .map_err(|e| format!("State lock poisoned: {e}"))?;
        if let Some(session) = sessions.get(&session_id) {
            replay_pty_buffer(&app, &session_id, &session.pty_buffer);
            return Ok(());
        }
    }
    let owned = owned_state
        .get(&session_id)
        .ok_or_else(|| format!("Session {session_id} is not tracked by Mission Control"))?;

    let (jsonl_path, _claude_home) = resolve_jsonl_path(&cwd, &session_id)?;
    let baseline_offset = std::fs::metadata(&jsonl_path)
        .map(|m| m.len())
        .unwrap_or(0);

    // Just attach to the existing tmux session — claude itself is already running.
    let tmux = shell_quote(&owned.tmux_session);
    let bash_cmd = format!(
        "cd {cwd} && tmux attach -t {tmux}",
        cwd = shell_quote(&cwd),
    );
    let (writer, master, reader, _pid) = spawn_in_pty(&cwd, &bash_cmd)?;
    let (tail_stop_tx, tail_stop_rx) = std::sync::mpsc::channel::<()>();
    let pty_buffer = Arc::new(Mutex::new(VecDeque::with_capacity(PTY_BUFFER_CAP)));
    let generation = next_generation();
    io_state
        .sessions
        .lock()
        .map_err(|e| format!("State lock poisoned: {e}"))?
        .insert(
            session_id.clone(),
            ClaudeIoSession {
                writer,
                _master: master,
                _tail_stop: tail_stop_tx,
                pty_buffer: pty_buffer.clone(),
                generation,
            },
        );

    start_workers(
        app,
        session_id,
        jsonl_path,
        baseline_offset,
        reader,
        tail_stop_rx,
        pty_buffer,
        generation,
    );
    Ok(())
}

/// Emit the current contents of a session's PTY ring buffer as one big
/// `pty` event so the frontend xterm can render the live screen state.
fn replay_pty_buffer(
    app: &tauri::AppHandle,
    session_id: &str,
    pty_buffer: &PtyBuffer,
) {
    let snapshot: Vec<u8> = match pty_buffer.lock() {
        Ok(buf) => buf.iter().copied().collect(),
        Err(_) => return,
    };
    if snapshot.is_empty() {
        return;
    }
    let text = String::from_utf8_lossy(&snapshot).into_owned();
    let _ = app.emit(
        "claude-event",
        serde_json::json!({
            "sessionId": session_id,
            "event": { "type": "pty", "text": text },
        }),
    );
}

/// Send a user message into a running session via tmux paste-buffer.
///
/// **Legacy.** Kept for the initial-prompt code path (where we want a single
/// "send this and submit" semantic on spawn). The interactive chat now uses
/// `write_session_pty` directly — see `write_session_pty` below.
#[tauri::command]
pub async fn send_claude_message(
    session_id: String,
    content: String,
    owned_state: tauri::State<'_, OwnedSessionsState>,
) -> Result<(), String> {
    let owned = owned_state
        .get(&session_id)
        .ok_or("No such session")?;
    tmux_paste_into(&owned.tmux_session, &content, true)
}

/// Write raw bytes directly to a session's PTY master. This is the path the
/// terminal UI uses: every keystroke in xterm.js becomes a `write_session_pty`
/// call. Since the PTY is connected via `bash → tmux attach → claude`, the
/// bytes flow straight through to claude — exactly as if the user were typing
/// in a real terminal. Multi-line, escape sequences, special characters: all
/// pass through unchanged.
#[tauri::command]
pub async fn write_session_pty(
    session_id: String,
    data: String,
    io_state: tauri::State<'_, ClaudeIoState>,
) -> Result<(), String> {
    let mut sessions = io_state
        .sessions
        .lock()
        .map_err(|e| format!("State lock poisoned: {e}"))?;
    let session = sessions
        .get_mut(&session_id)
        .ok_or("No active Claude session")?;
    use std::io::Write;
    session
        .writer
        .write_all(data.as_bytes())
        .map_err(|e| format!("PTY write failed: {e}"))?;
    session
        .writer
        .flush()
        .map_err(|e| format!("PTY flush failed: {e}"))?;
    Ok(())
}

/// Resize the PTY (and by extension the tmux window and claude's TUI render
/// area) to match the xterm.js viewport. Called on initial fit and on every
/// window resize. The master PTY's `resize` propagates SIGWINCH to bash,
/// which is forwarded to tmux, which redraws claude at the new dimensions.
#[tauri::command]
pub async fn resize_session_pty(
    session_id: String,
    cols: u16,
    rows: u16,
    io_state: tauri::State<'_, ClaudeIoState>,
) -> Result<(), String> {
    let sessions = io_state
        .sessions
        .lock()
        .map_err(|e| format!("State lock poisoned: {e}"))?;
    let session = sessions
        .get(&session_id)
        .ok_or("No active Claude session")?;
    use portable_pty::PtySize;
    session
        ._master
        .resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("PTY resize failed: {e}"))?;
    Ok(())
}

/// Send Ctrl-C to interrupt the current turn (Feature #14). The claude process
/// stays alive; only the in-flight reasoning is cancelled.
#[tauri::command]
pub async fn interrupt_claude_session(
    session_id: String,
    owned_state: tauri::State<'_, OwnedSessionsState>,
) -> Result<(), String> {
    let owned = owned_state
        .get(&session_id)
        .ok_or("No such session")?;
    run_tmux(&["send-keys", "-t", &owned.tmux_session, "C-c"])
}

/// Read a list of historical (dead) sessions, newest-first. Wave 4 history view.
#[tauri::command]
pub async fn list_session_history(
    limit: Option<usize>,
    owned_state: tauri::State<'_, OwnedSessionsState>,
) -> Result<Vec<OwnedSession>, String> {
    Ok(owned_state.list_history(limit.unwrap_or(200)))
}

/// Permanently drop a session from history.
#[tauri::command]
pub async fn forget_session_history(
    session_id: String,
    owned_state: tauri::State<'_, OwnedSessionsState>,
) -> Result<bool, String> {
    owned_state.forget_history(&session_id)
}

/// Resume a historical session: re-spawn `claude --resume <sid>` in the
/// original cwd and move the entry from history back to live entries with a
/// fresh pid + tmux session. The original session_id is preserved so the
/// JSONL transcript continues.
#[tauri::command]
pub async fn resume_owned_session(
    app: tauri::AppHandle,
    session_id: String,
    io_state: tauri::State<'_, ClaudeIoState>,
    owned_state: tauri::State<'_, OwnedSessionsState>,
) -> Result<NewSessionInfo, String> {
    let historical = owned_state
        .pop_history(&session_id)
        .map_err(|e| format!("history lookup failed: {e}"))?
        .ok_or_else(|| format!("Session {session_id} not in history"))?;

    let cwd = historical.cwd.clone();
    // Re-trust the workspace — the project may have moved or been mass-edited
    // since it was last open.
    let trust = ensure_workspace_trust(&cwd);
    if trust == TrustState::WriteFailed {
        let _ = app.emit(
            "claude-event",
            serde_json::json!({
                "sessionId": session_id,
                "event": {
                    "type": "system",
                    "subtype": "trust-warning",
                    "text": "Could not pre-accept workspace trust on resume.",
                },
            }),
        );
    }

    let (jsonl_path, claude_home) = resolve_jsonl_path(&cwd, &session_id)?;
    let baseline_offset = std::fs::metadata(&jsonl_path)
        .map(|m| m.len())
        .unwrap_or(0);

    if std::process::Command::new("wsl.exe")
        .args(["-e", "which", "tmux"])
        .output()
        .map(|o| !o.status.success())
        .unwrap_or(true)
    {
        return Err(
            "tmux is not installed in WSL. Install it with: sudo apt-get install -y tmux".into(),
        );
    }

    let bash_cmd = build_tmux_claude_cmd(&cwd, &session_id, "--resume");
    let (writer, master, reader, pid) = spawn_in_pty(&cwd, &bash_cmd)?;
    let pid = pid.unwrap_or(0);

    let (tail_stop_tx, tail_stop_rx) = std::sync::mpsc::channel::<()>();
    let pty_buffer = Arc::new(Mutex::new(VecDeque::with_capacity(PTY_BUFFER_CAP)));
    let generation = next_generation();
    io_state
        .sessions
        .lock()
        .map_err(|e| format!("State lock poisoned: {e}"))?
        .insert(
            session_id.clone(),
            ClaudeIoSession {
                writer,
                _master: master,
                _tail_stop: tail_stop_tx,
                pty_buffer: pty_buffer.clone(),
                generation,
            },
        );

    let started_at = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() * 1000)
        .unwrap_or(0);

    // Re-register in live entries with the new pid + started_at; preserve
    // label / worktree_path from the historical record.
    owned_state.insert(OwnedSession {
        session_id: session_id.clone(),
        cwd: cwd.clone(),
        pid,
        started_at,
        label: historical.label.clone(),
        claude_home: claude_home.to_string_lossy().into_owned(),
        worktree_path: historical.worktree_path.clone(),
        tmux_session: tmux_session_name(&session_id),
        ended_at: None,
    })?;

    start_workers(
        app,
        session_id.clone(),
        jsonl_path.clone(),
        baseline_offset,
        reader,
        tail_stop_rx,
        pty_buffer,
        generation,
    );

    Ok(NewSessionInfo {
        session_id,
        pid,
        jsonl_path: jsonl_path.to_string_lossy().into_owned(),
        started_at,
        cwd,
        worktree_path: historical.worktree_path,
    })
}

/// **Disconnect** our PTY from a session WITHOUT killing it. Closes our
/// tmux client; the tmux session and claude inside it keep running, so the
/// user can resume from the panel later (or from any external `tmux attach`).
/// Used as the default cleanup when the chat panel unmounts — closing a window
/// shouldn't end claude's life.
#[tauri::command]
pub async fn disconnect_session(
    session_id: String,
    io_state: tauri::State<'_, ClaudeIoState>,
) -> Result<(), String> {
    // Dropping the ClaudeIoSession entry drops the PTY master → SIGHUP →
    // bash exits → `tmux attach` disconnects (but server + session live on).
    // The drain thread reaches EOF and runs its cleanup; the cleanup now
    // checks whether the tmux session is still alive and only does full
    // teardown if it isn't.
    io_state
        .sessions
        .lock()
        .map_err(|e| format!("State lock poisoned: {e}"))?
        .remove(&session_id);
    Ok(())
}

/// **Stop** a session: drop our PTY, kill the tmux session (which SIGHUPs
/// claude), and clean up worktree + state. We own the teardown here because
/// preemptively removing the OwnedSession entry would prevent the drain
/// thread's EOF cleanup from seeing the worktree path — leaking the worktree.
#[tauri::command]
pub async fn stop_claude_session(
    app: tauri::AppHandle,
    session_id: String,
    io_state: tauri::State<'_, ClaudeIoState>,
    owned_state: tauri::State<'_, OwnedSessionsState>,
) -> Result<(), String> {
    let owned = owned_state.get(&session_id);
    if let Some(s) = &owned {
        let _ = run_tmux(&["kill-session", "-t", &s.tmux_session]);
    }

    io_state
        .sessions
        .lock()
        .map_err(|e| format!("State lock poisoned: {e}"))?
        .remove(&session_id);

    // Remove from owned-sessions AND capture the entry so we can do worktree
    // cleanup ourselves. The drain thread's EOF path will see Ok(None) here
    // and skip its (now redundant) cleanup branch.
    if let Ok(Some(entry)) = owned_state.remove(&session_id) {
        if let Some(wt) = &entry.worktree_path {
            remove_worktree_best_effort(&app, &session_id, wt);
        }
    }
    Ok(())
}

/// Run `git worktree remove <wt>`; on failure (typically because the worktree
/// has uncommitted changes) emit a `worktree-cleanup-failed` system event so
/// the UI can surface it. We do NOT force-remove — that would silently
/// destroy uncommitted work.
fn remove_worktree_best_effort(app: &tauri::AppHandle, session_id: &str, wt: &str) {
    let out = std::process::Command::new("wsl.exe")
        .args(["-e", "git", "worktree", "remove", wt])
        .output();
    if let Ok(out) = out {
        if !out.status.success() {
            let _ = app.emit(
                "claude-event",
                serde_json::json!({
                    "sessionId": session_id,
                    "event": {
                        "type": "system",
                        "subtype": "worktree-cleanup-failed",
                        "worktreePath": wt,
                        "stderr": String::from_utf8_lossy(&out.stderr).trim(),
                    },
                }),
            );
        }
    }
}

/// Update a session's friendly label (Feature #20).
#[tauri::command]
pub async fn update_session_label(
    session_id: String,
    label: Option<String>,
    owned_state: tauri::State<'_, OwnedSessionsState>,
) -> Result<(), String> {
    let label = label.and_then(|s| {
        let t = s.trim();
        if t.is_empty() {
            None
        } else {
            Some(t.to_string())
        }
    });
    let found = owned_state.set_label(&session_id, label)?;
    if !found {
        return Err("No such session".into());
    }
    Ok(())
}

/// Open the native directory picker. Used by the launch UI's "Browse…" button.
/// Returns the picked path translated to a WSL path if the user chose a Windows
/// drive (e.g. `C:\dev\foo` → `/mnt/c/dev/foo`).
#[tauri::command]
pub async fn pick_directory(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = tokio::sync::oneshot::channel::<Option<PathBuf>>();
    // Parent the dialog to the main palette window so Windows treats it as a
    // modal child rather than a sibling top-level window. Without this, the
    // native picker steals focus and our frameless alwaysOnTop palette can
    // get visually demoted behind the picker — the user perceives the
    // palette as "hiding."
    let parent = app.get_webview_window("main");
    let mut builder = app
        .dialog()
        .file()
        .set_title("Choose a working directory");
    if let Some(ref w) = parent {
        builder = builder.set_parent(w);
    }
    builder.pick_folder(move |path| {
        let resolved = path.and_then(|p| p.into_path().ok());
        let _ = tx.send(resolved);
    });
    let picked = rx
        .await
        .map_err(|e| format!("Picker dropped: {e}"))?;
    Ok(picked.map(|p| windows_path_to_wsl(&p)))
}

/// Open a file path in the user's editor of choice (IntelliJ if available,
/// Explorer fallback). Accepts WSL paths (`/mnt/c/...`) and translates to
/// Windows form so the editor understands. Optional `:LINE[:COL]` suffix is
/// stripped before opening (IntelliJ's CLI doesn't accept it inline).
#[tauri::command]
pub async fn open_path_in_editor(path: String) -> Result<(), String> {
    let raw = path.trim();
    if raw.is_empty() {
        return Err("Empty path".into());
    }
    // Strip a trailing `:NUM` or `:NUM:NUM` (line/col hints) the link
    // provider may have included.
    let stripped = strip_line_suffix(raw);
    let windows = wsl_path_to_windows(stripped);

    // Try IntelliJ first.
    if std::process::Command::new("idea64.exe")
        .arg(&windows)
        .spawn()
        .is_ok()
    {
        return Ok(());
    }
    // Fall back to the OS default (explorer.exe opens with the registered
    // app for the file type, or just the parent folder for directories).
    std::process::Command::new("explorer.exe")
        .arg(&windows)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to open path: {e}"))
}

fn strip_line_suffix(s: &str) -> &str {
    // Match trailing ":N" or ":N:N" where N is digits.
    let bytes = s.as_bytes();
    let mut end = s.len();
    let mut colons = 0;
    while colons < 2 {
        // Walk back over digits
        let digit_end = end;
        while end > 0 && bytes[end - 1].is_ascii_digit() {
            end -= 1;
        }
        if end == digit_end || end == 0 || bytes[end - 1] != b':' {
            // No digits found or no preceding colon — undo this attempt.
            return if colons == 0 { s } else { &s[..digit_end + colons - 1] };
        }
        end -= 1;
        colons += 1;
    }
    &s[..end]
}

/// Translate a WSL path like `/mnt/c/dev/foo.ts` to `C:\dev\foo.ts`. Pure
/// Linux paths (e.g. `/home/<u>/...`) become UNC (`\\wsl.localhost\Ubuntu\...`).
/// Already-Windows paths pass through unchanged.
fn wsl_path_to_windows(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("/mnt/") {
        if let Some((drive, tail)) = rest.split_once('/') {
            if drive.len() == 1 && drive.chars().all(|c| c.is_ascii_alphabetic()) {
                return format!(
                    "{}:\\{}",
                    drive.to_uppercase(),
                    tail.replace('/', "\\"),
                );
            }
        } else if rest.len() == 1 {
            return format!("{}:\\", rest.to_uppercase());
        }
    }
    if path.starts_with('/') {
        return format!(r"\\wsl.localhost\Ubuntu{}", path.replace('/', "\\"));
    }
    // Looks like a Windows path already, or relative — pass through.
    path.to_string()
}

/// Translate a Windows path like `C:\dev\fnba-utils` to `/mnt/c/dev/fnba-utils`.
/// UNC paths under `\\wsl.localhost\Ubuntu\...` become `/...`. Already-Linux
/// paths pass through unchanged.
fn windows_path_to_wsl(path: &Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    if let Some(rest) = s.strip_prefix("//wsl.localhost/") {
        // //wsl.localhost/<distro>/<rest> → /<rest>. If no rest, return root.
        return match rest.split_once('/') {
            Some((_distro, tail)) => format!("/{tail}"),
            None => "/".to_string(),
        };
    }
    // Detect "X:/..." drive prefix.
    if let Some((drive, rest)) = s.split_once(":/") {
        if drive.len() == 1 && drive.chars().all(|c| c.is_ascii_alphabetic()) {
            return format!("/mnt/{}/{rest}", drive.to_lowercase());
        }
    }
    s
}

// =============================================================================
// Misc
// =============================================================================

/// Block briefly waiting for `path` to appear, up to `timeout`. No-op if it
/// already exists.
fn wait_for_jsonl(path: &Path, timeout: Duration) {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if path.exists() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
