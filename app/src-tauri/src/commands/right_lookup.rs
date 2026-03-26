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
        .filter_map(|row| {
            let right_id: i32 = row.try_get("right_id").ok()??;
            let right_name: &str = row.try_get("right_name").ok()??;
            Some(RightInfo {
                right_id,
                right_name: right_name.to_string(),
            })
        })
        .collect();

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
