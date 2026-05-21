use crate::commands::assume_identity::load_all_connections;
use crate::db;
use crate::models::mission_control::{
    ClaudeSession, ConnectionStatus, ConversationMessage, QueryResult, SessionDetail,
    SessionSource, SessionStats, SessionStatus, SqlQueryState, SubagentInfo,
};
use crate::state::owned_sessions::{OwnedSession, OwnedSessionsState};
use crate::state::tmux_sessions::{
    invalidate_cache as invalidate_tmux_cache, list_all_tmux_sessions, ps_contains_claude,
    TmuxSessionInfo,
};
use std::collections::{HashMap, VecDeque};
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime};
use tokio::time::timeout;

// --- Shared helpers ---

/// How long after the last JSONL write we still consider a "user"-last session busy.
/// Claude streams responses within seconds; if the file is older than this the session
/// was interrupted or the user's message was never processed.
const BUSY_STALE_SECS: u64 = 10;

fn is_stale_at(mtime: SystemTime) -> bool {
    SystemTime::now()
        .duration_since(mtime)
        .map_or(false, |age| age.as_secs() > BUSY_STALE_SECS)
}

fn derive_status(last_role: &str, is_stale: bool) -> SessionStatus {
    if last_role == "user" && !is_stale {
        SessionStatus::Busy
    } else {
        SessionStatus::Idle
    }
}

pub(crate) fn cwd_to_project_hash(cwd: &str) -> String {
    // Claude encodes the cwd by replacing every non-alphanumeric (including `.`, `+`,
    // `_`, etc.) with `-`. Hyphens are preserved. The leading `/` becomes a leading `-`.
    let mut out = String::with_capacity(cwd.len() + 1);
    for ch in cwd.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' {
            out.push(ch);
        } else {
            out.push('-');
        }
    }
    out
}

fn read_subagents(projects_dir: &Path, cwd: &str, session_id: &str) -> Vec<SubagentInfo> {
    let hash = cwd_to_project_hash(cwd);
    let subagents_dir = projects_dir.join(&hash).join(session_id).join("subagents");

    let entries = match std::fs::read_dir(&subagents_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut agents = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !name.ends_with(".meta.json") {
            continue;
        }
        if let Ok(contents) = std::fs::read_to_string(&path) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&contents) {
                agents.push(SubagentInfo {
                    agent_type: val
                        .get("agentType")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    description: val
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                });
            }
        }
    }
    agents
}

// --- Lightweight JSONL tail for session status ---

/// Read the last ~8KB of a JSONL conversation file to extract the timestamp and
/// role of the most recent user/assistant message.  O(1) in file size.
fn tail_conversation_status(jsonl_path: &Path) -> (Option<String>, String) {
    use std::io::{Read, Seek, SeekFrom};

    let mut file = match std::fs::File::open(jsonl_path) {
        Ok(f) => f,
        Err(_) => return (None, "unknown".into()),
    };

    let file_len = match file.metadata() {
        Ok(m) => m.len(),
        Err(_) => return (None, "unknown".into()),
    };

    // Seek to the last 8KB (or start of file if smaller).
    // May land mid-character in UTF-8, so read as bytes and use lossy conversion.
    let offset = if file_len > 8192 { file_len - 8192 } else { 0 };
    if file.seek(SeekFrom::Start(offset)).is_err() {
        return (None, "unknown".into());
    }

    let mut bytes = Vec::new();
    if file.read_to_end(&mut bytes).is_err() {
        return (None, "unknown".into());
    }
    let buf = String::from_utf8_lossy(&bytes);

    // Walk lines in reverse to find the last user/assistant record
    for line in buf.lines().rev() {
        let val: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let record_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if record_type != "user" && record_type != "assistant" {
            continue;
        }
        let timestamp = val
            .get("timestamp")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let role = val
            .get("message")
            .and_then(|m| m.get("role"))
            .and_then(|v| v.as_str())
            .unwrap_or(record_type)
            .to_string();
        return (timestamp, role);
    }

    (None, "unknown".into())
}

