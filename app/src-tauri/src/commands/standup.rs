use crate::config::AppConfig;
use crate::models::standup::{
    parse_checklist, priority_rank, ChecklistItem, JiraIssue, StandupGroup, StandupLastRun,
    StandupReport, StandupRunResult, StatusGroup,
};
use base64::Engine;
use chrono::{Datelike, Local, Utc};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;

const STATUS_EMOJIS: &[(&str, &str)] = &[
    ("backlog", "📦"),
    ("selected for development", "📝"),
    ("specify", "📐"),
    ("track", "👁️"),
    ("spec review", "📋"),
    ("specify done", "📐"),
    ("implement", "💻"),
    ("in progress", "💻"),
    ("investigate", "🔎"),
    ("design wip", "🎨"),
    ("revisions pending", "🔄"),
    ("ready to review", "👀"),
    ("ready for review", "👀"),
    ("review", "🔍"),
    ("in review", "🔍"),
    ("ready to validate", "✔️"),
    ("validate", "✅"),
    ("ready for testing", "🧪"),
    ("in testing", "🧪"),
    ("testing", "🧪"),
    ("ready for acceptance", "✔️"),
    ("ready to release", "🚀"),
    ("done", "✅"),
    ("servicing top 10", "🔥"),
    ("origination top 10", "🔥"),
];

const STATUS_TO_GROUP: &[(&str, StatusGroup)] = &[
    ("spec review", StatusGroup::InProgress),
    ("specify done", StatusGroup::InProgress),
    ("implement", StatusGroup::InProgress),
    ("in progress", StatusGroup::InProgress),
    ("investigate", StatusGroup::InProgress),
    ("design wip", StatusGroup::InProgress),
    ("revisions pending", StatusGroup::InProgress),
    ("ready to review", StatusGroup::Review),
    ("ready for review", StatusGroup::Review),
    ("review", StatusGroup::Review),
    ("in review", StatusGroup::Review),
    ("ready to validate", StatusGroup::Review),
    ("validate", StatusGroup::Review),
    ("validate done", StatusGroup::Review),
    ("ready for testing", StatusGroup::Review),
    ("in testing", StatusGroup::Review),
    ("testing", StatusGroup::Review),
    ("ready for acceptance", StatusGroup::Review),
    ("selected for development", StatusGroup::Todo),
    ("specify", StatusGroup::Todo),
    ("backlog", StatusGroup::Todo),
    ("track", StatusGroup::Todo),
    ("servicing top 10", StatusGroup::Attention),
    ("origination top 10", StatusGroup::Attention),
    ("ready to release", StatusGroup::Done),
    ("done", StatusGroup::Done),
];

fn status_emoji(status: &str) -> &'static str {
    let lower = status.to_lowercase();
    STATUS_EMOJIS
        .iter()
        .find(|(s, _)| *s == lower)
        .map(|(_, e)| *e)
        .unwrap_or("⚪")
}

fn status_to_group(status: &str) -> StatusGroup {
    let lower = status.to_lowercase();
    STATUS_TO_GROUP
        .iter()
        .find(|(s, _)| *s == lower)
        .map(|(_, g)| *g)
        .unwrap_or(StatusGroup::Todo)
}

fn standup_config(cfg: &AppConfig) -> Result<&crate::config::StandupConfig, String> {
    let s = cfg
        .standup
        .as_ref()
        .ok_or_else(|| {
            "Standup is not configured (%LOCALAPPDATA%/fnba-utils/config.yaml missing)"
                .to_string()
        })?;
    if !s.enabled {
        return Err("Standup is disabled in config (set standup.enabled: true)".to_string());
    }
    let email = s.jira_email.as_deref().unwrap_or("");
    let token = s.jira_api_token.as_deref().unwrap_or("");
    if email.is_empty() || token.is_empty() {
        return Err("Standup config missing jira_email or jira_api_token".to_string());
    }
    Ok(s)
}

