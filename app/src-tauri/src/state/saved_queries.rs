//! SQLite-backed registry of saved SQL queries and the groups that organize
//! them. Replaces the per-window localStorage flat list used by older builds.
//!
//! Persistence: `%LOCALAPPDATA%\fnba-utils\saved-queries.db`. Kept in its own
//! DB (separate from `clipboard.db`) so clipboard PII never shares a file with
//! query strings, and so the `fnba-clipd` daemon's writes can't contend with
//! query saves.
//!
//! Schema is migrated idempotently on open via `CREATE TABLE IF NOT EXISTS`.
//! Future column additions should follow the `clipboard_history::migrate`
//! pattern (`PRAGMA table_info` + conditional `ALTER TABLE`).

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SqlGroup {
    pub id: String,
    pub name: String,
    pub order_idx: u32,
    pub color: Option<String>,
    pub pinned: bool,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SavedSqlQuery {
    pub id: String,
    pub name: String,
    pub sql: String,
    pub database: String,
    pub group_id: Option<String>,
    pub last_used_at: i64,
    pub created_at: i64,
}

/// Legacy entries handed over by the frontend during one-time migration.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LegacySavedQuery {
    pub name: String,
    pub sql: String,
    #[serde(default)]
    pub database: String,
}

pub struct SavedQueriesState {
    conn: Mutex<Connection>,
    #[allow(dead_code)]
    db_path: PathBuf,
}

fn epoch_ms_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn new_uuid() -> String {
    Uuid::new_v4().to_string()
}

fn map_db(e: rusqlite::Error) -> String {
    format!("saved_queries DB error: {e}")
}

fn resolve_db_path() -> PathBuf {
    crate::state::paths::data_file("saved-queries.db")
}

impl SavedQueriesState {
    pub fn load() -> Self {
        let db_path = resolve_db_path();
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(&db_path).unwrap_or_else(|e| {
            panic!(
                "Failed to open saved-queries DB at {}: {e}",
                db_path.display()
            )
        });
        let _ = conn.busy_timeout(Duration::from_secs(5));
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        let _ = conn.pragma_update(None, "synchronous", "NORMAL");
        // FK ON DELETE SET NULL on saved_sql_queries.group_id only fires when
        // foreign_keys is on; SQLite defaults it off per-connection.
        let _ = conn.pragma_update(None, "foreign_keys", "ON");
        Self::migrate(&conn).expect("saved-queries DB migration failed");
        Self {
            conn: Mutex::new(conn),
            db_path,
        }
    }