// --- Caches keyed by JSONL path + (mtime, len) ---
//
// Mission Control polls every 3s. JSONL files live on UNC paths into WSL
// (`\\wsl.localhost\Ubuntu\...`); each open+read is a 9P round-trip. By
// caching tail and parse results behind a freshness check on `metadata()`,
// repeated polls of an idle session collapse to a single fstat.
//
// Cached structs are derived from file content at known (mtime, len).
// `is_stale` is computed on each access from `mtime` + current time.

#[derive(Clone)]
struct JsonlTailEntry {
    mtime: SystemTime,
    len: u64,
    last_ts: Option<String>,
    last_role: String,
}

fn jsonl_tail_cache() -> &'static Mutex<HashMap<String, JsonlTailEntry>> {
    static CACHE: OnceLock<Mutex<HashMap<String, JsonlTailEntry>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Fetches metadata + (cached) tail in a single round-trip when content
/// hasn't changed. Returns a placeholder on metadata failure so callers don't
/// branch on Option.
fn tail_conversation_info(jsonl_path: &Path) -> JsonlTailEntry {
    let meta = match std::fs::metadata(jsonl_path) {
        Ok(m) => m,
        Err(_) => return JsonlTailEntry {
            mtime: SystemTime::UNIX_EPOCH,
            len: 0,
            last_ts: None,
            last_role: "unknown".into(),
        },
    };
    let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    let len = meta.len();
    let key = jsonl_path.to_string_lossy().into_owned();

    if let Ok(cache) = jsonl_tail_cache().lock() {
        if let Some(entry) = cache.get(&key) {
            if entry.mtime == mtime && entry.len == len {
                return entry.clone();
            }
        }
    }

    let (last_ts, last_role) = tail_conversation_status(jsonl_path);
    let entry = JsonlTailEntry { mtime, len, last_ts, last_role };
    if let Ok(mut cache) = jsonl_tail_cache().lock() {
        cache.insert(key, entry.clone());
    }
    entry
}

#[derive(Clone)]
struct ConversationParseEntry {
    mtime: SystemTime,
    len: u64,
    stats: SessionStats,
    recent: Vec<ConversationMessage>,
    git_branch: Option<String>,
    last_role: String,
}

fn conversation_parse_cache() -> &'static Mutex<HashMap<String, ConversationParseEntry>> {
    static CACHE: OnceLock<Mutex<HashMap<String, ConversationParseEntry>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

// --- Discovery ---

