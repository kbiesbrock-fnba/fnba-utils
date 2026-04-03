use crate::commands::assume_identity::load_all_connections;
use crate::db;
use crate::models::mission_control::{
    ClaudeSession, ConnectionStatus, ConversationMessage, QueryResult, SessionDetail, SessionStats,
    SubagentInfo,
};
use std::collections::VecDeque;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tokio::time::timeout;

// --- Shared helpers ---

/// How long after the last JSONL write we still consider a "user"-last session busy.
/// Claude streams responses within seconds; if the file is older than this the session
/// was interrupted or the user's message was never processed.
const BUSY_STALE_SECS: u64 = 10;

/// True when the file's mtime is older than `BUSY_STALE_SECS`.
fn jsonl_is_stale(path: &Path) -> bool {
    path.metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .map_or(false, |age| age.as_secs() > BUSY_STALE_SECS)
}

fn cwd_to_project_hash(cwd: &str) -> String {
    let trimmed = cwd.strip_prefix('/').unwrap_or(cwd);
    format!("-{}", trimmed.replace('/', "-"))
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
            status: "unknown".into(),
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

    // Seek to the last 8KB (or start of file if smaller)
    let offset = if file_len > 8192 { file_len - 8192 } else { 0 };
    if file.seek(SeekFrom::Start(offset)).is_err() {
        return (None, "unknown".into());
    }

    let mut buf = String::new();
    if file.read_to_string(&mut buf).is_err() {
        return (None, "unknown".into());
    }

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
        if !alive {
            continue;
        }

        let subagents = read_subagents(&projects_dir, &session.cwd, &session.session_id);
        session.subagent_count = subagents.len() as u32;
        session.subagents = subagents;

        // Lightweight tail of JSONL for status + last message time
        let hash = cwd_to_project_hash(&session.cwd);
        let jsonl_path = projects_dir.join(&hash).join(format!("{}.jsonl", &session.session_id));
        let (last_ts, last_role) = tail_conversation_status(&jsonl_path);
        session.status = if !alive {
            "dead"
        } else if last_role == "user" && !jsonl_is_stale(&jsonl_path) {
            "busy"
        } else {
            "idle"
        }
        .to_string();
        session.last_message_at = last_ts;

        sessions.push(session);
    }

    Ok(sessions)
}

#[tauri::command]
pub async fn get_claude_sessions() -> Result<Vec<ClaudeSession>, String> {
    let mut all_sessions = Vec::new();

    // WSL sessions: read via \\wsl.localhost\ UNC paths (pure file I/O, no process spawn)
    for (claude_dir, proc_root) in wsl_claude_dirs() {
        if let Ok(mut s) = discover_sessions(&claude_dir, Some(&proc_root)) {
            all_sessions.append(&mut s);
        }
    }

    // Native Windows sessions (if Claude Code is also installed natively)
    if let Some(home) = dirs::home_dir() {
        let claude = home.join(".claude");
        if let Ok(mut s) = discover_sessions(&claude, None) {
            all_sessions.append(&mut s);
        }
    }

    all_sessions.sort_by(|a, b| b.started_at.cmp(&a.started_at));

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

    let db_name = if database.is_empty() { "master" } else { &database };

    let mut client = db::connect_to(&server, db_name).await?;
    let (columns, rows) = db::execute_query(&mut client, &sql).await?;
    let row_count = rows.len();

    Ok(QueryResult {
        columns,
        rows,
        row_count,
    })
}

// --- Session detail ---

/// Extract a text summary from a message's content field.
/// Content can be a string or an array of content blocks.
fn extract_summary(content: &serde_json::Value, max_len: usize) -> (String, Option<String>) {
    if let Some(s) = content.as_str() {
        let clean = s.trim();
        let truncated = if clean.len() > max_len {
            let end = clean.char_indices().map(|(i, _)| i).take_while(|&i| i <= max_len).last().unwrap_or(0);
            format!("{}...", &clean[..end])
        } else {
            clean.to_string()
        };
        return (truncated, None);
    }

    if let Some(arr) = content.as_array() {
        for block in arr {
            let block_type = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
            match block_type {
                "text" => {
                    let text = block.get("text").and_then(|v| v.as_str()).unwrap_or("");
                    let clean = text.trim();
                    let truncated = if clean.len() > max_len {
                        let end = clean.char_indices().map(|(i, _)| i).take_while(|&i| i <= max_len).last().unwrap_or(0);
                        format!("{}...", &clean[..end])
                    } else {
                        clean.to_string()
                    };
                    return (truncated, None);
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
        Err(_) => return (stats, Vec::new(), None, "unknown".to_string()),
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

    let status = if last_role == "user" && !jsonl_is_stale(jsonl_path) {
        "busy".to_string()
    } else {
        "idle".to_string()
    };

    (stats, recent.into(), git_branch, status)
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
    let (stats, recent_messages, git_branch, status) = parse_conversation(&jsonl_path, 20);

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
        git_branch,
        status,
        stats,
        recent_messages,
        subagents,
    })
}

#[tauri::command]
pub async fn kill_session(pid: u32) -> Result<(), String> {
    // WSL PIDs can only be signaled from within WSL — one-shot user action
    std::process::Command::new("wsl.exe")
        .args(["-e", "kill", &pid.to_string()])
        .output()
        .map_err(|e| format!("Failed to kill session: {e}"))?;
    Ok(())
}

#[tauri::command]
pub async fn send_session_prompt(
    session_id: String,
    cwd: String,
    prompt: String,
) -> Result<(), String> {
    let prompt = prompt.trim().to_string();
    if prompt.is_empty() {
        return Err("Prompt cannot be empty".into());
    }

    // Use `claude --resume` with `-p` (print mode) to send the prompt.
    // Spawned in the background — the JSONL is updated as it runs and
    // the frontend polling picks up changes.
    let mut child = std::process::Command::new("wsl.exe")
        .args([
            "-e", "bash", "-lc",
            &format!(
                "cd '{}' && claude --resume '{}' -p",
                cwd.replace('\'', "'\\''"),
                session_id.replace('\'', "'\\''"),
            ),
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn claude: {e}"))?;

    // Pass prompt via stdin to avoid shell-escaping issues.
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        write!(stdin, "{prompt}").map_err(|e| format!("Write failed: {e}"))?;
    }

    // Don't wait for completion — claude -p can take a while.
    // Spawn a thread to reap the child so it doesn't become a zombie.
    std::thread::spawn(move || {
        let _ = child.wait();
    });

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
