//! Shared FNBA directory search — used by both the Right Lookup command and
//! Assume Identity's live user search. Everything that resolves people, rights,
//! and Windows logins lives here so the two commands share one implementation.
//!
//! Result rows are the unified [`RightAssociate`] "directory person": every row
//! carries the person's Windows `login` (when they have one) plus a `job_title`
//! / `department` role hint, so any search result — whether found by name or by
//! "holders of a right" — can be assumed and pinned to favorites.

use crate::db;
use crate::models::directory::{RightAssociate, RightInfo};
use tiberius::Row;

/// Read an optional `varchar`/`char` column as an owned `String`.
fn opt_str(row: &Row, col: &str) -> Result<Option<String>, String> {
    Ok(row
        .try_get::<&str, _>(col)
        .map_err(|e| format!("Column read error: {e}"))?
        .map(|s| s.to_string()))
}

/// Strip a leading `DOMAIN\` from an `associate_login.domain_username`
/// (e.g. `FNBA\jsmith` -> `jsmith`) to get the bare username the
/// `logincheck.fnba.assumeIdentity` proc expects.
fn strip_domain(login: &str) -> String {
    match login.rsplit_once('\\') {
        Some((_, user)) => user.to_string(),
        None => login.to_string(),
    }
}

/// Escape SQL Server LIKE-pattern metacharacters in user input so a literal
/// `_`, `%`, `[`, or `\` stays literal. Used in pair with `ESCAPE '\'` on the
/// LIKE expression. Without this, typing `_` matches any single char and `[a]`
/// becomes a character class — either gives broken / over-broad results.
fn escape_like(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '\\' | '%' | '_' | '[') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Parse a directory-person row. Callers must select the columns
/// `assoc_id, nickname, first_name, last_name, job_title, department,
/// domain_username` (department pre-`RTRIM`med).
fn parse_associate(row: &Row) -> Result<RightAssociate, String> {
    Ok(RightAssociate {
        assoc_id: row
            .try_get::<i32, _>("assoc_id")
            .map_err(|e| format!("Column read error: {e}"))?
            .ok_or("assoc_id was NULL")?,
        nickname: opt_str(row, "nickname")?,
        first_name: opt_str(row, "first_name")?,
        last_name: opt_str(row, "last_name")?,
        login: opt_str(row, "domain_username")?.map(|d| strip_domain(&d)),
        job_title: opt_str(row, "job_title")?,
        department: opt_str(row, "department")?,
    })
}

fn parse_right(row: &Row) -> Result<RightInfo, String> {
    let right_id: i32 = row
        .try_get("right_id")
        .map_err(|e| format!("Column read error: {e}"))?
        .ok_or("right_id was NULL")?;
    let right_name: &str = row
        .try_get("right_name")
        .map_err(|e| format!("Column read error: {e}"))?
        .ok_or("right_name was NULL")?;
    Ok(RightInfo {
        right_id,
        right_name: right_name.to_string(),
    })
}

#[tauri::command]
pub async fn get_all_rights(server: String) -> Result<Vec<RightInfo>, String> {
    let mut client = db::connect_to(&server, "notedb").await?;

    let sql = "SELECT right_id, right_name FROM notedb.fnba.rights ORDER BY right_name";

    let rows = client
        .query(sql, &[])
        .await
        .map_err(|e| format!("Rights query failed: {e}"))?
        .into_first_result()
        .await
        .map_err(|e| format!("Rights result read failed: {e}"))?;

    rows.iter().map(parse_right).collect()
}

/// Holders of a right. `LEFT JOIN perdb.fnba.associate` keeps orphan
/// `user_groups` rows visible (a row whose `assoc_id` no longer matches an
/// associate still surfaces in audits, just with NULL person fields), matching
/// the pre-refactor behavior. `LEFT JOIN associate_login` keeps people without
/// a Windows login visible too. The canonical login per holder is picked by
/// `OUTER APPLY` ordered by `date_modified DESC`, so the *most recently
/// touched* login wins rather than the alphabetically-smallest one.
#[tauri::command]
pub async fn get_right_associates(
    server: String,
    right_name: Option<String>,
    right_id: Option<i32>,
) -> Result<Vec<RightAssociate>, String> {
    if right_name.is_none() && right_id.is_none() {
        return Err("Either right_name or right_id must be provided".into());
    }

    let mut client = db::connect_to(&server, "notedb").await?;

    let sql = "\
DECLARE @rightId INT = @P1;
DECLARE @rightName VARCHAR(60) = @P2;

SELECT
    ug.assoc_id,
    per.nickname,
    per.first_name,
    per.last_name,
    per.job_title,
    RTRIM(dept.name) AS department,
    MAX(canonical.domain_username) AS domain_username
FROM notedb.fnba.rights r
JOIN notedb.fnba.group_rights gr ON gr.right_id = r.right_id
JOIN notedb.fnba.user_groups ug ON ug.right_group_id = gr.right_group_id
LEFT JOIN perdb.fnba.associate per ON per.assoc_id = ug.assoc_id
LEFT JOIN perdb.fnba.department dept ON dept.department_id = per.department_id
OUTER APPLY (
    SELECT TOP 1 al.domain_username
    FROM logincheck.fnba_reporting.associate_login al
    WHERE al.assoc_id = ug.assoc_id
    ORDER BY al.date_modified DESC, al.domain_username ASC
) canonical
WHERE
    (@rightName IS NOT NULL AND r.right_name = @rightName)
    OR
    (@rightId IS NOT NULL AND r.right_id = @rightId)
GROUP BY ug.assoc_id, per.nickname, per.first_name, per.last_name, per.job_title, RTRIM(dept.name)
ORDER BY per.nickname";

    let rows = client
        .query(sql, &[&right_id, &right_name])
        .await
        .map_err(|e| format!("Associates query failed: {e}"))?
        .into_first_result()
        .await
        .map_err(|e| format!("Associates result read failed: {e}"))?;

    rows.iter().map(parse_associate).collect()
}

