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
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, SystemTime};
use tauri::{Emitter, Manager};

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

/// Probe whether a tmux session is still alive. `tmux has-session -t <name>`
/// exits 0 when the session exists, non-zero otherwise.
fn tmux_session_alive(name: &str) -> bool {
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
/// session's buffer Arc so the drain thread can populate it.
fn start_workers(
    app: tauri::AppHandle,
    session_id: String,
    jsonl_path: PathBuf,
    baseline_offset: u64,
    reader: Box<dyn std::io::Read + Send>,
    tail_stop_rx: std::sync::mpsc::Receiver<()>,
    pty_buffer: std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<u8>>>,
) {
    // PTY drain: surface as debug `pty` events AND append to the ring buffer
    // so a late-subscribing frontend can replay. EOF = claude exited.
    {
        let app = app.clone();
        let sid = session_id.clone();
        std::thread::spawn(move || {
            let mut sink = [0u8; 4096];
            let mut r = reader;
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
                            let cleaned = std::process::Command::new("wsl.exe")
                                .args(["-e", "git", "worktree", "remove", wt])
                                .output();
                            if let Ok(out) = cleaned {
                                if !out.status.success() {
                                    let _ = app.emit(
                                        "claude-event",
                                        serde_json::json!({
                                            "sessionId": sid,
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
                    }
                }
                let _ = app.emit(
                    "claude-session-closed",
                    serde_json::json!({ "sessionId": sid, "exitCode": 0 }),
                );
            }
            // Always release the ClaudeIoState entry — the PTY is gone either way.
            if let Some(io_state) = app.try_state::<ClaudeIoState>() {
                if let Ok(mut sessions) = io_state.sessions.lock() {
                    sessions.remove(&sid);
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

/// Build the `bash -ilc` command for spawning a fresh session via tmux+claude.
/// Both `cd '<cwd>' &&` AND the wsl.exe `--cd` are required: bash -i runs the
/// user's interactive bashrc, which can chdir away (some snippets start in
/// $HOME). Claude resolves its session by hashing process.cwd() — if it
/// doesn't match the original, claude silently starts a new session.
fn build_spawn_cmd(cwd: &str, session_id: &str) -> String {
    let cwd_esc = shell_quote(cwd);
    let sid_esc = shell_quote(session_id);
    let tmux = shell_quote(&tmux_session_name(session_id));
    format!(
        "cd {cwd_esc} && tmux new-session -d -x 220 -y 50 -s {tmux} \
         'claude --session-id {sid_esc} --permission-mode acceptEdits --allowedTools {tools}' && \
         tmux set-option -t {tmux} -g aggressive-resize on >/dev/null 2>&1; \
         tmux attach -t {tmux}",
        tools = PRE_APPROVED_TOOLS,
    )
}

/// Build the resume command (Wave 4 #27 hook — same shape, plus `--resume`).
fn build_resume_cmd(cwd: &str, session_id: &str) -> String {
    let cwd_esc = shell_quote(cwd);
    let sid_esc = shell_quote(session_id);
    let tmux = shell_quote(&tmux_session_name(session_id));
    format!(
        "cd {cwd_esc} && tmux new-session -d -x 220 -y 50 -s {tmux} \
         'claude --resume {sid_esc} --permission-mode acceptEdits --allowedTools {tools}' && \
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

    let bash_cmd = build_spawn_cmd(&effective_cwd, &session_id);
    let (writer, master, reader, pid) = spawn_in_pty(&effective_cwd, &bash_cmd)?;
    let pid = pid.unwrap_or(0);

    let (tail_stop_tx, tail_stop_rx) = std::sync::mpsc::channel::<()>();
    let pty_buffer = std::sync::Arc::new(std::sync::Mutex::new(
        std::collections::VecDeque::with_capacity(PTY_BUFFER_CAP),
    ));
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
            },
        );

    let started_at = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() * 1000)
        .unwrap_or(0);

    owned_state.insert(OwnedSession {
        session_id: session_id.clone(),
        cwd: cwd.clone(),
        pid,
        started_at,
        label: None,
        claude_home: claude_home.to_string_lossy().into_owned(),
        worktree_path: worktree_path.clone(),
        tmux_session: tmux_session_name(&session_id),
    })?;

    start_workers(
        app.clone(),
        session_id.clone(),
        jsonl_path.clone(),
        baseline_offset,
        reader,
        tail_stop_rx,
        pty_buffer,
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

    Ok(NewSessionInfo {
        session_id,
        pid,
        jsonl_path: jsonl_path.to_string_lossy().into_owned(),
        started_at,
        cwd,
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
    let pty_buffer = std::sync::Arc::new(std::sync::Mutex::new(
        std::collections::VecDeque::with_capacity(PTY_BUFFER_CAP),
    ));
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
    );
    Ok(())
}

/// Emit the current contents of a session's PTY ring buffer as one big
/// `pty` event so the frontend xterm can render the live screen state.
fn replay_pty_buffer(
    app: &tauri::AppHandle,
    session_id: &str,
    pty_buffer: &std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<u8>>>,
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
/// claude), and clean up worktree + state. The drain thread's EOF path
/// handles the same steps; we run them eagerly here so the caller's UI
/// reflects the change immediately.
#[tauri::command]
pub async fn stop_claude_session(
    session_id: String,
    io_state: tauri::State<'_, ClaudeIoState>,
    owned_state: tauri::State<'_, OwnedSessionsState>,
) -> Result<(), String> {
    // Kill the tmux session explicitly so claude exits even if no other tmux
    // client attached. Do this BEFORE dropping our PTY so the drain thread's
    // EOF cleanup observes tmux is dead and runs full state teardown.
    let owned = owned_state.get(&session_id);
    if let Some(s) = &owned {
        let _ = run_tmux(&["kill-session", "-t", &s.tmux_session]);
    }

    io_state
        .sessions
        .lock()
        .map_err(|e| format!("State lock poisoned: {e}"))?
        .remove(&session_id);

    // Defensive removal from owned-sessions in case the drain thread hasn't
    // observed the EOF yet (or never will because the PTY closed too fast).
    let _ = owned_state.remove(&session_id);
    Ok(())
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

/// Translate a Windows path like `C:\dev\fnba-utils` to `/mnt/c/dev/fnba-utils`.
/// UNC paths under `\\wsl.localhost\Ubuntu\...` become `/...`. Already-Linux
/// paths pass through unchanged.
fn windows_path_to_wsl(path: &Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    if let Some(rest) = s.strip_prefix("//wsl.localhost/") {
        // //wsl.localhost/<distro>/<rest> → /<rest>
        if let Some((_distro, tail)) = rest.split_once('/') {
            return format!("/{tail}");
        }
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