    fn migrate(conn: &Connection) -> rusqlite::Result<()> {
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS sql_groups (
                id        TEXT PRIMARY KEY,
                name      TEXT NOT NULL,
                order_idx INTEGER NOT NULL DEFAULT 0,
                color     TEXT,
                pinned    INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS saved_sql_queries (
                id           TEXT PRIMARY KEY,
                name         TEXT NOT NULL,
                sql_text     TEXT NOT NULL,
                database     TEXT NOT NULL DEFAULT '',
                group_id     TEXT REFERENCES sql_groups(id) ON DELETE SET NULL,
                last_used_at INTEGER NOT NULL DEFAULT 0,
                created_at   INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_sql_queries_group     ON saved_sql_queries(group_id);
            CREATE INDEX IF NOT EXISTS idx_sql_queries_last_used ON saved_sql_queries(last_used_at DESC);
            ",
        )
    }

    // ---------- Groups ----------

    pub fn list_groups(&self) -> Result<Vec<SqlGroup>, String> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| format!("saved_queries lock poisoned: {e}"))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, name, order_idx, color, pinned
                   FROM sql_groups
                  ORDER BY pinned DESC, order_idx ASC, name ASC",
            )
            .map_err(map_db)?;
        let rows = stmt
            .query_map([], row_to_group)
            .map_err(map_db)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db)?;
        Ok(rows)
    }

    pub fn add_group(&self, name: String) -> Result<SqlGroup, String> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| format!("saved_queries lock poisoned: {e}"))?;
        let id = new_uuid();
        let next_order: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(order_idx) + 1, 0) FROM sql_groups",
                [],
                |r| r.get(0),
            )
            .map_err(map_db)?;
        conn.execute(
            "INSERT INTO sql_groups (id, name, order_idx, color, pinned)
             VALUES (?1, ?2, ?3, NULL, 0)",
            params![id, name, next_order],
        )
        .map_err(map_db)?;
        Ok(SqlGroup {
            id,
            name,
            order_idx: next_order as u32,
            color: None,
            pinned: false,
        })
    }

    pub fn rename_group(&self, id: &str, name: &str) -> Result<(), String> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| format!("saved_queries lock poisoned: {e}"))?;
        conn.execute(
            "UPDATE sql_groups SET name = ?1 WHERE id = ?2",
            params![name, id],
        )
        .map_err(map_db)?;
        Ok(())
    }

    pub fn set_group_color(&self, id: &str, color: Option<&str>) -> Result<(), String> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| format!("saved_queries lock poisoned: {e}"))?;
        conn.execute(
            "UPDATE sql_groups SET color = ?1 WHERE id = ?2",
            params![color, id],
        )
        .map_err(map_db)?;
        Ok(())
    }

    pub fn set_group_pinned(&self, id: &str, pinned: bool) -> Result<(), String> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| format!("saved_queries lock poisoned: {e}"))?;
        conn.execute(
            "UPDATE sql_groups SET pinned = ?1 WHERE id = ?2",
            params![pinned as i64, id],
        )
        .map_err(map_db)?;
        Ok(())
    }

    /// Reassign order_idx to each id in `ids` by its position in the slice.
    /// Groups not present are left untouched.
    pub fn reorder_groups(&self, ids: &[String]) -> Result<(), String> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| format!("saved_queries lock poisoned: {e}"))?;
        let tx = conn.transaction().map_err(map_db)?;
        for (i, id) in ids.iter().enumerate() {
            tx.execute(
                "UPDATE sql_groups SET order_idx = ?1 WHERE id = ?2",
                params![i as i64, id],
            )
            .map_err(map_db)?;
        }
        tx.commit().map_err(map_db)
    }

    pub fn remove_group(&self, id: &str) -> Result<bool, String> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| format!("saved_queries lock poisoned: {e}"))?;
        let n = conn
            .execute("DELETE FROM sql_groups WHERE id = ?1", params![id])
            .map_err(map_db)?;
        Ok(n > 0)
    }

    // ---------- Queries ----------

    pub fn list_queries(&self) -> Result<Vec<SavedSqlQuery>, String> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| format!("saved_queries lock poisoned: {e}"))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, name, sql_text, database, group_id, last_used_at, created_at
                   FROM saved_sql_queries
                  ORDER BY last_used_at DESC, name ASC",
            )
            .map_err(map_db)?;
        let rows = stmt
            .query_map([], row_to_query)
            .map_err(map_db)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db)?;
        Ok(rows)
    }

    pub fn add_query(
        &self,
        name: String,
        sql: String,
        database: String,
        group_id: Option<String>,
    ) -> Result<SavedSqlQuery, String> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| format!("saved_queries lock poisoned: {e}"))?;
        let id = new_uuid();
        let now = epoch_ms_now();
        conn.execute(
            "INSERT INTO saved_sql_queries
                (id, name, sql_text, database, group_id, last_used_at, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6)",
            params![id, name, sql, database, group_id, now],
        )
        .map_err(map_db)?;
        Ok(SavedSqlQuery {
            id,
            name,
            sql,
            database,
            group_id,
            last_used_at: 0,
            created_at: now,
        })
    }

    pub fn update_query(
        &self,
        id: &str,
        name: &str,
        sql: &str,
        database: &str,
    ) -> Result<(), String> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| format!("saved_queries lock poisoned: {e}"))?;
        conn.execute(
            "UPDATE saved_sql_queries
                SET name = ?1, sql_text = ?2, database = ?3
              WHERE id = ?4",
            params![name, sql, database, id],
        )
        .map_err(map_db)?;
        Ok(())
    }

    pub fn move_query_to_group(
        &self,
        id: &str,
        group_id: Option<&str>,
    ) -> Result<(), String> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| format!("saved_queries lock poisoned: {e}"))?;
        conn.execute(
            "UPDATE saved_sql_queries SET group_id = ?1 WHERE id = ?2",
            params![group_id, id],
        )
        .map_err(map_db)?;
        Ok(())
    }

    pub fn remove_query(&self, id: &str) -> Result<bool, String> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| format!("saved_queries lock poisoned: {e}"))?;
        let n = conn
            .execute("DELETE FROM saved_sql_queries WHERE id = ?1", params![id])
            .map_err(map_db)?;
        Ok(n > 0)
    }

    pub fn record_used(&self, id: &str) -> Result<(), String> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| format!("saved_queries lock poisoned: {e}"))?;
        let now = epoch_ms_now();
        conn.execute(
            "UPDATE saved_sql_queries SET last_used_at = ?1 WHERE id = ?2",
            params![now, id],
        )
        .map_err(map_db)?;
        Ok(())
    }

    /// Idempotent: drops legacy entries whose (name, sql_text) already exist.
    /// Imported entries land in the "Ungrouped" bucket (group_id = NULL).
    pub fn migrate_legacy(
        &self,
        entries: Vec<LegacySavedQuery>,
    ) -> Result<u32, String> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| format!("saved_queries lock poisoned: {e}"))?;
        let tx = conn.transaction().map_err(map_db)?;
        let mut migrated: u32 = 0;
        for entry in entries {
            if entry.name.trim().is_empty() || entry.sql.trim().is_empty() {
                continue;
            }
            let exists: i64 = tx
                .query_row(
                    "SELECT COUNT(1) FROM saved_sql_queries
                      WHERE name = ?1 AND sql_text = ?2",
                    params![entry.name, entry.sql],
                    |r| r.get(0),
                )
                .map_err(map_db)?;
            if exists > 0 {
                continue;
            }
            let id = new_uuid();
            let now = epoch_ms_now();
            tx.execute(
                "INSERT INTO saved_sql_queries
                    (id, name, sql_text, database, group_id, last_used_at, created_at)
                 VALUES (?1, ?2, ?3, ?4, NULL, 0, ?5)",
                params![id, entry.name, entry.sql, entry.database, now],
            )
            .map_err(map_db)?;
            migrated += 1;
        }
        tx.commit().map_err(map_db)?;
        Ok(migrated)
    }
}

fn row_to_group(row: &rusqlite::Row<'_>) -> rusqlite::Result<SqlGroup> {
    let pinned: i64 = row.get(4)?;
    Ok(SqlGroup {
        id: row.get(0)?,
        name: row.get(1)?,
        order_idx: row.get::<_, i64>(2)?.max(0) as u32,
        color: row.get(3)?,
        pinned: pinned != 0,
    })
}

fn row_to_query(row: &rusqlite::Row<'_>) -> rusqlite::Result<SavedSqlQuery> {
    Ok(SavedSqlQuery {
        id: row.get(0)?,
        name: row.get(1)?,
        sql: row.get(2)?,
        database: row.get(3)?,
        group_id: row.get(4)?,
        last_used_at: row.get(5)?,
        created_at: row.get(6)?,
    })
}
