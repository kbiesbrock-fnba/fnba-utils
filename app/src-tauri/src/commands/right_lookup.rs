use crate::db;
use crate::models::right_lookup::{RightAssociate, RightInfo};

const SERVER: &str = "meleagris";

#[tauri::command]
pub async fn get_all_rights() -> Result<Vec<RightInfo>, String> {
    let mut client = db::connect_to(SERVER, "notedb").await?;

    let sql = "SELECT right_id, right_name FROM notedb.fnba.rights ORDER BY right_name";

    let rows = client
        .query(sql, &[])
        .await
        .map_err(|e| format!("Rights query failed: {e}"))?
        .into_first_result()
        .await
        .map_err(|e| format!("Rights result read failed: {e}"))?;

    let rights: Vec<RightInfo> = rows
        .iter()
        .map(|row| {
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
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok(rights)
}

#[tauri::command]
pub async fn get_right_associates(
    right_name: Option<String>,
    right_id: Option<i32>,
) -> Result<Vec<RightAssociate>, String> {
    if right_name.is_none() && right_id.is_none() {
        return Err("Either right_name or right_id must be provided".into());
    }

    let mut client = db::connect_to(SERVER, "notedb").await?;

    let sql = "\
DECLARE @rightId INT = @P1;
DECLARE @rightName VARCHAR(60) = @P2;

SELECT
    per.assoc_id,
    per.nickname,
    per.first_name,
    per.last_name
FROM notedb.fnba.rights r
JOIN notedb.fnba.group_rights gr ON gr.right_id = r.right_id
JOIN notedb.fnba.user_groups ug ON ug.right_group_id = gr.right_group_id
LEFT JOIN perdb.fnba.associate per ON per.assoc_id = ug.assoc_id
WHERE
    (@rightName IS NOT NULL AND r.right_name = @rightName)
    OR
    (@rightId IS NOT NULL AND r.right_id = @rightId)
ORDER BY per.assoc_id";

    let rows = client
        .query(sql, &[&right_id, &right_name])
        .await
        .map_err(|e| format!("Associates query failed: {e}"))?
        .into_first_result()
        .await
        .map_err(|e| format!("Associates result read failed: {e}"))?;

    let associates: Vec<RightAssociate> = rows
        .iter()
        .map(|row| {
            Ok(RightAssociate {
                assoc_id: row
                    .try_get::<i32, _>("assoc_id")
                    .map_err(|e| format!("Column read error: {e}"))?
                    .unwrap_or(0),
                nickname: row
                    .try_get::<&str, _>("nickname")
                    .map_err(|e| format!("Column read error: {e}"))?
                    .map(|s| s.to_string()),
                first_name: row
                    .try_get::<&str, _>("first_name")
                    .map_err(|e| format!("Column read error: {e}"))?
                    .map(|s| s.to_string()),
                last_name: row
                    .try_get::<&str, _>("last_name")
                    .map_err(|e| format!("Column read error: {e}"))?
                    .map(|s| s.to_string()),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok(associates)
}

#[tauri::command]
pub async fn search_associates(query: String) -> Result<Vec<RightAssociate>, String> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }

    let mut client = db::connect_to(SERVER, "notedb").await?;

    let sql = "\
DECLARE @search VARCHAR(100) = @P1;

SELECT TOP 20 assoc_id, nickname, first_name, last_name
FROM perdb.fnba.associate
WHERE nickname LIKE '%' + @search + '%'
   OR first_name LIKE '%' + @search + '%'
   OR last_name LIKE '%' + @search + '%'
ORDER BY nickname";

    let rows = client
        .query(sql, &[&query])
        .await
        .map_err(|e| format!("Associate search failed: {e}"))?
        .into_first_result()
        .await
        .map_err(|e| format!("Associate search result read failed: {e}"))?;

    let associates: Vec<RightAssociate> = rows
        .iter()
        .map(|row| {
            Ok(RightAssociate {
                assoc_id: row
                    .try_get::<i32, _>("assoc_id")
                    .map_err(|e| format!("Column read error: {e}"))?
                    .unwrap_or(0),
                nickname: row
                    .try_get::<&str, _>("nickname")
                    .map_err(|e| format!("Column read error: {e}"))?
                    .map(|s| s.to_string()),
                first_name: row
                    .try_get::<&str, _>("first_name")
                    .map_err(|e| format!("Column read error: {e}"))?
                    .map(|s| s.to_string()),
                last_name: row
                    .try_get::<&str, _>("last_name")
                    .map_err(|e| format!("Column read error: {e}"))?
                    .map(|s| s.to_string()),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok(associates)
}

#[tauri::command]
pub async fn get_associate_rights(assoc_id: i32) -> Result<Vec<RightInfo>, String> {
    let mut client = db::connect_to(SERVER, "notedb").await?;

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

    let rights: Vec<RightInfo> = rows
        .iter()
        .map(|row| {
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
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok(rights)
}