async fn fetch_issues(s: &crate::config::StandupConfig) -> Result<Vec<JiraIssue>, String> {
    let email = s.jira_email.as_deref().unwrap_or("");
    let token = s.jira_api_token.as_deref().unwrap_or("");
    let auth = format!("{}:{}", email, token);
    let auth_b64 = base64::engine::general_purpose::STANDARD.encode(auth.as_bytes());

    let url = format!("https://{}/rest/api/3/search/jql", s.jira_domain);
    let jql = "assignee = currentUser() AND (statusCategory != Done OR statusCategoryChangedDate >= startOfWeek())";

    let client = reqwest::Client::builder()
        .build()
        .map_err(|e| format!("reqwest build failed: {e}"))?;

    let resp = client
        .get(&url)
        .header("Authorization", format!("Basic {}", auth_b64))
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .query(&[
            ("jql", jql),
            (
                "fields",
                // The checklist field is included so we can flag rows with a non-empty
                // Smart Checklist payload in the panel — without doing a per-issue
                // get_issue_detail round trip.
                "summary,status,customfield_10028,priority,duedate,issuetype,customfield_13097",
            ),
            ("maxResults", "50"),
        ])
        .send()
        .await
        .map_err(|e| format!("Jira request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Jira API error: {} - {}", status, body));
    }

    let payload: Value = resp
        .json()
        .await
        .map_err(|e| format!("Jira response parse failed: {e}"))?;

    let issues = payload
        .get("issues")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Jira response missing 'issues' array".to_string())?;

    let mut out = Vec::with_capacity(issues.len());
    for issue in issues {
        let key = issue.get("key").and_then(|v| v.as_str()).unwrap_or("");
        let fields = issue.get("fields").cloned().unwrap_or(Value::Null);
        let summary = fields.get("summary").and_then(|v| v.as_str()).unwrap_or("");
        let status_obj = fields.get("status").cloned().unwrap_or(Value::Null);
        let status_name = status_obj.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let status_cat = status_obj
            .get("statusCategory")
            .and_then(|v| v.get("key"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let story_points = fields
            .get("customfield_10028")
            .and_then(|v| v.as_f64());
        let priority = fields
            .get("priority")
            .and_then(|v| v.get("name"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let due_date = fields
            .get("duedate")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        let issue_type = fields
            .get("issuetype")
            .and_then(|v| v.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("Task")
            .to_string();
        let is_bug = issue_type.eq_ignore_ascii_case("bug");
        let p_rank = priority_rank(priority.as_deref());

        let checklist_text: Option<String> = fields
            .get(CHECKLIST_FIELD_ID)
            .and_then(extract_checklist_text)
            .filter(|s| !s.trim().is_empty());
        let checklist = checklist_text
            .as_deref()
            .map(parse_checklist)
            .unwrap_or_default();
        let has_checklist = !checklist.is_empty();

        out.push(JiraIssue {
            key: key.to_string(),
            summary: summary.to_string(),
            status: status_name.to_string(),
            status_category: status_cat.to_string(),
            status_group: status_to_group(status_name),
            story_points,
            url: format!("https://{}/browse/{}", s.jira_domain, key),
            priority,
            priority_rank: p_rank,
            due_date,
            issue_type,
            is_bug,
            has_checklist,
            checklist_text,
            checklist,
        });
    }

    Ok(out)
}

fn build_report(issues: Vec<JiraIssue>) -> StandupReport {
    let mut by_group: HashMap<StatusGroup, Vec<JiraIssue>> = HashMap::new();
    for issue in issues.iter() {
        by_group
            .entry(issue.status_group)
            .or_default()
            .push(issue.clone());
    }

    let mut groups = Vec::new();
    for g in StatusGroup::ordered() {
        let bucket = by_group.remove(g).unwrap_or_default();
        if bucket.is_empty() {
            continue;
        }
        let total_points: f64 = bucket.iter().filter_map(|i| i.story_points).sum();
        groups.push(StandupGroup {
            group: *g,
            label: g.label().to_string(),
            emoji: g.emoji().to_string(),
            issues: bucket,
            total_points,
        });
    }

    StandupReport {
        generated_at: Utc::now().to_rfc3339(),
        issue_count: issues.len(),
        groups,
    }
}

/// Build a compact Adaptive Card: a single markdown `TextBlock`.
///
/// The configured Teams webhook is a Power Automate Workflows endpoint, which only
/// accepts an `AdaptiveCard` envelope — so we keep the wrapper but collapse the body to
/// one markdown block (bold group headers, `[KEY](url)` links, emoji icons). No colored
/// containers or per-issue ColumnSets; it renders as a plain compact markdown message
/// rather than card chrome.
///
/// Layout (grouped headings, points appended):
/// ```text
///   🗓 **Standup — <date>** · N issues · P pts
///
///   💻 **In Progress** (n)
///   - [KEY](url) summary · X pt
///   ...
/// ```
///
/// Line breaks: Teams `TextBlock` markdown ignores a bare `\n`, so groups are separated
/// by `\n\n` (paragraph break) and the bullets within a group by `\r` (soft break).
fn build_adaptive_card(report: &StandupReport) -> Value {
    let now = Local::now();
    let date_label = format!(
        "{}, {} {}",
        weekday_label(now.weekday()),
        month_label(now.month()),
        now.day()
    );

    let total_points: f64 = report.groups.iter().map(|g| g.total_points).sum();
    let visible_count: usize = report
        .groups
        .iter()
        .filter(|g| g.group != StatusGroup::Attention)
        .map(|g| g.issues.len())
        .sum();

    let mut md = format!(
        "🗓 **Standup — {}** · {} issue{} · {} pt{}",
        date_label,
        visible_count,
        if visible_count == 1 { "" } else { "s" },
        format_points(total_points),
        if (total_points - 1.0).abs() < f64::EPSILON { "" } else { "s" },
    );

    for group in &report.groups {
        if group.group == StatusGroup::Attention {
            continue; // panel-only, not surfaced in Teams
        }

        md.push_str("\n\n");
        md.push_str(&format!(
            "{} **{}** ({})",
            group.emoji,
            group.label,
            group.issues.len()
        ));

        for issue in &group.issues {
            md.push('\r');
            md.push_str(&format!("- [{}]({}) {}", issue.key, issue.url, issue.summary));
            if let Some(pts) = issue.story_points {
                md.push_str(&format!(" · {} pt", format_points(pts)));
            }
        }
    }

    json!({
        "$schema": "http://adaptivecards.io/schemas/adaptive-card.json",
        "type": "AdaptiveCard",
        "version": "1.4",
        "body": [{
            "type": "TextBlock",
            "text": md,
            "wrap": true,
        }],
    })
}

fn format_points(pts: f64) -> String {
    if pts.fract().abs() < f64::EPSILON {
        format!("{}", pts as i64)
    } else {
        format!("{:.1}", pts)
    }
}

fn weekday_label(d: chrono::Weekday) -> &'static str {
    match d {
        chrono::Weekday::Mon => "Mon",
        chrono::Weekday::Tue => "Tue",
        chrono::Weekday::Wed => "Wed",
        chrono::Weekday::Thu => "Thu",
        chrono::Weekday::Fri => "Fri",
        chrono::Weekday::Sat => "Sat",
        chrono::Weekday::Sun => "Sun",
    }
}

fn month_label(m: u32) -> &'static str {
    const NAMES: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    NAMES[(m.saturating_sub(1) as usize).min(11)]
}

async fn post_to_teams(webhook: &str, card: &Value) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .build()
        .map_err(|e| format!("reqwest build failed: {e}"))?;
    let resp = client
        .post(webhook)
        .header("Content-Type", "application/json")
        .json(card)
        .send()
        .await
        .map_err(|e| format!("Teams request failed: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Teams webhook error: {} - {}", status, body));
    }
    Ok(())
}

