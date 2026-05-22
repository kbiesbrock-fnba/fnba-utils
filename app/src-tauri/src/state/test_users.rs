//! Test User pool used by the PII substitutor.
//!
//! When the clipboard listener detects PII in a new entry, it picks a random
//! enabled Test User and substitutes the detected fields with that user's
//! matching values. The pool lives next to the clipboard history in the same
//! SQLite database (separate connection — WAL handles concurrent access).
//!
//! First-run seeds 10 fake identities so the substitutor is useful out of the
//! box. Users edit / add / disable / delete via the Test Users panel inside
//! the clipboard manager window. The daemon reads this pool read-only.

use crate::clipboard::pii::PiiSubject;
use rand::seq::SliceRandom;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct TestCard {
    pub number: String,
    pub expiry: String,
    pub cvv: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TestUser {
    #[serde(default)]
    pub id: Option<i64>,
    pub label: String,
    #[serde(default)]
    pub first_name: Option<String>,
    #[serde(default)]
    pub last_name: Option<String>,
    #[serde(default)]
    pub ssn: Option<String>,
    #[serde(default)]
    pub dob: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub account_num: Option<String>,
    #[serde(default)]
    pub routing_num: Option<String>,
    #[serde(default)]
    pub cards: Vec<TestCard>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

fn nonempty(s: &Option<String>) -> Option<&str> {
    s.as_deref().filter(|v| !v.is_empty())
}

impl PiiSubject for TestUser {
    fn ssn(&self) -> Option<&str> {
        nonempty(&self.ssn)
    }
    fn dob(&self) -> Option<&str> {
        nonempty(&self.dob)
    }
    fn email(&self) -> Option<&str> {
        nonempty(&self.email)
    }
    fn phone(&self) -> Option<&str> {
        nonempty(&self.phone)
    }
    fn account(&self) -> Option<&str> {
        nonempty(&self.account_num)
    }
    fn routing(&self) -> Option<&str> {
        nonempty(&self.routing_num)
    }
    fn card(&self, nth: usize) -> Option<&str> {
        if self.cards.is_empty() {
            return None;
        }
        let card = &self.cards[nth % self.cards.len()];
        if card.number.is_empty() {
            None
        } else {
            Some(&card.number)
        }
    }
}

pub struct TestUsersState {
    conn: Mutex<Connection>,
    #[allow(dead_code)]
    db_path: PathBuf,
}

impl TestUsersState {
    pub fn load() -> Self {
        let db_path = resolve_db_path();
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(&db_path).unwrap_or_else(|e| {
            panic!(
                "Failed to open test-users DB at {}: {e}",
                db_path.display()
            )
        });
        let _ = conn.busy_timeout(Duration::from_secs(5));
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        let _ = conn.pragma_update(None, "synchronous", "NORMAL");
        Self::migrate(&conn).expect("test_users DB migration failed");
        let state = Self {
            conn: Mutex::new(conn),
            db_path,
        };
        // Seed only if empty — never overwrite user edits.
        if let Ok(0) = state.count_all() {
            for user in seed_users() {
                let _ = state.upsert(&user);
            }
        }
        state
    }

    fn migrate(conn: &Connection) -> rusqlite::Result<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS test_users (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                label         TEXT NOT NULL,
                first_name    TEXT,
                last_name     TEXT,
                ssn           TEXT,
                dob           TEXT,
                email         TEXT,
                phone         TEXT,
                address       TEXT,
                account_num   TEXT,
                routing_num   TEXT,
                cards_json    TEXT,
                enabled       INTEGER NOT NULL DEFAULT 1,
                created_at    INTEGER NOT NULL
            );",
        )
    }

    pub fn count_all(&self) -> Result<i64, String> {
        let conn = self.conn.lock().map_err(|e| format!("test_users lock poisoned: {e}"))?;
        conn.query_row("SELECT COUNT(*) FROM test_users", [], |r| r.get(0))
            .map_err(map_db)
    }

    pub fn list_all(&self) -> Result<Vec<TestUser>, String> {
        let conn = self.conn.lock().map_err(|e| format!("test_users lock poisoned: {e}"))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, label, first_name, last_name, ssn, dob, email, phone, address,
                        account_num, routing_num, cards_json, enabled
                   FROM test_users
                  ORDER BY id ASC",
            )
            .map_err(map_db)?;
        let rows = stmt
            .query_map([], row_to_user)
            .map_err(map_db)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db)?;
        Ok(rows)
    }

    pub fn list_enabled(&self) -> Result<Vec<TestUser>, String> {
        let conn = self.conn.lock().map_err(|e| format!("test_users lock poisoned: {e}"))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, label, first_name, last_name, ssn, dob, email, phone, address,
                        account_num, routing_num, cards_json, enabled
                   FROM test_users
                  WHERE enabled = 1
                  ORDER BY id ASC",
            )
            .map_err(map_db)?;
        let rows = stmt
            .query_map([], row_to_user)
            .map_err(map_db)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db)?;
        Ok(rows)
    }

    pub fn pick_random_enabled(&self) -> Result<Option<TestUser>, String> {
        let users = self.list_enabled()?;
        if users.is_empty() {
            return Ok(None);
        }
        let mut rng = rand::thread_rng();
        Ok(users.choose(&mut rng).cloned())
    }

    pub fn upsert(&self, user: &TestUser) -> Result<i64, String> {
        let conn = self.conn.lock().map_err(|e| format!("test_users lock poisoned: {e}"))?;
        let cards_json = serde_json::to_string(&user.cards)
            .map_err(|e| format!("test_users cards encode: {e}"))?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        match user.id {
            Some(id) => {
                conn.execute(
                    "UPDATE test_users
                        SET label = ?1, first_name = ?2, last_name = ?3, ssn = ?4, dob = ?5,
                            email = ?6, phone = ?7, address = ?8, account_num = ?9,
                            routing_num = ?10, cards_json = ?11, enabled = ?12
                      WHERE id = ?13",
                    params![
                        user.label,
                        user.first_name,
                        user.last_name,
                        user.ssn,
                        user.dob,
                        user.email,
                        user.phone,
                        user.address,
                        user.account_num,
                        user.routing_num,
                        cards_json,
                        user.enabled as i64,
                        id,
                    ],
                )
                .map_err(map_db)?;
                Ok(id)
            }
            None => {
                conn.execute(
                    "INSERT INTO test_users
                        (label, first_name, last_name, ssn, dob, email, phone, address,
                         account_num, routing_num, cards_json, enabled, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                    params![
                        user.label,
                        user.first_name,
                        user.last_name,
                        user.ssn,
                        user.dob,
                        user.email,
                        user.phone,
                        user.address,
                        user.account_num,
                        user.routing_num,
                        cards_json,
                        user.enabled as i64,
                        now,
                    ],
                )
                .map_err(map_db)?;
                Ok(conn.last_insert_rowid())
            }
        }
    }

    pub fn delete(&self, id: i64) -> Result<bool, String> {
        let conn = self.conn.lock().map_err(|e| format!("test_users lock poisoned: {e}"))?;
        let n = conn
            .execute("DELETE FROM test_users WHERE id = ?1", params![id])
            .map_err(map_db)?;
        Ok(n > 0)
    }

    pub fn set_enabled(&self, id: i64, enabled: bool) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("test_users lock poisoned: {e}"))?;
        conn.execute(
            "UPDATE test_users SET enabled = ?1 WHERE id = ?2",
            params![enabled as i64, id],
        )
        .map_err(map_db)?;
        Ok(())
    }
}

