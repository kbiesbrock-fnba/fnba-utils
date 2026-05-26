use crate::models::standup::{parse_checklist, JiraIssue, StandupGroup, StandupReport, StatusGroup};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::path::PathBuf;

/// SQLite database for standup state.
///
/// `%LOCALAPPDATA%\fnba-utils\standup.db` holds:
///   - `issue_state` — per-key hide flag + first/last-seen timestamps
///   - `run_history` — one row per run_standup() invocation
///   - `run_snapshot` — denormalized issue list for each run (lets us answer
///     "what was on my list last Tuesday?")
pub struct StandupDb {
    conn: Connection,
}

fn db_path() -> Option<PathBuf> {
    Some(crate::state::paths::data_file("standup.db"))
}

impl StandupDb {
    pub fn open() -> Result<Self, String> {
        let path = db_path().ok_or_else(|| "Could not determine home dir".to_string())?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let conn = Connection::open(&path).map_err(|e| e.to_string())?;
        let db = Self { conn };
        db.bootstrap()?;
        Ok(db)
    }

    fn bootstrap(&self) -> Result<(), String> {
        self.conn
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS issue_state (
                    key TEXT PRIMARY KEY,
                    hidden INTEGER NOT NULL DEFAULT 0,
                    first_seen_at TEXT NOT NULL,
                    last_seen_at TEXT NOT NULL,
                    manual_order INTEGER
                );
                CREATE TABLE IF NOT EXISTS run_history (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    run_at TEXT NOT NULL,
                    issue_count INTEGER NOT NULL,
                    posted_to_teams INTEGER NOT NULL,
                    error TEXT
                );
                CREATE TABLE IF NOT EXISTS run_snapshot (
                    run_id INTEGER NOT NULL REFERENCES run_history(id) ON DELETE CASCADE,
                    issue_key TEXT NOT NULL,
                    summary TEXT NOT NULL,
                    status TEXT NOT NULL,
                    status_group TEXT NOT NULL,
                    story_points REAL,
                    url TEXT NOT NULL,
                    priority TEXT,
                    priority_rank INTEGER NOT NULL DEFAULT 10,
                    due_date TEXT,
                    issue_type TEXT NOT NULL DEFAULT 'Task',
                    is_bug INTEGER NOT NULL DEFAULT 0,
                    has_checklist INTEGER NOT NULL DEFAULT 0,
                    checklist_text TEXT,
                    PRIMARY KEY (run_id, issue_key)
                );
                CREATE INDEX IF NOT EXISTS idx_run_history_run_at ON run_history(run_at DESC);
                CREATE INDEX IF NOT EXISTS idx_run_snapshot_run_id ON run_snapshot(run_id);
                ",
            )
            .map_err(|e| e.to_string())?;

        // Best-effort migrations for users who already created the DB at v1.2.
        // SQLite has no ADD COLUMN IF NOT EXISTS — these will error if the column
        // already exists; we swallow that and continue.
        for stmt in [
            "ALTER TABLE issue_state ADD COLUMN manual_order INTEGER",
            "ALTER TABLE run_snapshot ADD COLUMN priority TEXT",
            "ALTER TABLE run_snapshot ADD COLUMN priority_rank INTEGER NOT NULL DEFAULT 10",
            "ALTER TABLE run_snapshot ADD COLUMN due_date TEXT",
            "ALTER TABLE run_snapshot ADD COLUMN issue_type TEXT NOT NULL DEFAULT 'Task'",
            "ALTER TABLE run_snapshot ADD COLUMN is_bug INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE run_snapshot ADD COLUMN has_checklist INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE run_snapshot ADD COLUMN checklist_text TEXT",
        ] {
            let _ = self.conn.execute(stmt, []);
        }

