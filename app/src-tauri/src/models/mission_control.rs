use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentInfo {
    pub agent_type: String,
    pub description: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeSession {
    pub pid: u32,
    pub session_id: String,
    pub cwd: String,
    pub started_at: u64,
    pub kind: Option<String>,
    pub name: Option<String>,
    pub entrypoint: Option<String>,
    pub is_alive: bool,
    pub subagent_count: u32,
    pub subagents: Vec<SubagentInfo>,
}
