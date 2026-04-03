use crate::models::mission_control::{ClaudeSession, SubagentInfo};
use std::path::{Path, PathBuf};

// --- Shared helpers ---

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
        },
    ))
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