fn last_run_path() -> Option<PathBuf> {
    Some(crate::state::paths::data_file("standup-last-run.json"))
}

fn save_last_run(record: &StandupLastRun) {
    let Some(path) = last_run_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(record) {
        let _ = std::fs::write(&path, json);
    }
}

#[tauri::command]
pub async fn get_standup_last_run() -> Result<Option<StandupLastRun>, String> {
    let Some(path) = last_run_path() else { return Ok(None) };
    match std::fs::read_to_string(&path) {
        Ok(s) => match serde_json::from_str::<StandupLastRun>(&s) {
            Ok(rec) => Ok(Some(rec)),
            Err(e) => Err(format!("last-run parse error: {e}")),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("last-run read error: {e}")),
    }
}

/// Read-only fetch: pulls from Jira and groups, but does not post to Teams or touch last-run.
#[tauri::command]
pub async fn get_standup_report(
    cfg: tauri::State<'_, AppConfig>,
) -> Result<StandupReport, String> {
    let s = standup_config(&cfg)?;
    let issues = fetch_issues(s).await?;
    Ok(build_report(issues))
}

/// Full run: fetch, build card, post to Teams (if configured), copy to clipboard, stamp last-run.
#[tauri::command]
pub async fn run_standup(
    app: tauri::AppHandle,
    cfg: tauri::State<'_, AppConfig>,
    post_to_teams_flag: bool,
) -> Result<StandupRunResult, String> {
    let s = standup_config(&cfg)?;

    let result = run_standup_inner(s, post_to_teams_flag).await;

    let last_run = match &result {
        Ok(r) => StandupLastRun {
            at: r.report.generated_at.clone(),
            issue_count: r.report.issue_count,
            posted_to_teams: r.posted_to_teams,
            error: None,
        },
        Err(e) => StandupLastRun {
            at: Utc::now().to_rfc3339(),
            issue_count: 0,
            posted_to_teams: false,
            error: Some(e.clone()),
        },
    };
    save_last_run(&last_run);

    // Persist the run into SQLite for the panel. DB errors are non-fatal — log + continue.
    if let Ok(mut db) = crate::standup_db::StandupDb::open() {
        match &result {
            Ok(r) => {
                if let Err(e) = db.record_run(&r.report, r.posted_to_teams, None) {
                    eprintln!("standup_db: record_run failed: {e}");
                }
                let keys: Vec<String> = r
                    .report
                    .groups
                    .iter()
                    .flat_map(|g| g.issues.iter().map(|i| i.key.clone()))
                    .collect();
                if let Err(e) = db.mark_seen(&keys) {
                    eprintln!("standup_db: mark_seen failed: {e}");
                }
            }
            Err(e) => {
                // Synthesize a minimal failed-run record so the history shows it.
                let empty = StandupReport {
                    generated_at: Utc::now().to_rfc3339(),
                    issue_count: 0,
                    groups: Vec::new(),
                };
                if let Err(e2) = db.record_run(&empty, false, Some(e.as_str())) {
                    eprintln!("standup_db: record_run (error path) failed: {e2}");
                }
            }
        }
    } else {
        eprintln!("standup_db: could not open database (run not recorded)");
    }

    // Notify any open panel windows to refresh.
    use tauri::Emitter;
    let _ = app.emit("standup-updated", ());

    result
}

