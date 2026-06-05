use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JiraIssue {
    pub key: String,
    pub summary: String,
    pub status: String,
    pub status_category: String,
    pub status_group: StatusGroup,
    pub story_points: Option<f64>,
    pub url: String,
    pub priority: Option<String>,
    pub priority_rank: i32, // lower = higher priority (Highest=1, Lowest=5, unknown=10)
    pub due_date: Option<String>, // YYYY-MM-DD
    pub issue_type: String,
    pub is_bug: bool,
    pub has_checklist: bool,
    /// Raw checklist field text (Smart Checklist syntax). Persisted into the
    /// run_snapshot so the panel can show sub-rows without re-querying Jira.
    pub checklist_text: Option<String>,
    /// Parsed checklist items derived from `checklist_text`.
    pub checklist: Vec<ChecklistItem>,
    /// Include this issue's row in the Teams post / clipboard copy. Opt-in —
    /// defaults to false. The user stars items in the preview to mark them as
    /// "next up"; the backend also auto-stars up to 3 To Do items on each
    /// preview so a fresh user (or someone whose starred items have moved on)
    /// doesn't have to start from zero. Only honored for the To Do group at
    /// format time; other groups always post.
    #[serde(default)]
    pub post_to_teams: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChecklistItem {
    pub text: String,
    pub checked: bool,
    pub is_header: bool,
}

/// Parse Smart Checklist (Titanium / Railsware) text into structured items.
/// Permissive: handles markdown task-list, Railsware `*x`/`*~` shortcuts,
/// `>` and `#` headers, and legacy `*`/`+`/`-` bullets.
pub fn parse_checklist(raw: &str) -> Vec<ChecklistItem> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("> ").or_else(|| trimmed.strip_prefix(">")) {
            out.push(ChecklistItem {
                text: rest.trim().to_string(),
                checked: false,
                is_header: true,
            });
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("# ").or_else(|| trimmed.strip_prefix("#")) {
            out.push(ChecklistItem {
                text: rest.trim().to_string(),
                checked: false,
                is_header: true,
            });
            continue;
        }

        let lowered = trimmed.to_ascii_lowercase();
        if lowered.starts_with("- [x]") || lowered.starts_with("* [x]") {
            out.push(ChecklistItem {
                text: trimmed[5..].trim().to_string(),
                checked: true,
                is_header: false,
            });
            continue;
        }
        if lowered.starts_with("- [ ]") || lowered.starts_with("* [ ]") {
            out.push(ChecklistItem {
                text: trimmed[5..].trim().to_string(),
                checked: false,
                is_header: false,
            });
            continue;
        }
        if lowered.starts_with("*x ")
            || lowered.starts_with("*~ ")
            || lowered.starts_with("+x ")
        {
            out.push(ChecklistItem {
                text: trimmed[3..].trim().to_string(),
                checked: true,
                is_header: false,
            });
            continue;
        }
        if let Some(rest) = trimmed
            .strip_prefix("* ")
            .or_else(|| trimmed.strip_prefix("- "))
        {
            out.push(ChecklistItem {
                text: rest.trim().to_string(),
                checked: false,
                is_header: false,
            });
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("+ ") {
            out.push(ChecklistItem {
                text: rest.trim().to_string(),
                checked: true,
                is_header: false,
            });
            continue;
        }
        if let Some(rest) = trimmed
            .strip_prefix('*')
            .or_else(|| trimmed.strip_prefix('-'))
            .or_else(|| trimmed.strip_prefix('+'))
        {
            let r = rest.trim();
            if !r.is_empty() {
                out.push(ChecklistItem {
                    text: r.to_string(),
                    checked: false,
                    is_header: false,
                });
                continue;
            }
        }

        out.push(ChecklistItem {
            text: trimmed.to_string(),
            checked: false,
            is_header: false,
        });
    }
    out
}

pub fn priority_rank(name: Option<&str>) -> i32 {
    match name.map(|s| s.to_lowercase()) {
        Some(p) if p == "highest" => 1,
        Some(p) if p == "high" => 2,
        Some(p) if p == "medium" => 3,
        Some(p) if p == "low" => 4,
        Some(p) if p == "lowest" => 5,
        _ => 10,
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum StatusGroup {
    InProgress,
    Review,
    Todo,
    Attention,
    Done,
}

impl StatusGroup {
    pub fn label(self) -> &'static str {
        match self {
            StatusGroup::InProgress => "In Progress",
            StatusGroup::Review => "In Review",
            StatusGroup::Todo => "To Do",
            StatusGroup::Attention => "Needs Attention",
            StatusGroup::Done => "Done This Week",
        }
    }

    pub fn emoji(self) -> &'static str {
        match self {
            StatusGroup::InProgress => "💻",
            StatusGroup::Review => "🔍",
            StatusGroup::Todo => "📝",
            StatusGroup::Attention => "🔥",
            StatusGroup::Done => "✅",
        }
    }

    /// Sort order in the panel + Teams card.
    pub fn ordered() -> &'static [StatusGroup] {
        &[
            StatusGroup::InProgress,
            StatusGroup::Review,
            StatusGroup::Todo,
            StatusGroup::Attention,
            StatusGroup::Done,
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StandupGroup {
    pub group: StatusGroup,
    pub label: String,
    pub emoji: String,
    pub issues: Vec<JiraIssue>,
    pub total_points: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StandupReport {
    pub generated_at: String, // RFC3339
    pub issue_count: usize,
    pub groups: Vec<StandupGroup>,
}

/// Result of `preview_standup` and `post_standup_to_teams`.
///
/// `teams_configured` drives whether the frontend's Post button is enabled.
/// `teams_channel_url` is the deep-link the frontend opens after a successful
/// post so Teams pops to the channel — None means leave Teams alone.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StandupRunResult {
    pub report: StandupReport,
    pub posted_to_teams: bool,
    pub copied_to_clipboard: bool,
    pub warnings: Vec<String>,
    pub teams_configured: bool,
    pub teams_channel_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StandupLastRun {
    pub at: String, // RFC3339
    pub issue_count: usize,
    pub posted_to_teams: bool,
    pub error: Option<String>,
}
