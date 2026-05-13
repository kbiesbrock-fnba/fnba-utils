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

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMessage {
    pub role: String,
    pub timestamp: String,
    pub summary: String,
    pub tool_name: Option<String>,
}

#[derive(Serialize, Default, Clone)]
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

/// Managed state for parallel-resume Claude sidecars driven by the Mission Control chat UI.
///
/// Architecture: each entry is a second `claude --resume <id>` (interactive, no `--print`)
/// running in parallel with whatever already had the session open (e.g. IntelliJ). Both
/// processes append to the same JSONL transcript — that's the documented "two terminals,
/// one session" pattern. We send user input via PTY keystrokes (bracketed-paste wrapped)
/// and render output by tailing the JSONL file, not by parsing the TUI stream.
///
/// Ownership:
/// - `_master`: held to keep the PTY open. Dropping closes the slave → claude exits on SIGHUP.
/// - `_tail_stop`: send-side of a channel watched by the JSONL tail thread. Dropping
///   (or sending) signals the thread to exit.
pub struct ClaudeIoState {
    pub sessions: std::sync::Mutex<std::collections::HashMap<String, ClaudeIoSession>>,
}

pub struct ClaudeIoSession {
    pub writer: Box<dyn std::io::Write + Send>,
    pub _master: Box<dyn portable_pty::MasterPty + Send>,
    pub _tail_stop: std::sync::mpsc::Sender<()>,
}

impl ClaudeIoState {
    pub fn new() -> Self {
        Self {
            sessions: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }
}

/// Managed state for in-flight ad-hoc SQL queries so they can be cancelled.
///
/// Keyed by a frontend-generated `query_id`. The sender fires when the user
/// clicks Cancel; the corresponding `execute_sql_query` task watches the
/// receiver via `tokio::select!` and drops its `SqlClient` (closing the TCP
/// connection, which causes SQL Server to abort the request).
pub struct SqlQueryState {
    pub queries:
        std::sync::Mutex<std::collections::HashMap<String, tokio::sync::oneshot::Sender<()>>>,
}

impl SqlQueryState {
    pub fn new() -> Self {
        Self {
            queries: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }
}