async fn run_standup_inner(
    s: &crate::config::StandupConfig,
    post_to_teams_flag: bool,
) -> Result<StandupRunResult, String> {
    let issues = fetch_issues(s).await?;
    let report = build_report(issues);

    let mut warnings: Vec<String> = Vec::new();
    let mut posted = false;

    if post_to_teams_flag {
        match s.teams_webhook_url.as_deref() {
            Some(url) if !url.is_empty() => {
                let card = build_adaptive_card(&report);
                match post_to_teams(url, &card).await {
                    Ok(()) => posted = true,
                    Err(e) => warnings.push(format!("Teams post failed: {e}")),
                }
            }
            _ => warnings.push("Teams webhook not configured; skipped post".to_string()),
        }
    }

    let copied = match copy_report_to_clipboard(&report) {
        Ok(()) => true,
        Err(e) => {
            warnings.push(format!("Clipboard copy failed: {e}"));
            false
        }
    };

    Ok(StandupRunResult {
        report,
        posted_to_teams: posted,
        copied_to_clipboard: copied,
        warnings,
        teams_configured: s
            .teams_webhook_url
            .as_deref()
            .is_some_and(|v| !v.is_empty()),
        teams_channel_url: s
            .teams_channel_url
            .as_deref()
            .filter(|v| !v.is_empty())
            .map(|v| v.to_string()),
    })
}

