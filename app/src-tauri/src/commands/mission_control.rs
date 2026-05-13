use crate::commands::assume_identity::load_all_connections;
use crate::db;
use crate::models::mission_control::{
    ClaudeIoSession, ClaudeIoState, ClaudeSession, ConnectionStatus, ConversationMessage,
    QueryResult, SessionDetail, SessionStats, SessionStatus, SqlQueryState, SubagentInfo,
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime};
use tauri::{Emitter, Manager};
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

fn cwd_to_project_hash(cwd: &str) -> String {
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

fn parse_session(val: &serde_json::Value) -> Option<(u32, ClaudeSession)> {
    let pid = val.get("pid").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    if pid == 0 {
        return None;
    }

    let session_id = val
        .get("sessionId")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let cwd = val
        .get("cwd")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    Some((
        pid,
        ClaudeSession {
            pid,
            session_id,
            cwd,
            started_at: val.get("startedAt").and_then(|v| v.as_u64()).unwrap_or(0),
            kind: val
                .get("kind")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            name: val
                .get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            entrypoint: val
                .get("entrypoint")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            is_alive: true,
            subagent_count: 0,
            subagents: Vec::new(),
            status: SessionStatus::Unknown,
            last_message_at: None,
        },
    ))
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
fn wsl_claude_dirs() -> Vec<(PathBuf, PathBuf)> {
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

/// Discover sessions under a given `.claude` root, using `proc_root` (if provided)
/// to check whether WSL PIDs are alive via `/proc/<pid>`.
fn discover_sessions(
    claude_dir: &Path,
    proc_root: Option<&Path>,
    seen_jsonls: &mut HashSet<String>,
) -> Result<Vec<ClaudeSession>, String> {
    let sessions_dir = claude_dir.join("sessions");
    let projects_dir = claude_dir.join("projects");

    let entries = match std::fs::read_dir(&sessions_dir) {
        Ok(e) => e,
        Err(_) => return Ok(Vec::new()),
    };

    let mut sessions = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        let contents = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let val: serde_json::Value = match serde_json::from_str(&contents) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let (pid, mut session) = match parse_session(&val) {
            Some(s) => s,
            None => continue,
        };

        // PID-alive check: use /proc via UNC if available, otherwise skip
        let alive = match proc_root {
            Some(root) => root.join(pid.to_string()).exists(),
            None => Path::new(&format!("/proc/{pid}")).exists(),
        };
        session.is_alive = alive;

        if alive {
            let subagents = read_subagents(&projects_dir, &session.cwd, &session.session_id);
            session.subagent_count = subagents.len() as u32;
            session.subagents = subagents;
        }

        // Lightweight tail of JSONL for status + last message time
        let hash = cwd_to_project_hash(&session.cwd);
        let jsonl_path = projects_dir.join(&hash).join(format!("{}.jsonl", &session.session_id));
        seen_jsonls.insert(jsonl_path.to_string_lossy().into_owned());
        let info = tail_conversation_info(&jsonl_path);
        session.status = if !alive {
            SessionStatus::Dead
        } else {
            derive_status(&info.last_role, is_stale_at(info.mtime))
        };
        session.last_message_at = info.last_ts;

        sessions.push(session);
    }

    Ok(sessions)
}

#[tauri::command]
pub async fn get_claude_sessions() -> Result<Vec<ClaudeSession>, String> {
    let mut all_sessions = Vec::new();
    let mut seen_jsonls: HashSet<String> = HashSet::new();

    // WSL sessions: read via \\wsl.localhost\ UNC paths (pure file I/O, no process spawn)
    for (claude_dir, proc_root) in wsl_claude_dirs() {
        if let Ok(mut s) = discover_sessions(&claude_dir, Some(&proc_root), &mut seen_jsonls) {
            all_sessions.append(&mut s);
        }
    }

    // Native Windows sessions (if Claude Code is also installed natively)
    if let Some(home) = dirs::home_dir() {
        let claude = home.join(".claude");
        if let Ok(mut s) = discover_sessions(&claude, None, &mut seen_jsonls) {
            all_sessions.append(&mut s);
        }
    }

    // Filter out dead sessions — only show alive ones
    all_sessions.retain(|s| s.is_alive);
    all_sessions.sort_by(|a, b| b.started_at.cmp(&a.started_at));

    // Prune cache entries for JSONLs we no longer see — caches are otherwise
    // unbounded and grow forever as historical sessions accumulate.
    if let Ok(mut cache) = jsonl_tail_cache().lock() {
        cache.retain(|k, _| seen_jsonls.contains(k));
    }
    if let Ok(mut cache) = conversation_parse_cache().lock() {
        cache.retain(|k, _| seen_jsonls.contains(k));
    }

    Ok(all_sessions)
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

/// Find the claude_dir that contains a given session.
fn find_claude_dir_for_session(pid: u32) -> Option<(PathBuf, Option<PathBuf>)> {
    for (claude_dir, proc_root) in wsl_claude_dirs() {
        if claude_dir.join("sessions").join(format!("{pid}.json")).exists() {
            return Some((claude_dir, Some(proc_root)));
        }
    }
    if let Some(home) = dirs::home_dir() {
        let claude = home.join(".claude");
        if claude.join("sessions").join(format!("{pid}.json")).exists() {
            return Some((claude, None));
        }
    }
    None
}

#[tauri::command]
pub async fn get_session_detail(
    session_id: String,
    cwd: String,
    pid: u32,
) -> Result<SessionDetail, String> {
    let (claude_dir, proc_root) = find_claude_dir_for_session(pid)
        .ok_or_else(|| format!("Session {pid} not found"))?;

    let session_path = claude_dir.join("sessions").join(format!("{pid}.json"));
    let session_json = std::fs::read_to_string(&session_path)
        .map_err(|e| format!("Cannot read session file: {e}"))?;
    let val: serde_json::Value =
        serde_json::from_str(&session_json).map_err(|e| format!("Invalid session JSON: {e}"))?;

    let (_, session) = parse_session(&val).ok_or("Invalid session data")?;

    let is_alive = match &proc_root {
        Some(root) => root.join(pid.to_string()).exists(),
        None => Path::new(&format!("/proc/{pid}")).exists(),
    };

    let projects_dir = claude_dir.join("projects");
    let hash = cwd_to_project_hash(&cwd);
    let jsonl_path = projects_dir.join(&hash).join(format!("{session_id}.jsonl"));
    let parsed = parse_conversation_cached(&jsonl_path, 20);
    let status = if parsed.mtime == SystemTime::UNIX_EPOCH {
        SessionStatus::Unknown
    } else {
        derive_status(&parsed.last_role, is_stale_at(parsed.mtime))
    };

    let subagents = read_subagents(&projects_dir, &cwd, &session_id);

    Ok(SessionDetail {
        pid,
        session_id,
        cwd,
        started_at: session.started_at,
        kind: session.kind,
        name: session.name,
        entrypoint: session.entrypoint,
        is_alive,
        git_branch: parsed.git_branch,
        status,
        stats: parsed.stats,
        recent_messages: parsed.recent,
        subagents,
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

// --- Claude session I/O: parallel-resume + JSONL tail ---
//
// We spawn a *second* `claude --resume <id>` interactively (no --print) inside a PTY,
// running in parallel with whatever already has the session open (e.g. IntelliJ's
// claude plugin). Per the docs ("Same session in multiple terminals: both terminals
// write to the same session file"), both processes append to the same JSONL transcript,
// so anything we send is visible to the original session.
//
// User input goes in via PTY keystrokes wrapped in bracketed-paste escape sequences.
// Output is rendered by *tailing the JSONL file*, not by parsing the TUI output —
// JSONL records are already structured (user/assistant/tool_use/tool_result/usage)
// and avoid the ANSI parsing nightmare of scraping the terminal.

/// Phase 1: pre-approve the common interactive tool set so the parallel claude never
/// blocks on a permission prompt. Phase 2 will swap this for a `--permission-prompt-tool`
/// MCP bridge that surfaces approvals in the chat UI.
const ALLOWED_TOOLS_PHASE1: &str =
    "Read,Edit,Write,Bash,Glob,Grep,WebFetch,WebSearch,TodoWrite,NotebookEdit";

/// Mark `cwd` as trusted in `~/.claude.json` so the spawned claude doesn't show the
/// "Quick safety check: Is this a project you trust?" dialog at startup. Without this,
/// the dialog blocks input — and when send_claude_message fires its bracketed-paste
/// + Enter, the Enter accepts the dialog while the paste content is silently dropped.
///
/// The trust flag is per-cwd; setting it here is the same effect as the user clicking
/// "Yes, I trust this folder" once. Best-effort: if we can't find or update the file,
/// the dialog will appear and the first send will dismiss it (subsequent sends work).
fn ensure_workspace_trust(cwd: &str) {
    // Build candidate `.claude.json` paths. WSL homes are under \\wsl.localhost\Ubuntu\home\*;
    // also probe the native Windows home as a fallback for natively-installed claude.
    // When cwd is a WSL path like /home/<user>/..., scope to that user's home so we
    // don't flip trust flags in another WSL user's config on multi-user hosts.
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

    for path in candidates {
        if !path.exists() {
            continue;
        }
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
            return;
        }
        if let Some(obj) = project.as_object_mut() {
            obj.insert(
                "hasTrustDialogAccepted".to_string(),
                serde_json::Value::Bool(true),
            );
        }
        if let Ok(new_contents) = serde_json::to_string_pretty(&config) {
            let _ = std::fs::write(&path, new_contents);
        }
        return;
    }
}

/// Convert a JSONL conversation record (the file format claude writes to disk) into
/// the same envelope shape ChatPane.vue already consumes from the SDK stream-json
/// path. Returns `None` for noise records (custom-title, agent-name, summary, etc.)
/// that shouldn't surface in the chat.
fn jsonl_to_event(record: &serde_json::Value) -> Option<serde_json::Value> {
    let record_type = record.get("type").and_then(|v| v.as_str())?;
    match record_type {
        "user" => {
            // Skip meta echoes (claude writes user records with isMeta=true for
            // command/permission scaffolding that aren't real user turns).
            if record
                .get("isMeta")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                return None;
            }
            let message = record.get("message").cloned().unwrap_or(serde_json::Value::Null);
            Some(serde_json::json!({ "type": "user", "message": message }))
        }
        "assistant" => {
            let message = record.get("message").cloned().unwrap_or(serde_json::Value::Null);
            Some(serde_json::json!({ "type": "assistant", "message": message }))
        }
        "system" => {
            // JSONL system records carry their own subtype field; pass-through.
            Some(record.clone())
        }
        // Noise records — file metadata, not user-facing chat events.
        "summary" | "custom-title" | "agent-name" | "permission-mode" | "pr-link" => None,
        _ => None,
    }
}

#[tauri::command]
pub async fn start_claude_session(
    app: tauri::AppHandle,
    session_id: String,
    cwd: String,
    state: tauri::State<'_, ClaudeIoState>,
) -> Result<(), String> {
    use portable_pty::{native_pty_system, CommandBuilder, PtySize};

    if state
        .sessions
        .lock()
        .map_err(|e| format!("State lock poisoned: {e}"))?
        .contains_key(&session_id)
    {
        return Err("Claude session already open".into());
    }

    // Resolve JSONL transcript path. Tauri runs on Windows so `dirs::home_dir()` is
    // useless here — WSL sessions live under `\\wsl.localhost\Ubuntu\home\...\.claude`.
    // Scan the same set of claude dirs that discovery uses, then probe for the file.
    let jsonl_path = {
        let hash = cwd_to_project_hash(&cwd);
        let filename = format!("{session_id}.jsonl");
        let mut found = None;
        for (claude_dir, _) in wsl_claude_dirs() {
            let p = claude_dir.join("projects").join(&hash).join(&filename);
            if p.exists() {
                found = Some(p);
                break;
            }
        }
        if found.is_none() {
            if let Some(home) = dirs::home_dir() {
                let p = home.join(".claude").join("projects").join(&hash).join(&filename);
                if p.exists() {
                    found = Some(p);
                }
            }
        }
        found.ok_or_else(|| format!("Could not locate JSONL for session {session_id}"))?
    };
    // Tail starts from current size so we don't re-emit history (the panel already
    // loaded that via get_session_detail).
    let baseline_offset = std::fs::metadata(&jsonl_path)
        .map(|m| m.len())
        .unwrap_or(0);

    // Pre-accept the workspace trust dialog so it doesn't intercept our first input.
    ensure_workspace_trust(&cwd);

    // Spawn parallel claude in PTY. No --print, no stream-json flags — interactive
    // resume is what avoids the sdk-cli fork. -ilc keeps NODE_EXTRA_CA_CERTS etc.
    //
    // Why `cd '...' &&` AND `wsl.exe --cd`: --cd alone isn't enough. `bash -i` runs
    // the user's interactive bashrc, which can chdir away (e.g. some bashrc snippets
    // start in $HOME). Claude resolves its session by hashing process.cwd() — if it
    // doesn't match the original, claude can't find the JSONL and silently starts a
    // brand-new session with the same name. The explicit `cd` runs after bashrc and
    // pins us back to the right directory before claude starts.
    let bash_cmd = format!(
        "cd '{cwd_escaped}' && claude --resume '{sid}' --permission-mode acceptEdits --allowedTools {tools}",
        cwd_escaped = cwd.replace('\'', "'\\''"),
        sid = session_id.replace('\'', "'\\''"),
        tools = ALLOWED_TOOLS_PHASE1,
    );

    let pair = native_pty_system()
        .openpty(PtySize {
            rows: 50,
            cols: 200,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Failed to open PTY: {e}"))?;

    let mut cmd = CommandBuilder::new("wsl.exe");
    cmd.args(["--cd", &cwd, "-e", "bash", "-ilc", &bash_cmd]);

    let _child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("Failed to spawn claude in PTY: {e}"))?;
    drop(pair.slave);

    // PTY output drain. We don't render the TUI in chat (the JSONL is the source of
    // truth for actual conversation), but we need to drain to avoid backpressure AND
    // we surface chunks as debug `pty` events so the Debug toggle in the chat panel
    // can show what claude is actually doing — invaluable for diagnosing "no response"
    // situations (rate limit, auth error, prompt-but-not-submitted, etc.).
    // EOF = claude exited → emit session-closed and clean up state.
    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("Failed to clone PTY reader: {e}"))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|e| format!("Failed to take PTY writer: {e}"))?;

    // Channel for stopping the JSONL tail thread. Created up front so the sender can
    // be moved into the state entry below (alongside writer/master) BEFORE either
    // worker thread is spawned — otherwise a near-instant claude exit could fire the
    // drain thread's cleanup before this fn even reaches the insert, leaving a phantom
    // entry whose tail thread polls forever.
    let (tail_stop_tx, tail_stop_rx) = std::sync::mpsc::channel::<()>();
    state
        .sessions
        .lock()
        .map_err(|e| format!("State lock poisoned: {e}"))?
        .insert(
            session_id.clone(),
            ClaudeIoSession {
                writer,
                _master: pair.master,
                _tail_stop: tail_stop_tx,
            },
        );

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
                        // Surface as a debug event. Lossy decode keeps non-UTF-8 bytes
                        // visible (as U+FFFD) instead of dropping them.
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
            let _ = app.emit(
                "claude-session-closed",
                serde_json::json!({ "sessionId": sid, "exitCode": 0 }),
            );
            if let Ok(mut sessions) = app.state::<ClaudeIoState>().sessions.lock() {
                sessions.remove(&sid);
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
            // Carry-over for partial last lines between polls (rare but possible).
            let mut carry = String::new();
            loop {
                if matches!(
                    tail_stop_rx.recv_timeout(Duration::from_millis(200)),
                    Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected)
                ) {
                    break;
                }
                let len = match std::fs::metadata(&path) {
                    Ok(m) => m.len(),
                    Err(_) => continue, // file may not exist yet
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
                // Use bytes-actually-read, not metadata len — the file can grow between
                // the metadata() call and read_to_end, so seeking to `len` next round
                // would silently skip the new tail.
                offset += read as u64;

                let chunk = String::from_utf8_lossy(&buf);
                let combined = format!("{carry}{chunk}");
                carry.clear();

                // split('\n') on "a\nb\n" yields ["a","b",""]; on "a\nb" yields ["a","b"].
                // Either way, the last element is the carry-over (empty if the chunk ended
                // on a newline, the partial line otherwise).
                let mut lines: Vec<&str> = combined.split('\n').collect();
                if let Some(tail) = lines.pop() {
                    carry.push_str(tail);
                }

                for line in lines {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let record: serde_json::Value = match serde_json::from_str(trimmed) {
                        Ok(v) => v,
                        Err(_) => continue, // skip malformed lines (partial flush, etc.)
                    };
                    if let Some(event) = jsonl_to_event(&record) {
                        let _ = app.emit(
                            "claude-event",
                            serde_json::json!({ "sessionId": sid, "event": event }),
                        );
                    }
                }
            }
        });
    }

    Ok(())
}

#[tauri::command]
pub async fn send_claude_message(
    session_id: String,
    content: String,
    state: tauri::State<'_, ClaudeIoState>,
) -> Result<(), String> {
    // Bracketed paste: prefix \x1b[200~ and suffix \x1b[201~, then submit with \r.
    // Bracketed-paste mode is enabled at claude startup ([?2004h in the TUI init);
    // wrapping like this is what xterms do when pasting and is the safest way to
    // inject multi-line text without claude's input handler treating chord-like
    // characters specially.
    // Strip embedded paste-end markers — if the user's content contains \x1b[201~
    // claude's TUI would exit paste mode mid-message and treat the remainder as raw
    // keystrokes. Removing the marker is safe (it never carries meaningful payload).
    let safe_content = content.replace("\x1b[201~", "");

    let mut payload = String::with_capacity(safe_content.len() + 16);
    payload.push_str("\x1b[200~");
    payload.push_str(&safe_content);
    payload.push_str("\x1b[201~");
    payload.push('\r');

    let mut sessions = state
        .sessions
        .lock()
        .map_err(|e| format!("State lock poisoned: {e}"))?;
    let session = sessions
        .get_mut(&session_id)
        .ok_or("No active Claude session")?;
    session
        .writer
        .write_all(payload.as_bytes())
        .map_err(|e| format!("Write failed: {e}"))?;
    session
        .writer
        .flush()
        .map_err(|e| format!("Flush failed: {e}"))?;
    Ok(())
}

#[tauri::command]
pub async fn stop_claude_session(
    session_id: String,
    state: tauri::State<'_, ClaudeIoState>,
) -> Result<(), String> {
    // Dropping the entry drops `_master` (PTY slave closes → claude exits via SIGHUP)
    // and `_tail_stop` (tail thread sees Disconnected on its recv → exits).
    state
        .sessions
        .lock()
        .map_err(|e| format!("State lock poisoned: {e}"))?
        .remove(&session_id);
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