/// Find all WSL `.claude` directories by scanning `\\wsl.localhost\<distro>\home\*\.claude\sessions`.
/// Returns `(claude_dir, proc_root)` pairs. Pure file I/O — no process spawn.
pub(crate) fn wsl_claude_dirs() -> Vec<(PathBuf, PathBuf)> {
    let root = PathBuf::from(r"\\wsl.localhost\Ubuntu");
    let proc_root = root.join("proc");
    let home_dir = root.join("home");

    let homes = match std::fs::read_dir(&home_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut results = Vec::new();
    for user in homes.flatten() {
        let claude = user.path().join(".claude");
        if claude.join("sessions").is_dir() {
            results.push((claude, proc_root.clone()));
        }
    }
    results
}

/// Mission Control lists every tmux session on the host so the panel doubles
/// as an "all my terminals" dashboard. MC-spawned sessions (tmux name
/// `claude-<uuid>`) are enriched with JSONL-derived status + subagents;
/// external sessions running `claude` are tagged `claude-external`; everything
/// else is plain `tmux`. The tmux probe is cached at 2s in
/// `state::tmux_sessions`.
#[tauri::command]
pub async fn get_claude_sessions(
    owned_state: tauri::State<'_, OwnedSessionsState>,
    force_refresh: Option<bool>,
) -> Result<Vec<ClaudeSession>, String> {
    if force_refresh.unwrap_or(false) {
        invalidate_tmux_cache();
    }

    let owned = owned_state.list_alive();
    // Index owned entries by their tmux session name so we can decide each
    // tmux row's source in O(1).
    let mut owned_by_tmux: HashMap<String, OwnedSession> = HashMap::with_capacity(owned.len());
    for o in owned {
        owned_by_tmux.insert(o.tmux_session.clone(), o);
    }

    let tmux_rows = list_all_tmux_sessions();
    let mut sessions: Vec<ClaudeSession> = Vec::with_capacity(tmux_rows.len());
    let mut seen_jsonls: std::collections::HashSet<String> = std::collections::HashSet::new();

    for tmux in tmux_rows {
        match owned_by_tmux.remove(&tmux.name) {
            Some(o) => sessions.push(build_mc_session(o, &tmux, &mut seen_jsonls)),
            None => sessions.push(build_external_session(&tmux)),
        }
    }

    // Sort: MC first (newest-first), then claude-external, then tmux
    // (most-recently-created first).
    sessions.sort_by(|a, b| {
        let rank = |s: &ClaudeSession| match s.source {
            SessionSource::Mc => 0,
            SessionSource::ClaudeExternal => 1,
            SessionSource::Tmux => 2,
        };
        rank(a).cmp(&rank(b)).then_with(|| b.started_at.cmp(&a.started_at))
    });

    if let Ok(mut cache) = jsonl_tail_cache().lock() {
        cache.retain(|k, _| seen_jsonls.contains(k));
    }
    if let Ok(mut cache) = conversation_parse_cache().lock() {
        cache.retain(|k, _| seen_jsonls.contains(k));
    }

    Ok(sessions)
}

fn build_mc_session(
    owned: OwnedSession,
    tmux: &TmuxSessionInfo,
    seen_jsonls: &mut std::collections::HashSet<String>,
) -> ClaudeSession {
    let claude_dir = PathBuf::from(&owned.claude_home);
    let projects_dir = claude_dir.join("projects");
    let hash = cwd_to_project_hash(&owned.cwd);
    let jsonl_path = projects_dir.join(&hash).join(format!("{}.jsonl", &owned.session_id));
    seen_jsonls.insert(jsonl_path.to_string_lossy().into_owned());

    let info = tail_conversation_info(&jsonl_path);
    let status = derive_status(&info.last_role, is_stale_at(info.mtime));
    let subagents = read_subagents(&projects_dir, &owned.cwd, &owned.session_id);

    ClaudeSession {
        pid: owned.pid,
        session_id: owned.session_id,
        cwd: owned.cwd,
        started_at: owned.started_at,
        kind: Some("interactive".to_string()),
        name: owned.label.clone(),
        entrypoint: Some("mc".to_string()),
        is_alive: true,
        subagent_count: subagents.len() as u32,
        subagents,
        status,
        last_message_at: info.last_ts,
        label: owned.label,
        worktree_path: owned.worktree_path,
        source: SessionSource::Mc,
        tmux_session_name: tmux.name.clone(),
        running_command: tmux.current_command.clone(),
        current_path: tmux.current_path.clone(),
        attached: tmux.attached,
        window_count: tmux.window_count,
    }
}

fn build_external_session(tmux: &TmuxSessionInfo) -> ClaudeSession {
    let source = classify_external(tmux);
    let cwd = tmux.current_path.clone().unwrap_or_default();
    // tmux `session_created` is epoch seconds; ClaudeSession.started_at is ms.
    let started_at = (tmux.created_at.max(0) as u64) * 1000;
    ClaudeSession {
        pid: tmux.pane_pid.unwrap_or(0).max(0) as u32,
        // Synthetic session_id: prefix lets the frontend / backend route
        // attach + write calls without a separate id namespace.
        session_id: format!("tmux:{}", tmux.name),
        cwd,
        started_at,
        kind: None,
        name: None,
        entrypoint: None,
        is_alive: true,
        subagent_count: 0,
        subagents: Vec::new(),
        // External rows have no JSONL → we can't tell busy/idle. "unknown"
        // keeps the dot neutral.
        status: SessionStatus::Unknown,
        last_message_at: None,
        label: None,
        worktree_path: None,
        source,
        tmux_session_name: tmux.name.clone(),
        running_command: tmux.current_command.clone(),
        current_path: tmux.current_path.clone(),
        attached: tmux.attached,
        window_count: tmux.window_count,
    }
}

/// Decide whether a non-MC tmux session is running claude. `pane_current_command`
/// is checked first; if it's a generic interpreter we follow up with a `ps` on
/// the pane's pid to look for `claude` in argv. Anything else is plain `tmux`.
fn classify_external(tmux: &TmuxSessionInfo) -> SessionSource {
    let cmd = match tmux.current_command.as_deref() {
        Some(c) => c,
        None => return SessionSource::Tmux,
    };
    if cmd == "claude" {
        return SessionSource::ClaudeExternal;
    }
    // The claude CLI is a Node app — when running it the foreground process
    // often shows up as `node`. Same story for python-based wrappers and a
    // bash launcher script. Probe argv on those candidates only.
    let needs_argv_check = matches!(cmd, "node" | "python" | "python3" | "bash" | "sh");
    if needs_argv_check {
        if let Some(pid) = tmux.pane_pid {
            if ps_contains_claude(pid) {
                return SessionSource::ClaudeExternal;
            }
        }
    }
    SessionSource::Tmux
}

// --- Connection statuses ---

const STATUS_QUERY_TIMEOUT: Duration = Duration::from_secs(8);

#[tauri::command]
pub async fn get_connection_statuses() -> Result<Vec<ConnectionStatus>, String> {
    let (current_user, connections) = load_all_connections()?;

    let mut handles = Vec::new();
    for conn in connections {
        let imposter = current_user.clone();
        let label = conn.label.clone();
        let server = conn.server.clone();
        handles.push(tokio::spawn(async move {
            match timeout(STATUS_QUERY_TIMEOUT, async {
                let mut client = db::connect(&server).await?;
                db::check_acting_as(&mut client, &imposter).await
            })
            .await
            {
                Ok(Ok((login, name, is_self))) => ConnectionStatus {
                    label,
                    server,
                    acting_as_login: Some(login),
                    acting_as_name: Some(name),
                    is_self,
                    error: None,
                },
                Ok(Err(e)) => ConnectionStatus {
                    label,
                    server,
                    acting_as_login: None,
                    acting_as_name: None,
                    is_self: false,
                    error: Some(e),
                },
                Err(_) => ConnectionStatus {
                    label,
                    server,
                    acting_as_login: None,
                    acting_as_name: None,
                    is_self: false,
                    error: Some(format!(
                        "Timed out after {}s",
                        STATUS_QUERY_TIMEOUT.as_secs()
                    )),
                },
            }
        }));
    }

    let mut results = Vec::new();
    for handle in handles {
        match handle.await {
            Ok(status) => results.push(status),
            Err(e) => {
                results.push(ConnectionStatus {
                    label: "unknown".into(),
                    server: "unknown".into(),
                    acting_as_login: None,
                    acting_as_name: None,
                    is_self: false,
                    error: Some(format!("Task failed: {e}")),
                });
            }
        }
    }

    Ok(results)
}

// --- Ad-hoc SQL query ---

#[tauri::command]
pub async fn execute_sql_query(
    server: String,
    database: String,
    sql: String,
    query_id: String,
    state: tauri::State<'_, SqlQueryState>,
) -> Result<QueryResult, String> {
    let server = server.trim().to_string();
    let database = database.trim().to_string();
    let sql = sql.trim().to_string();

    if server.is_empty() {
        return Err("Server cannot be empty".into());
    }
    if sql.is_empty() {
        return Err("Query cannot be empty".into());
    }
    if query_id.is_empty() {
        return Err("Query id cannot be empty".into());
    }

    let db_name = if database.is_empty() { "master" } else { &database };

    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
    {
        let mut map = state
            .queries
            .lock()
            .map_err(|e| format!("SQL query state lock poisoned: {e}"))?;
        map.insert(query_id.clone(), cancel_tx);
    }

    let work = async {
        let mut client = db::connect_to(&server, db_name).await?;
        db::execute_query(&mut client, &sql).await
    };

    let outcome: Result<(Vec<String>, Vec<Vec<String>>), String> = tokio::select! {
        biased;
        _ = cancel_rx => Err("Query was cancelled".to_string()),
        res = work => res,
    };

    // Always clear our entry so a stale sender can't be fired against a future query
    // that happens to reuse the same id.
    if let Ok(mut map) = state.queries.lock() {
        map.remove(&query_id);
    }

    let (columns, rows) = outcome?;
    let row_count = rows.len();

    Ok(QueryResult {
        columns,
        rows,
        row_count,
    })
}

#[tauri::command]
pub async fn kill_sql_query(
    query_id: String,
    state: tauri::State<'_, SqlQueryState>,
) -> Result<(), String> {
    let sender = state
        .queries
        .lock()
        .map_err(|e| format!("SQL query state lock poisoned: {e}"))?
        .remove(&query_id);
    if let Some(tx) = sender {
        // Receiver may have already completed normally — ignore send failure.
        let _ = tx.send(());
    }
    Ok(())
}

// --- Session detail ---

/// Truncate a string at a char boundary, appending "..." if truncated.
fn truncate_at_char_boundary(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        return s.to_string();
    }
    let end = s
        .char_indices()
        .map(|(i, _)| i)
        .take_while(|&i| i <= max_len)
        .last()
        .unwrap_or(0);
    format!("{}...", &s[..end])
}