/// Preview-flow command: fetch + clipboard + record as preview (posted_to_teams=false).
/// Does NOT post to Teams. The frontend renders this, then calls `post_standup_to_teams`
/// with the same `StandupReport` if the user clicks Post.
#[tauri::command]
pub async fn preview_standup(
    app: tauri::AppHandle,
    cfg: tauri::State<'_, AppConfig>,
) -> Result<StandupRunResult, String> {
    let s = standup_config(&cfg)?;
    let issues = fetch_issues(s).await?;
    let report = build_report(issues);

    let mut warnings: Vec<String> = Vec::new();
    let copied = match copy_report_to_clipboard(&report) {
        Ok(()) => true,
        Err(e) => {
            warnings.push(format!("Clipboard copy failed: {e}"));
            false
        }
    };

    // Record the preview run so the always-on-top panel sees fresh data.
    // posted_to_teams=false here; `post_standup_to_teams` flips the row if the
    // user actually posts. Best-effort: DB errors don't fail the preview.
    if let Ok(mut db) = crate::standup_db::StandupDb::open() {
        if let Err(e) = db.record_run(&report, false, None) {
            eprintln!("standup_db: record_run (preview) failed: {e}");
        }
        let keys: Vec<String> = report
            .groups
            .iter()
            .flat_map(|g| g.issues.iter().map(|i| i.key.clone()))
            .collect();
        if let Err(e) = db.mark_seen(&keys) {
            eprintln!("standup_db: mark_seen failed: {e}");
        }
    }

    use tauri::Emitter;
    let _ = app.emit("standup-updated", ());

    Ok(StandupRunResult {
        report,
        posted_to_teams: false,
        copied_to_clipboard: copied,
        warnings,
        teams_configured: s
            .teams_webhook_url
            .as_deref()
            .is_some_and(|v| !v.is_empty()),
        teams_channel_url: s
            .teams_channel_url
            .as_deref()
            .filter(|v| !v.is_empty())
            .map(|v| v.to_string()),
    })
}

/// Post the *previewed* report to Teams.
///
/// Frontend echoes back the `StandupReport` it just rendered so the post matches
/// exactly what the user saw — no re-fetch. On success, flips the existing
/// preview row's `posted_to_teams` flag (keyed by `report.generated_at`) and
/// stamps the last-run JSON. If no preview row exists for that timestamp
/// (e.g. DB was cleared between preview and post), inserts a fresh row.
#[tauri::command]
pub async fn post_standup_to_teams(
    app: tauri::AppHandle,
    cfg: tauri::State<'_, AppConfig>,
    report: StandupReport,
) -> Result<StandupRunResult, String> {
    let s = standup_config(&cfg)?;

    let webhook = s
        .teams_webhook_url
        .as_deref()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| {
            "Teams webhook not configured (set standup.teams_webhook_url in config.yaml)"
                .to_string()
        })?;

    let card = build_adaptive_card(&report);
    post_to_teams(webhook, &card).await?;

    // Update the preview row (or insert one if the preview didn't make it to disk).
    if let Ok(mut db) = crate::standup_db::StandupDb::open() {
        match db.mark_run_posted(&report.generated_at) {
            Ok(0) => {
                // No matching preview row — insert one now so history reflects the post.
                if let Err(e) = db.record_run(&report, true, None) {
                    eprintln!("standup_db: record_run (post fallback) failed: {e}");
                }
            }
            Ok(_) => {}
            Err(e) => eprintln!("standup_db: mark_run_posted failed: {e}"),
        }
    } else {
        eprintln!("standup_db: could not open database (post not recorded)");
    }

    let last_run = StandupLastRun {
        at: report.generated_at.clone(),
        issue_count: report.issue_count,
        posted_to_teams: true,
        error: None,
    };
    save_last_run(&last_run);

    use tauri::Emitter;
    let _ = app.emit("standup-updated", ());

    Ok(StandupRunResult {
        report,
        posted_to_teams: true,
        copied_to_clipboard: false, // clipboard already done at preview time
        warnings: Vec::new(),
        teams_configured: true,
        teams_channel_url: s
            .teams_channel_url
            .as_deref()
            .filter(|v| !v.is_empty())
            .map(|v| v.to_string()),
    })
}

fn copy_report_to_clipboard(report: &StandupReport) -> Result<(), String> {
    let mut text = String::new();
    for group in &report.groups {
        if group.group == StatusGroup::Attention {
            continue;
        }
        text.push_str(&format!(
            "{} {} ({})\n",
            group.emoji, group.label, group.issues.len()
        ));
        for issue in &group.issues {
            let pts = issue
                .story_points
                .map(format_points)
                .unwrap_or_else(|| "—".to_string());
            text.push_str(&format!(
                "  [{}] {}: {} {} ({})\n",
                issue.key, issue.summary, status_emoji(&issue.status), issue.status, pts
            ));
        }
        text.push('\n');
    }

    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.set_text(text).map_err(|e| e.to_string())?;
    Ok(())
}