/// Live people search by nickname / first / last name / Windows login.
/// `LEFT JOIN associate_login` keeps people without a Windows login surfaced
/// (consistent with `get_right_associates`); they render as `noLogin` rows in
/// the picker. User input is escaped against SQL Server LIKE metacharacters so
/// a literal `_` / `%` / `[` doesn't act as a wildcard or character class.
/// Canonical login per person prefers one matching the search text (so typing
/// a login shows that exact login even if it's not the person's most-recent
/// one); name-only matches fall back to most-recent `date_modified`.
#[tauri::command]
pub async fn search_associates(
    server: String,
    query: String,
) -> Result<Vec<RightAssociate>, String> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }
    let escaped = escape_like(query.trim());

    let mut client = db::connect_to(&server, "logincheck").await?;

    let sql = "\
DECLARE @search VARCHAR(100) = @P1;

SELECT TOP 25
    a.assoc_id,
    a.nickname,
    a.first_name,
    a.last_name,
    a.job_title,
    RTRIM(d.name) AS department,
    MAX(canonical.domain_username) AS domain_username
FROM perdb.fnba.associate a
LEFT JOIN logincheck.fnba_reporting.associate_login al ON al.assoc_id = a.assoc_id
LEFT JOIN perdb.fnba.department d ON d.department_id = a.department_id
OUTER APPLY (
    SELECT TOP 1 al2.domain_username
    FROM logincheck.fnba_reporting.associate_login al2
    WHERE al2.assoc_id = a.assoc_id
    ORDER BY CASE WHEN al2.domain_username LIKE '%' + @search + '%' ESCAPE '\\' THEN 0 ELSE 1 END,
             al2.date_modified DESC, al2.domain_username ASC
) canonical
WHERE a.nickname LIKE '%' + @search + '%' ESCAPE '\\'
   OR a.first_name LIKE '%' + @search + '%' ESCAPE '\\'
   OR a.last_name LIKE '%' + @search + '%' ESCAPE '\\'
   OR al.domain_username LIKE '%' + @search + '%' ESCAPE '\\'
GROUP BY a.assoc_id, a.nickname, a.first_name, a.last_name, a.job_title, RTRIM(d.name)
ORDER BY a.nickname";

    let rows = client
        .query(sql, &[&escaped])
        .await
        .map_err(|e| format!("Associate search failed: {e}"))?
        .into_first_result()
        .await
        .map_err(|e| format!("Associate search result read failed: {e}"))?;

    rows.iter().map(parse_associate).collect()
}

#[tauri::command]
pub async fn get_associate_rights(
    server: String,
    assoc_id: i32,
) -> Result<Vec<RightInfo>, String> {
    let mut client = db::connect_to(&server, "notedb").await?;

    let sql = "\
SELECT DISTINCT r.right_id, r.right_name
FROM notedb.fnba.rights r
JOIN notedb.fnba.group_rights gr ON gr.right_id = r.right_id
JOIN notedb.fnba.user_groups ug ON ug.right_group_id = gr.right_group_id
WHERE ug.assoc_id = @P1
ORDER BY r.right_name";

    let rows = client
        .query(sql, &[&assoc_id])
        .await
        .map_err(|e| format!("Associate rights query failed: {e}"))?
        .into_first_result()
        .await
        .map_err(|e| format!("Associate rights result read failed: {e}"))?;

    rows.iter().map(parse_right).collect()
}

/// Resolve a single assumable Windows login for an associate. Picks the most
/// recently-modified login (rather than the alphabetic minimum), matching the
/// canonical-login choice used by `search_associates` / `get_right_associates`.
/// Returns the bare username (no `DOMAIN\`), or `None` if no login row exists.
#[tauri::command]
pub async fn get_assume_login(server: String, assoc_id: i32) -> Result<Option<String>, String> {
    let mut client = db::connect_to(&server, "logincheck").await?;

    let sql = "\
SELECT TOP 1 domain_username
FROM logincheck.fnba_reporting.associate_login
WHERE assoc_id = @P1
ORDER BY date_modified DESC, domain_username ASC";

    let row = client
        .query(sql, &[&assoc_id])
        .await
        .map_err(|e| format!("Login resolve query failed: {e}"))?
        .into_row()
        .await
        .map_err(|e| format!("Login resolve result read failed: {e}"))?;

    Ok(row.and_then(|r| {
        r.try_get::<&str, _>("domain_username")
            .ok()
            .flatten()
            .map(strip_domain)
    }))
}