/// Extract a text summary from a message's content field.
/// Content can be a string or an array of content blocks.
fn extract_summary(content: &serde_json::Value, max_len: usize) -> (String, Option<String>) {
    if let Some(s) = content.as_str() {
        return (truncate_at_char_boundary(s.trim(), max_len), None);
    }

    if let Some(arr) = content.as_array() {
        for block in arr {
            let block_type = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
            match block_type {
                "text" => {
                    let text = block.get("text").and_then(|v| v.as_str()).unwrap_or("");
                    return (truncate_at_char_boundary(text.trim(), max_len), None);
                }
                "tool_use" => {
                    let name = block
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    return (format!("[{name}]"), Some(name.to_string()));
                }
                _ => continue,
            }
        }
    }

    (String::new(), None)
}

/// Stream a session's conversation JSONL, collecting stats and recent messages.
fn parse_conversation(
    jsonl_path: &Path,
    max_recent: usize,
) -> (SessionStats, Vec<ConversationMessage>, Option<String>, String) {
    let mut stats = SessionStats::default();
    let mut recent: VecDeque<ConversationMessage> = VecDeque::with_capacity(max_recent + 1);
    let mut git_branch: Option<String> = None;
    let mut last_role = String::new();

    let file = match std::fs::File::open(jsonl_path) {
        Ok(f) => f,
        Err(_) => return (stats, Vec::new(), None, String::new()),
    };

    for line in std::io::BufReader::new(file).lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };

        let val: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let record_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if record_type != "user" && record_type != "assistant" {
            if git_branch.is_none() {
                if let Some(b) = val.get("gitBranch").and_then(|v| v.as_str()) {
                    git_branch = Some(b.to_string());
                }
            }
            continue;
        }

        let msg = match val.get("message") {
            Some(m) => m,
            None => continue,
        };

        let role = msg
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if let Some(b) = val.get("gitBranch").and_then(|v| v.as_str()) {
            git_branch = Some(b.to_string());
        }

        // Accumulate token usage
        if let Some(usage) = msg.get("usage") {
            let input = usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0)
                + usage
                    .get("cache_read_input_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0)
                + usage
                    .get("cache_creation_input_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
            let output = usage.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
            stats.total_input_tokens += input;
            stats.total_output_tokens += output;
        }

        stats.message_count += 1;
        match role.as_str() {
            "user" => stats.user_message_count += 1,
            "assistant" => stats.assistant_message_count += 1,
            _ => {}
        }

        let timestamp = val
            .get("timestamp")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let content = msg.get("content").cloned().unwrap_or(serde_json::Value::Null);
        let (summary, tool_name) = extract_summary(&content, 200);

        if !summary.is_empty() {
            recent.push_back(ConversationMessage {
                role: role.clone(),
                timestamp,
                summary,
                tool_name,
            });
            if recent.len() > max_recent {
                recent.pop_front();
            }
        }

        last_role = role;
    }

    (stats, recent.into(), git_branch, last_role)
}