// --- Panel commands (SQLite-backed) ---

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StandupPanelState {
    pub report: Option<StandupReport>,
    pub last_run: Option<crate::standup_db::RunSummary>,
    pub hidden_keys: Vec<String>,
    /// Map of issue key -> manual_order (lower comes first). Keys missing here have no manual override.
    pub manual_orders: std::collections::HashMap<String, i64>,
    pub history: Vec<crate::standup_db::RunSummary>,
}

/// Read panel state from local cache only — does NOT hit Jira.
/// Returns the most recent successful run's report (rebuilt from snapshot),
/// plus hidden keys and recent run history.
#[tauri::command]
pub async fn get_standup_panel_state() -> Result<StandupPanelState, String> {
    let db = match crate::standup_db::StandupDb::open() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("standup_db: open failed: {e}");
            return Ok(StandupPanelState {
                report: None,
                last_run: None,
                hidden_keys: Vec::new(),
                manual_orders: std::collections::HashMap::new(),
                history: Vec::new(),
            });
        }
    };

    let last_run = db.last_successful_run()?;
    let report = match &last_run {
        Some(run) => {
            let snap = db.snapshot_for_run(run.id)?;
            Some(crate::standup_db::report_from_snapshot(&run.run_at, snap))
        }
        None => None,
    };
    let hidden_keys = db.get_hidden_keys()?;
    let history = db.list_runs(20)?;
    let manual_orders: std::collections::HashMap<String, i64> =
        db.get_manual_orders()?.into_iter().collect();

    Ok(StandupPanelState {
        report,
        last_run,
        hidden_keys,
        manual_orders,
        history,
    })
}

#[tauri::command]
pub async fn set_issue_hidden(key: String, hidden: bool) -> Result<(), String> {
    let db = crate::standup_db::StandupDb::open()?;
    db.set_hidden(&key, hidden)
}

#[tauri::command]
pub async fn clear_hidden_issues() -> Result<usize, String> {
    let db = crate::standup_db::StandupDb::open()?;
    db.clear_all_hidden()
}

