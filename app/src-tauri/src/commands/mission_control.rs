use crate::models::mission_control::{ClaudeSession, SubagentInfo};
use std::path::{Path, PathBuf};

fn claude_dir() -> Result<PathBuf, String> {
    dirs::home_dir()
        .map(|h| h.join(".claude"))
        .ok_or_else(|| "Could not determine home directory".to_string())
}

fn is_pid_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{}", pid)).exists()
}

/// Convert a cwd like `/mnt/c/dev/fnba-utils` to the project hash
/// format Claude uses: `-mnt-c-dev-fnba-utils` (leading slash stripped,
/// remaining slashes replaced with dashes).
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

#[tauri::command]
pub async fn get_claude_sessions() -> Result<Vec<ClaudeSession>, String> {
    let claude = claude_dir()?;
    let sessions_dir = claude.join("sessions");
    let projects_dir = claude.join("projects");

    let entries = std::fs::read_dir(&sessions_dir)
        .map_err(|e| format!("Cannot read sessions directory: {e}"))?;

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

        let pid = val.get("pid").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        if pid == 0 {
            continue;
        }

        if !is_pid_alive(pid) {
            continue;
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
        let started_at = val.get("startedAt").and_then(|v| v.as_u64()).unwrap_or(0);
        let kind = val
            .get("kind")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let name = val
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let entrypoint = val
            .get("entrypoint")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let subagents = read_subagents(&projects_dir, &cwd, &session_id);
        let subagent_count = subagents.len() as u32;

        sessions.push(ClaudeSession {
            pid,
            session_id,
            cwd,
            started_at,
            kind,
            name,
            entrypoint,
            is_alive: true,
            subagent_count,
            subagents,
        });
    }

    // Most recent first
    sessions.sort_by(|a, b| b.started_at.cmp(&a.started_at));

    Ok(sessions)
}