/// Cached wrapper around `parse_conversation`. Returns the parse result; status
/// must be derived by the caller from `last_role` + current staleness.
fn parse_conversation_cached(
    jsonl_path: &Path,
    max_recent: usize,
) -> ConversationParseEntry {
    let meta = match std::fs::metadata(jsonl_path) {
        Ok(m) => m,
        Err(_) => return ConversationParseEntry {
            mtime: SystemTime::UNIX_EPOCH,
            len: 0,
            stats: SessionStats::default(),
            recent: Vec::new(),
            git_branch: None,
            last_role: String::new(),
        },
    };
    let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    let len = meta.len();
    let key = jsonl_path.to_string_lossy().into_owned();

    if let Ok(cache) = conversation_parse_cache().lock() {
        if let Some(entry) = cache.get(&key) {
            if entry.mtime == mtime && entry.len == len {
                return entry.clone();
            }
        }
    }

    let (stats, recent, git_branch, last_role) = parse_conversation(jsonl_path, max_recent);
    let entry = ConversationParseEntry { mtime, len, stats, recent, git_branch, last_role };
    if let Ok(mut cache) = conversation_parse_cache().lock() {
        cache.insert(key, entry.clone());
    }
    entry
}

#[tauri::command]
pub async fn get_session_detail(
    session_id: String,
    owned_state: tauri::State<'_, OwnedSessionsState>,
    io_state: tauri::State<'_, crate::models::mission_control::ClaudeIoState>,
) -> Result<SessionDetail, String> {
    let owned: OwnedSession = owned_state
        .get(&session_id)
        .ok_or_else(|| format!("Session {session_id} is not tracked by Mission Control"))?;

    let claude_dir = PathBuf::from(&owned.claude_home);
    let projects_dir = claude_dir.join("projects");
    let hash = cwd_to_project_hash(&owned.cwd);
    let jsonl_path = projects_dir.join(&hash).join(format!("{session_id}.jsonl"));

    // Liveness: trust our own ClaudeIoState first — if we're holding the PTY
    // for this session, it's by definition alive even if `tmux has-session`
    // hasn't caught up yet (there's a ~hundreds-of-ms gap between
    // start_new_claude_session returning and bash -ilc actually running
    // `tmux new-session`). Fall back to tmux probe for restored-after-restart
    // sessions where io_state is empty.
    let in_io_state = io_state
        .sessions
        .lock()
        .map(|s| s.contains_key(&owned.session_id))
        .unwrap_or(false);
    let is_alive = in_io_state
        || crate::commands::claude_io::tmux_session_alive(&format!(
            "claude-{}",
            owned.session_id
        ));

    let parsed = parse_conversation_cached(&jsonl_path, 20);
    let status = if !is_alive {
        SessionStatus::Dead
    } else if parsed.mtime == SystemTime::UNIX_EPOCH {
        SessionStatus::Unknown
    } else {
        derive_status(&parsed.last_role, is_stale_at(parsed.mtime))
    };
    let subagents = read_subagents(&projects_dir, &owned.cwd, &session_id);

    Ok(SessionDetail {
        pid: owned.pid,
        session_id,
        cwd: owned.cwd,
        started_at: owned.started_at,
        kind: Some("interactive".to_string()),
        name: owned.label.clone(),
        entrypoint: Some("mc".to_string()),
        is_alive,
        git_branch: parsed.git_branch,
        status,
        stats: parsed.stats,
        recent_messages: parsed.recent,
        subagents,
        label: owned.label,
        worktree_path: owned.worktree_path,
    })
}

#[tauri::command]
pub async fn kill_session(pid: u32) -> Result<(), String> {
    // WSL PIDs can only be signaled from within WSL — one-shot user action
    let out = std::process::Command::new("wsl.exe")
        .args(["-e", "kill", &pid.to_string()])
        .output()
        .map_err(|e| format!("Failed to spawn wsl kill: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("wsl kill {pid} failed (exit {})", out.status)
        } else {
            format!("wsl kill {pid}: {stderr}")
        });
    }
    Ok(())
}


#[tauri::command]
pub async fn open_in_explorer(cwd: String) -> Result<(), String> {
    let windows_path = if let Some(rest) = cwd.strip_prefix("/mnt/") {
        if let Some((drive, path)) = rest.split_once('/') {
            format!("{}:\\{}", drive.to_uppercase(), path.replace('/', "\\"))
        } else {
            format!("{}:\\", rest.to_uppercase())
        }
    } else {
        // Pure Linux path — open via UNC
        format!(r"\\wsl.localhost\Ubuntu{}", cwd.replace('/', "\\"))
    };

    std::process::Command::new("explorer.exe")
        .arg(&windows_path)
        .spawn()
        .map_err(|e| format!("Failed to open explorer: {e}"))?;
    Ok(())
}
