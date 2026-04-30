use serde::Serialize;

#[derive(Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Idle,
    Busy,
    Dead,
    Unknown,
}

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
    pub status: SessionStatus,
    pub last_message_at: Option<String>,
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
    pub status: SessionStatus,
    pub stats: SessionStats,
    pub recent_messages: Vec<ConversationMessage>,
    pub subagents: Vec<SubagentInfo>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionStatus {
    pub label: String,
    pub server: String,
    pub acting_as_login: Option<String>,
    pub acting_as_name: Option<String>,
    pub is_self: bool,
    pub error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub row_count: usize,
}

/// Managed state for tracking active PTY sessions.
pub struct PtyState {
    pub sessions: std::sync::Mutex<std::collections::HashMap<String, PtySession>>,
}

pub struct PtySession {
    pub writer: Box<dyn std::io::Write + Send>,
    pub master: Box<dyn portable_pty::MasterPty + Send>,
    pub session_id: String,
    pub cwd: String,
}

impl PtyState {
    pub fn new() -> Self {
        Self {
            sessions: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }
}
