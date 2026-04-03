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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMessage {
    pub role: String,
    pub timestamp: String,
    pub summary: String,
    pub tool_name: Option<String>,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionStats {
    pub message_count: u32,
    pub user_message_count: u32,
    pub assistant_message_count: u32,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetail {
    pub pid: u32,
    pub session_id: String,
    pub cwd: String,
    pub started_at: u64,
    pub kind: Option<String>,
    pub name: Option<String>,
    pub entrypoint: Option<String>,
    pub is_alive: bool,
    pub git_branch: Option<String>,
    pub status: String,
    pub stats: SessionStats,
    pub recent_messages: Vec<ConversationMessage>,
    pub subagents: Vec<SubagentInfo>,
}