/// Apply a manual ordering. `ordered_keys` is the new full order for ONE section
/// (bugs or non-bugs) — each key in the list gets a manual_order matching its
/// index in the list. Keys NOT in this list keep whatever manual_order they had.
#[tauri::command]
pub async fn set_issue_order(ordered_keys: Vec<String>) -> Result<(), String> {
    let db = crate::standup_db::StandupDb::open()?;
    for (i, key) in ordered_keys.iter().enumerate() {
        db.set_manual_order(key, Some(i as i64))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn clear_manual_order() -> Result<usize, String> {
    let db = crate::standup_db::StandupDb::open()?;
    db.clear_all_manual_order()
}

/// Read a single past run's snapshot, rebuilt as a StandupReport. Lets the panel
/// show "what was on my list last Tuesday" without re-querying Jira.
#[tauri::command]
pub async fn get_run_snapshot(run_id: i64) -> Result<Option<StandupReport>, String> {
    let db = crate::standup_db::StandupDb::open()?;
    let runs = db.list_runs(1000)?;
    let target = match runs.into_iter().find(|r| r.id == run_id) {
        Some(r) => r,
        None => return Ok(None),
    };
    let snap = db.snapshot_for_run(target.id)?;
    Ok(Some(crate::standup_db::report_from_snapshot(
        &target.run_at,
        snap,
    )))
}

// --- Issue detail (double-click target) ---

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueDetail {
    pub key: String,
    pub url: String,
    pub summary: String,
    pub status: String,
    pub status_group: StatusGroup,
    pub priority: Option<String>,
    pub due_date: Option<String>,
    pub story_points: Option<f64>,
    pub issue_type: String,
    pub is_bug: bool,
    pub assignee: Option<String>,
    pub reporter: Option<String>,
    pub labels: Vec<String>,
    pub description: String,
    pub spec: Option<String>,
    pub checklist: Vec<ChecklistItem>,
    /// Raw value of the checklist field, before parsing. Surfaced to the UI for
    /// debugging when parsing returns 0 items but Jira sent something — lets us
    /// learn the actual format without round-tripping through stderr.
    pub checklist_raw: Option<String>,
    pub created: Option<String>,
    pub updated: Option<String>,
}

/// Smart Checklist (Titanium plugin) custom field ID. JQL notation is `cf[13097]`;
/// the REST issue endpoint requires the `customfield_NNNNN` form.
const CHECKLIST_FIELD_ID: &str = "customfield_13097";

/// Extract the checklist payload string from a raw Jira field value.
/// Handles several wrapping conventions:
///   - bare string: use as-is
///   - `{ "v": "..." }` envelope (Smart Checklist): unwrap to inner string
///   - `{ "text": "..." }` (some plugins): same
///   - ADF document object: flatten to plain text
///   - anything else non-null: best-effort `to_string()` fallback
fn extract_checklist_text(raw: &Value) -> Option<String> {
    if raw.is_null() {
        return None;
    }
    if let Some(s) = raw.as_str() {
        return Some(s.to_string());
    }
    if let Some(obj) = raw.as_object() {
        if let Some(v) = obj.get("v").and_then(|v| v.as_str()) {
            return Some(v.to_string());
        }
        if let Some(t) = obj.get("text").and_then(|v| v.as_str()) {
            return Some(t.to_string());
        }
        let flat = adf_to_text(raw);
        if !flat.trim().is_empty() {
            return Some(flat);
        }
        return serde_json::to_string_pretty(raw).ok();
    }
    Some(raw.to_string())
}

/// Cache of resolved custom field IDs, keyed by display name (lowercased).
/// Jira custom field IDs are stable per workspace; safe to cache for the session.
static FIELD_ID_CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, Option<String>>>> =
    std::sync::OnceLock::new();

fn field_cache() -> &'static std::sync::Mutex<std::collections::HashMap<String, Option<String>>> {
    FIELD_ID_CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

async fn lookup_field_id(s: &crate::config::StandupConfig, name: &str) -> Option<String> {
    let key = name.to_lowercase();
    {
        let cache = field_cache().lock().ok()?;
        if let Some(cached) = cache.get(&key) {
            return cached.clone();
        }
    }

    let email = s.jira_email.as_deref().unwrap_or("");
    let token = s.jira_api_token.as_deref().unwrap_or("");
    let auth = format!("{}:{}", email, token);
    let auth_b64 = base64::engine::general_purpose::STANDARD.encode(auth.as_bytes());

    let url = format!("https://{}/rest/api/3/field", s.jira_domain);
    let client = reqwest::Client::builder().build().ok()?;
    let resp = client
        .get(&url)
        .header("Authorization", format!("Basic {}", auth_b64))
        .header("Accept", "application/json")
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let fields: Vec<Value> = resp.json().await.ok()?;
    let id = fields.iter().find_map(|f| {
        let fname = f.get("name")?.as_str()?;
        if fname.eq_ignore_ascii_case(name) {
            f.get("id").and_then(|v| v.as_str()).map(|s| s.to_string())
        } else {
            None
        }
    });

    if let Ok(mut cache) = field_cache().lock() {
        cache.insert(key, id.clone());
    }
    id
}

/// Recursively flatten an Atlassian Document Format node tree to plain text.
/// Block nodes (paragraph, heading, list items) get newline separators.
fn adf_to_text(node: &Value) -> String {
    let mut out = String::new();
    flatten_adf(node, &mut out, 0);
    // Collapse 3+ consecutive newlines to 2; trim trailing whitespace.
    let trimmed = out.trim_end().to_string();
    let mut collapsed = String::with_capacity(trimmed.len());
    let mut nl_count = 0;
    for ch in trimmed.chars() {
        if ch == '\n' {
            nl_count += 1;
            if nl_count <= 2 {
                collapsed.push(ch);
            }
        } else {
            nl_count = 0;
            collapsed.push(ch);
        }
    }
    collapsed
}

fn flatten_adf(node: &Value, out: &mut String, list_depth: usize) {
    if let Some(text) = node.get("text").and_then(|v| v.as_str()) {
        out.push_str(text);
        return;
    }
    let typ = node.get("type").and_then(|v| v.as_str()).unwrap_or("");

    let is_list_item = typ == "listItem";
    if is_list_item {
        for _ in 0..list_depth {
            out.push_str("  ");
        }
        out.push_str("- ");
    }

    let child_depth = if typ == "bulletList" || typ == "orderedList" {
        list_depth + 1
    } else {
        list_depth
    };

    if let Some(content) = node.get("content").and_then(|v| v.as_array()) {
        for child in content {
            flatten_adf(child, out, child_depth);
        }
    }

    match typ {
        "paragraph" | "heading" | "listItem" | "codeBlock" | "blockquote" => out.push('\n'),
        "hardBreak" => out.push('\n'),
        _ => {}
    }
}

#[tauri::command]
pub async fn get_issue_detail(
    cfg: tauri::State<'_, AppConfig>,
    key: String,
) -> Result<IssueDetail, String> {
    let s = standup_config(&cfg)?;
    let email = s.jira_email.as_deref().unwrap_or("");
    let token = s.jira_api_token.as_deref().unwrap_or("");
    let auth = format!("{}:{}", email, token);
    let auth_b64 = base64::engine::general_purpose::STANDARD.encode(auth.as_bytes());

    // Resolve the spec field name → field ID once and cache. The checklist field
    // ID is hard-coded above.
    let spec_field_id = match s.spec_field_name.as_deref() {
        Some(name) if !name.is_empty() => lookup_field_id(s, name).await,
        _ => None,
    };

    let mut fields_list = String::from(
        "summary,status,priority,duedate,issuetype,customfield_10028,description,assignee,reporter,labels,created,updated",
    );
    fields_list.push(',');
    fields_list.push_str(CHECKLIST_FIELD_ID);
    if let Some(ref fid) = spec_field_id {
        fields_list.push(',');
        fields_list.push_str(fid);
    }

    let url = format!("https://{}/rest/api/3/issue/{}", s.jira_domain, key);

    let client = reqwest::Client::builder()
        .build()
        .map_err(|e| format!("reqwest build failed: {e}"))?;

    let resp = client
        .get(&url)
        .header("Authorization", format!("Basic {}", auth_b64))
        .header("Accept", "application/json")
        .query(&[("fields", fields_list.as_str())])
        .send()
        .await
        .map_err(|e| format!("Jira request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Jira API error: {} - {}", status, body));
    }

    let payload: Value = resp
        .json()
        .await
        .map_err(|e| format!("Jira response parse failed: {e}"))?;

    let fields = payload.get("fields").cloned().unwrap_or(Value::Null);
    let status_obj = fields.get("status").cloned().unwrap_or(Value::Null);
    let status_name = status_obj.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let status_group = status_to_group(status_name);

    let issue_type = fields
        .get("issuetype")
        .and_then(|v| v.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("Task")
        .to_string();
    let is_bug = issue_type.eq_ignore_ascii_case("bug");

    let priority = fields
        .get("priority")
        .and_then(|v| v.get("name"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let due_date = fields
        .get("duedate")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let story_points = fields
        .get("customfield_10028")
        .and_then(|v| v.as_f64());

    let assignee = fields
        .get("assignee")
        .and_then(|v| v.get("displayName"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let reporter = fields
        .get("reporter")
        .and_then(|v| v.get("displayName"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let labels: Vec<String> = fields
        .get("labels")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let description = fields
        .get("description")
        .map(adf_to_text)
        .unwrap_or_default();

    let spec = spec_field_id.as_ref().and_then(|fid| {
        let raw = fields.get(fid)?;
        let text = if raw.is_string() {
            raw.as_str().map(|s| s.to_string())
        } else if raw.is_object() {
            // Likely an ADF document.
            Some(adf_to_text(raw))
        } else if raw.is_null() {
            None
        } else {
            Some(raw.to_string())
        }?;
        let trimmed = text.trim().to_string();
        if trimmed.is_empty() { None } else { Some(trimmed) }
    });

    let checklist_raw: Option<String> = fields
        .get(CHECKLIST_FIELD_ID)
        .and_then(extract_checklist_text)
        .filter(|s| !s.trim().is_empty());
    let checklist = checklist_raw
        .as_deref()
        .map(parse_checklist)
        .unwrap_or_default();

    let created = fields
        .get("created")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let updated = fields
        .get("updated")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Ok(IssueDetail {
        key: key.clone(),
        url: format!("https://{}/browse/{}", s.jira_domain, key),
        summary: fields
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        status: status_name.to_string(),
        status_group,
        priority,
        due_date,
        story_points,
        issue_type,
        is_bug,
        assignee,
        reporter,
        labels,
        description,
        spec,
        checklist,
        checklist_raw,
        created,
        updated,
    })
}