fn row_to_user(row: &rusqlite::Row<'_>) -> rusqlite::Result<TestUser> {
    let cards_json: Option<String> = row.get(11)?;
    let cards: Vec<TestCard> = cards_json
        .as_deref()
        .and_then(|s| serde_json::from_str::<Vec<TestCard>>(s).ok())
        .unwrap_or_default();
    let enabled: i64 = row.get(12)?;
    Ok(TestUser {
        id: row.get::<_, Option<i64>>(0)?,
        label: row.get(1)?,
        first_name: row.get(2)?,
        last_name: row.get(3)?,
        ssn: row.get(4)?,
        dob: row.get(5)?,
        email: row.get(6)?,
        phone: row.get(7)?,
        address: row.get(8)?,
        account_num: row.get(9)?,
        routing_num: row.get(10)?,
        cards,
        enabled: enabled != 0,
    })
}

fn map_db(e: rusqlite::Error) -> String {
    format!("test_users DB error: {e}")
}

fn resolve_db_path() -> PathBuf {
    if let Some(data) = dirs::data_local_dir() {
        return data.join("fnba-utils").join("clipboard.db");
    }
    if let Some(data) = dirs::data_dir() {
        return data.join("fnba-utils").join("clipboard.db");
    }
    PathBuf::from("clipboard.db")
}