        Ok(())
    }

    /// Insert a run + its snapshot rows. Returns the new run_id.
    pub fn record_run(
        &mut self,
        report: &StandupReport,
        posted_to_teams: bool,
        error: Option<&str>,
    ) -> Result<i64, String> {
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;
        tx.execute(
            "INSERT INTO run_history (run_at, issue_count, posted_to_teams, error)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                report.generated_at,
                report.issue_count as i64,
                if posted_to_teams { 1 } else { 0 },
                error,
            ],
        )
        .map_err(|e| e.to_string())?;
        let run_id = tx.last_insert_rowid();

        for group in &report.groups {
            for issue in &group.issues {
                tx.execute(
                    "INSERT INTO run_snapshot
                       (run_id, issue_key, summary, status, status_group, story_points,
                        url, priority, priority_rank, due_date, issue_type, is_bug,
                        has_checklist, checklist_text)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                    params![
                        run_id,
                        issue.key,
                        issue.summary,
                        issue.status,
                        status_group_to_db(issue.status_group),
                        issue.story_points,
                        issue.url,
                        issue.priority,
                        issue.priority_rank,
                        issue.due_date,
                        issue.issue_type,
                        if issue.is_bug { 1i64 } else { 0i64 },
                        if issue.has_checklist { 1i64 } else { 0i64 },
                        issue.checklist_text,
                    ],
                )
                .map_err(|e| e.to_string())?;
            }
        }

        tx.commit().map_err(|e| e.to_string())?;
        Ok(run_id)
    }

    /// Upsert seen timestamps for the given issue keys.
    pub fn mark_seen(&mut self, keys: &[String]) -> Result<(), String> {
        let now = Utc::now().to_rfc3339();
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;
        for key in keys {
            tx.execute(
                "INSERT INTO issue_state (key, hidden, first_seen_at, last_seen_at)
                 VALUES (?1, 0, ?2, ?2)
                 ON CONFLICT(key) DO UPDATE SET last_seen_at = excluded.last_seen_at",
                params![key, now],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Flip an existing run's posted_to_teams flag to true. Keyed by run_at
    /// (the RFC3339 generated_at the frontend echoes back from the preview).
    /// Returns the number of rows updated — 0 means we couldn't find the preview row
    /// (caller should fall back to recording a fresh row).
    pub fn mark_run_posted(&self, run_at: &str) -> Result<usize, String> {
        self.conn
            .execute(
                "UPDATE run_history SET posted_to_teams = 1, error = NULL WHERE run_at = ?1",
                params![run_at],
            )
            .map_err(|e| e.to_string())
    }

    pub fn set_hidden(&self, key: &str, hidden: bool) -> Result<(), String> {
        let now = Utc::now().to_rfc3339();
        self.conn
            .execute(
                "INSERT INTO issue_state (key, hidden, first_seen_at, last_seen_at)
                 VALUES (?1, ?2, ?3, ?3)
                 ON CONFLICT(key) DO UPDATE SET hidden = excluded.hidden",
                params![key, if hidden { 1 } else { 0 }, now],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn clear_all_hidden(&self) -> Result<usize, String> {
        let n = self
            .conn
            .execute("UPDATE issue_state SET hidden = 0 WHERE hidden = 1", [])
            .map_err(|e| e.to_string())?;
        Ok(n)
    }

    pub fn get_hidden_keys(&self) -> Result<Vec<String>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT key FROM issue_state WHERE hidden = 1")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| e.to_string())?);
        }
        Ok(out)
    }

    pub fn list_runs(&self, limit: usize) -> Result<Vec<RunSummary>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, run_at, issue_count, posted_to_teams, error
                 FROM run_history
                 ORDER BY run_at DESC
                 LIMIT ?1",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![limit as i64], |row| {
                Ok(RunSummary {
                    id: row.get(0)?,
                    run_at: row.get(1)?,
                    issue_count: row.get::<_, i64>(2)? as usize,
                    posted_to_teams: row.get::<_, i64>(3)? != 0,
                    error: row.get::<_, Option<String>>(4)?,
                })
            })
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| e.to_string())?);
        }
        Ok(out)
    }

    pub fn last_successful_run(&self) -> Result<Option<RunSummary>, String> {
        self.conn
            .query_row(
                "SELECT id, run_at, issue_count, posted_to_teams, error
                 FROM run_history
                 WHERE error IS NULL
                 ORDER BY run_at DESC
                 LIMIT 1",
                [],
                |row| {
                    Ok(RunSummary {
                        id: row.get(0)?,
                        run_at: row.get(1)?,
                        issue_count: row.get::<_, i64>(2)? as usize,
                        posted_to_teams: row.get::<_, i64>(3)? != 0,
                        error: row.get::<_, Option<String>>(4)?,
                    })
                },
            )
            .optional()
            .map_err(|e| e.to_string())
    }

    pub fn snapshot_for_run(&self, run_id: i64) -> Result<Vec<SnapshotIssue>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT issue_key, summary, status, status_group, story_points, url,
                        priority, priority_rank, due_date, issue_type, is_bug, has_checklist,
                        checklist_text
                 FROM run_snapshot
                 WHERE run_id = ?1
                 ORDER BY status_group, issue_key",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![run_id], |row| {
                Ok(SnapshotIssue {
                    key: row.get(0)?,
                    summary: row.get(1)?,
                    status: row.get(2)?,
                    status_group: row.get(3)?,
                    story_points: row.get::<_, Option<f64>>(4)?,
                    url: row.get(5)?,
                    priority: row.get::<_, Option<String>>(6)?,
                    priority_rank: row.get::<_, i64>(7)? as i32,
                    due_date: row.get::<_, Option<String>>(8)?,
                    issue_type: row.get::<_, String>(9)?,
                    is_bug: row.get::<_, i64>(10)? != 0,
                    has_checklist: row.get::<_, i64>(11)? != 0,
                    checklist_text: row.get::<_, Option<String>>(12)?,
                })
            })
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| e.to_string())?);
        }
        Ok(out)
    }

    pub fn set_manual_order(&self, key: &str, order: Option<i64>) -> Result<(), String> {
        let now = Utc::now().to_rfc3339();
        self.conn
            .execute(
                "INSERT INTO issue_state (key, hidden, first_seen_at, last_seen_at, manual_order)
                 VALUES (?1, 0, ?2, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE SET manual_order = excluded.manual_order",
                params![key, now, order],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn clear_all_manual_order(&self) -> Result<usize, String> {
        let n = self
            .conn
            .execute(
                "UPDATE issue_state SET manual_order = NULL WHERE manual_order IS NOT NULL",
                [],
            )
            .map_err(|e| e.to_string())?;
        Ok(n)
    }

    /// Return (key, manual_order) for every issue with a non-null manual_order.
    pub fn get_manual_orders(&self) -> Result<Vec<(String, i64)>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT key, manual_order FROM issue_state WHERE manual_order IS NOT NULL")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| e.to_string())?);
        }
        Ok(out)
    }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RunSummary {
    pub id: i64,
    pub run_at: String,
    pub issue_count: usize,
    pub posted_to_teams: bool,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotIssue {
    pub key: String,
    pub summary: String,
    pub status: String,
    pub status_group: String,
    pub story_points: Option<f64>,
    pub url: String,
    pub priority: Option<String>,
    pub priority_rank: i32,
    pub due_date: Option<String>,
    pub issue_type: String,
    pub is_bug: bool,
    pub has_checklist: bool,
    pub checklist_text: Option<String>,
}

pub fn status_group_to_db(g: StatusGroup) -> &'static str {
    match g {
        StatusGroup::InProgress => "in_progress",
        StatusGroup::Review => "review",
        StatusGroup::Todo => "todo",
        StatusGroup::Attention => "attention",
        StatusGroup::Done => "done",
    }
}

pub fn status_group_from_db(s: &str) -> StatusGroup {
    match s {
        "in_progress" => StatusGroup::InProgress,
        "review" => StatusGroup::Review,
        "attention" => StatusGroup::Attention,
        "done" => StatusGroup::Done,
        _ => StatusGroup::Todo,
    }
}

/// Rebuild a StandupReport from a stored snapshot. Used by the panel to render
/// past runs without re-querying Jira.
pub fn report_from_snapshot(
    run_at: &str,
    snapshot: Vec<SnapshotIssue>,
) -> StandupReport {
    let mut by_group: std::collections::HashMap<StatusGroup, Vec<JiraIssue>> =
        std::collections::HashMap::new();
    for issue in snapshot.into_iter() {
        let g = status_group_from_db(&issue.status_group);
        let checklist = issue
            .checklist_text
            .as_deref()
            .map(parse_checklist)
            .unwrap_or_default();
        by_group.entry(g).or_default().push(JiraIssue {
            key: issue.key,
            summary: issue.summary,
            status: issue.status,
            status_category: String::new(),
            status_group: g,
            story_points: issue.story_points,
            url: issue.url,
            priority: issue.priority,
            priority_rank: issue.priority_rank,
            due_date: issue.due_date,
            issue_type: issue.issue_type,
            is_bug: issue.is_bug,
            has_checklist: issue.has_checklist,
            checklist_text: issue.checklist_text,
            checklist,
        });
    }

    let mut groups = Vec::new();
    let mut total = 0usize;
    for g in StatusGroup::ordered() {
        let bucket = by_group.remove(g).unwrap_or_default();
        if bucket.is_empty() {
            continue;
        }
        let total_points: f64 = bucket.iter().filter_map(|i| i.story_points).sum();
        total += bucket.len();
        groups.push(StandupGroup {
            group: *g,
            label: g.label().to_string(),
            emoji: g.emoji().to_string(),
            issues: bucket,
            total_points,
        });
    }

    StandupReport {
        generated_at: run_at.to_string(),
        issue_count: total,
        groups,
    }
}