// --- Seed data ---
//
// 10 fake-but-plausible identities. SSNs use the 900-block which the SSA
// does not issue (reserved for IRS ITIN-like numbers, never assigned as real
// SSNs). Cards are the standard issuer-published test PANs (all Luhn-valid).
// Phones are 555-01xx (the reserved fictional range). Emails point at a
// non-resolvable .local domain.

fn seed_users() -> Vec<TestUser> {
    let routing = "021000021"; // valid ABA checksum; commonly used in test fixtures

    let presets: &[(&str, &str, &str, &str, &str, &str, &str, &[(&str, &str, &str)])] = &[
        (
            "Test Alice Tester",
            "Alice", "Tester", "900-11-1111", "1990-01-15",
            "alice.tester@test.fnba.local", "555-010-0001",
            &[("4242424242424242", "12/29", "123")],
        ),
        (
            "Test Bob Sample",
            "Bob", "Sample", "900-22-2222", "1985-03-22",
            "bob.sample@test.fnba.local", "555-010-0002",
            &[("5555555555554444", "11/28", "234")],
        ),
        (
            "Test Carol Demo",
            "Carol", "Demo", "900-33-3333", "1978-07-04",
            "carol.demo@test.fnba.local", "555-010-0003",
            &[("378282246310005", "10/27", "3456")], // Amex
        ),
        (
            "Test Dave Mock",
            "Dave", "Mock", "900-44-4444", "1992-11-30",
            "dave.mock@test.fnba.local", "555-010-0004",
            &[("6011111111111117", "09/26", "345")], // Discover
        ),
        (
            "Test Erin Trial",
            "Erin", "Trial", "900-55-5555", "1980-04-12",
            "erin.trial@test.fnba.local", "555-010-0005",
            &[("4000056655665556", "08/29", "456")],
        ),
        (
            "Test Frank Faux",
            "Frank", "Faux", "900-66-6666", "1975-09-08",
            "frank.faux@test.fnba.local", "555-010-0006",
            &[("5105105105105100", "07/28", "567")],
        ),
        (
            "Test Grace Stub",
            "Grace", "Stub", "900-77-7777", "1995-02-19",
            "grace.stub@test.fnba.local", "555-010-0007",
            &[("4012888888881881", "06/27", "678")],
        ),
        (
            "Test Harry Dummy",
            "Harry", "Dummy", "900-88-8888", "1988-12-25",
            "harry.dummy@test.fnba.local", "555-010-0008",
            &[("2223003122003222", "05/29", "789")], // Mastercard 2-series
        ),
        (
            "Test Ivy Placeholder",
            "Ivy", "Placeholder", "900-99-9999", "1970-06-30",
            "ivy.placeholder@test.fnba.local", "555-010-0009",
            &[("4222222222222", "04/27", "890")], // 13-digit Visa
        ),
        (
            "Test Jack Proxy",
            "Jack", "Proxy", "900-10-1010", "1983-08-17",
            "jack.proxy@test.fnba.local", "555-010-0010",
            &[("4111111111111111", "03/28", "901")],
        ),
    ];

    presets
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let acct = format!("1000{:08}", 10_000_001 + i as i64);
            TestUser {
                id: None,
                label: p.0.to_string(),
                first_name: Some(p.1.to_string()),
                last_name: Some(p.2.to_string()),
                ssn: Some(p.3.to_string()),
                dob: Some(p.4.to_string()),
                email: Some(p.5.to_string()),
                phone: Some(p.6.to_string()),
                address: Some(format!("{} Test Lane, Springfield, IL 62701", 100 + i * 10)),
                account_num: Some(acct),
                routing_num: Some(routing.to_string()),
                cards: p
                    .7
                    .iter()
                    .map(|(n, e, c)| TestCard {
                        number: n.to_string(),
                        expiry: e.to_string(),
                        cvv: c.to_string(),
                    })
                    .collect(),
                enabled: true,
            }
        })
        .collect()
}
