use crate::db;
use crate::models::identity::{
    AssumeIdentityResult, IdentityConnection, IdentityData, IdentityImposter, IdentityState,
    IdentityUser,
};
use std::collections::HashMap;

const DEFAULT_DATA: &str = include_str!("../../../../data/identity-defaults.json");

#[derive(serde::Deserialize)]
struct DefaultsFile {
    #[serde(default)]
    imposters: Vec<String>,
    connections: Vec<IdentityConnection>,
}

fn get_windows_username() -> Result<String, String> {
    std::env::var("USERNAME")
        .map_err(|_| "Could not determine Windows username (USERNAME env var not set)".to_string())
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct CustomData {
    #[serde(rename = "Imposters", default)]
    imposters: Vec<String>,
    #[serde(rename = "Users", default)]
    users: Vec<IdentityUser>,
    #[serde(
        rename = "Connections",
        default,
        deserialize_with = "deserialize_connections"
    )]
    connections: Vec<IdentityConnection>,
    /// Last-assumed timestamp per favorite (epoch millis), keyed by composite
    /// `label␟username`. Drives the recency-based hot-pick ordering — the
    /// most-recently-assumed favorite renders as #1. Never-assumed favorites
    /// (LastUsed = 0) sort by their position in the pinned Users list (stable sort).
    #[serde(rename = "LastUsed", default)]
    last_used: HashMap<String, i64>,
}

/// Stable key identifying one favorite. A username can appear under several
/// labels (e.g. one person pinned as both BSA and Customer Service), so
/// ordering keys on the `label`+`username` pair, not the username alone.
/// Must match the frontend's key (`${label}${username}`).
fn fav_key(label: &str, username: &str) -> String {
    format!("{label}\u{1f}{username}")
}

/// Sort favorites by recency of use, most recent first. Items never assumed
/// keep their input order (stable sort) — i.e. the order they were pinned in.
fn sort_favorites_by_recency(users: &mut [IdentityUser], last_used: &HashMap<String, i64>) {
    users.sort_by(|a, b| {
        let ta = last_used
            .get(&fav_key(&a.label, &a.username))
            .copied()
            .unwrap_or(0);
        let tb = last_used
            .get(&fav_key(&b.label, &b.username))
            .copied()
            .unwrap_or(0);
        tb.cmp(&ta)
    });
}

/// Atomic JSON write: stage the new bytes to a sibling `.tmp` file, then
/// `rename` it over the live file. A crash, AV interception, or power loss in
/// the middle leaves either the old file or the new file — never a half-written
/// one. The previous `std::fs::write` could truncate the file mid-write and
/// then every subsequent `serde_json::from_str` would fail, hard-breaking the
/// feature until a manual delete.
fn write_atomic_json(path: &std::path::Path, contents: &str) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, contents).map_err(|e| format!("Write error: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("Rename error: {e}")
    })
}

fn deserialize_connections<'de, D>(deserializer: D) -> Result<Vec<IdentityConnection>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;

    struct ConnectionVisitor;

    impl<'de> de::Visitor<'de> for ConnectionVisitor {
        type Value = Vec<IdentityConnection>;

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("an array of strings or {label,server} objects")
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: de::SeqAccess<'de>,
        {
            let mut out = Vec::new();
            while let Some(val) = seq.next_element::<serde_json::Value>()? {
                match val {
                    serde_json::Value::String(s) => {
                        out.push(IdentityConnection {
                            label: "Local".to_string(),
                            server: s,
                            is_custom: false,
                        });
                    }
                    serde_json::Value::Object(_) => {
                        let conn: IdentityConnection = serde_json::from_value(val)
                            .map_err(de::Error::custom)?;
                        out.push(conn);
                    }
                    _ => return Err(de::Error::custom("expected string or object")),
                }
            }
            Ok(out)
        }
    }

    deserializer.deserialize_seq(ConnectionVisitor)
}

/// Load all connections (defaults + custom), sorted with "Local" first.
/// Returns `(current_user, connections)`.
pub fn load_all_connections() -> Result<(String, Vec<IdentityConnection>), String> {
    let current_user = get_windows_username()?;
    let defaults: DefaultsFile =
        serde_json::from_str(DEFAULT_DATA).map_err(|e| format!("Failed to parse defaults: {e}"))?;

    let mut all_connections = defaults.connections;

    let custom_path = crate::state::paths::data_file("assumeIdentity.json");
    if custom_path.exists() {
        let contents = std::fs::read_to_string(&custom_path)
            .map_err(|e| format!("Failed to read {}: {e}", custom_path.display()))?;
        let custom: CustomData = serde_json::from_str(&contents)
            .map_err(|e| format!("Custom config {} is malformed: {e}", custom_path.display()))?;
        for conn in custom.connections {
            if !all_connections
                .iter()
                .any(|c| c.server.eq_ignore_ascii_case(&conn.server))
            {
                all_connections.push(IdentityConnection {
                    label: conn.label,
                    server: conn.server,
                    is_custom: true,
                });
            }
        }
    }

    all_connections.sort_by(|a, b| {
        let a_local = a.label.eq_ignore_ascii_case("local");
        let b_local = b.label.eq_ignore_ascii_case("local");
        match (a_local, b_local) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.label.cmp(&b.label),
        }
    });

    Ok((current_user, all_connections))
}

/// Return one entry per label+username pair (no deduplication).
#[tauri::command]
pub async fn get_identity_data() -> Result<IdentityData, String> {
    let (current_user, all_connections) = load_all_connections()?;

    let defaults: DefaultsFile =
        serde_json::from_str(DEFAULT_DATA).map_err(|e| format!("Failed to parse defaults: {e}"))?;

    // Build imposters list: current user first, then defaults, then custom (deduped)
    let mut imposters = vec![IdentityImposter {
        name: current_user.clone(),
        is_custom: false,
    }];
    for imp in &defaults.imposters {
        if !imposters.iter().any(|i| i.name.eq_ignore_ascii_case(imp)) {
            imposters.push(IdentityImposter {
                name: imp.clone(),
                is_custom: false,
            });
        }
    }

    let custom_path = crate::state::paths::data_file("assumeIdentity.json");
    let mut custom: CustomData = if custom_path.exists() {
        let contents = std::fs::read_to_string(&custom_path)
            .map_err(|e| format!("Failed to read {}: {e}", custom_path.display()))?;
        serde_json::from_str(&contents)
            .map_err(|e| format!("Custom config {} is malformed: {e}", custom_path.display()))?
    } else {
        CustomData::default()
    };

    for imp in &custom.imposters {
        if !imposters.iter().any(|i| i.name.eq_ignore_ascii_case(imp)) {
            imposters.push(IdentityImposter {
                name: imp.clone(),
                is_custom: true,
            });
        }
    }

    // One-time migration: `remove_favorite` strips a pin's LastUsed stamp on
    // unpin, so a LastUsed entry with no matching Users pin can only be a
    // retired shipped default the operator actually used — pin it so it
    // survives the defaults removal. Idempotent: once pinned, every stamp
    // matches a Users entry and this loop has nothing left to do.
    let mut migrated: Vec<IdentityUser> = Vec::new();
    for key in custom.last_used.keys() {
        if custom
            .users
            .iter()
            .any(|u| fav_key(&u.label, &u.username) == *key)
        {
            continue;
        }
        if let Some((label, username)) = key.split_once('\u{1f}') {
            migrated.push(IdentityUser {
                username: username.to_string(),
                label: label.to_string(),
                is_custom: true,
            });
        }
    }
    if !migrated.is_empty() {
        custom.users.extend(migrated);
        let json = serde_json::to_string_pretty(&custom)
            .map_err(|e| format!("Serialization error: {e}"))?;
        write_atomic_json(&custom_path, &json)?;
    }

    let mut all_users: Vec<IdentityUser> = custom.users;
    for user in &mut all_users {
        user.is_custom = true;
    }

    // Bubble the most-recently-assumed favorites to the top so the frontend's
    // 1–9 hot-pick digits track real usage.
    sort_favorites_by_recency(&mut all_users, &custom.last_used);

    Ok(IdentityData {
        current_user,
        imposters,
        users: all_users,
        connections: all_connections,
    })
}

#[tauri::command]
pub async fn execute_assume_identity(
    imposter: String,
    user: String,
    connection: String,
) -> Result<AssumeIdentityResult, String> {
    let imposter = imposter.trim().to_string();
    let user = user.trim().to_string();
    let connection = connection.trim().to_string();

    if imposter.is_empty() {
        return Err("Imposter login cannot be empty".into());
    }
    if user.is_empty() {
        return Err("User login cannot be empty".into());
    }
    if connection.is_empty() {
        return Err("Connection server cannot be empty".into());
    }

    let login = format!("FNBA\\{imposter}");

    let mut client = db::connect(&connection).await?;

    match db::assume_identity(&mut client, &imposter, &user).await? {
        db::SwitchOutcome::ImposterNotFound => {
            Err(format!("Login {login} not found on {connection}"))
        }

        db::SwitchOutcome::AlreadyAssuming {
            acting_as_login,
            acting_as_name,
            password,
            changed_at,
            on_host,
        } => Ok(AssumeIdentityResult {
            server: connection,
            login,
            before: None,
            after: Some(IdentityState {
                acting_as_login,
                acting_as_name,
                password,
                changed_at,
                on_host,
            }),
            password_changed: false,
            already_assuming: true,
            message: Some("Already acting as this identity - no change needed.".into()),
        }),

        db::SwitchOutcome::Switched { before, after } => {
            let password_changed = before.password != after.password;
            Ok(AssumeIdentityResult {
                server: connection,
                login,
                before: Some(before),
                after: Some(after),
                password_changed,
                already_assuming: false,
                message: if password_changed {
                    None
                } else {
                    Some("Password did not change — the switch may have failed.".into())
                },
            })
        }
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
    added_connection: bool,
    added_imposter: bool,
}

#[tauri::command]
pub async fn save_custom_entry(
    connection: Option<String>,
    connection_label: Option<String>,
    imposter: Option<String>,
) -> Result<SaveResult, String> {
    let custom_path = crate::state::paths::data_file("assumeIdentity.json");

    let mut data: CustomData = if custom_path.exists() {
        let contents =
            std::fs::read_to_string(&custom_path).map_err(|e| format!("Read error: {e}"))?;
        serde_json::from_str(&contents).map_err(|e| {
            format!("Custom config {} is malformed: {e}", custom_path.display())
        })?
    } else {
        CustomData::default()
    };

    let mut added_connection = false;
    let mut added_imposter = false;

    // Load defaults to check against both lists
    let defaults: DefaultsFile =
        serde_json::from_str(DEFAULT_DATA).map_err(|e| format!("Failed to parse defaults: {e}"))?;

    if let Some(conn) = connection {
        let in_defaults = defaults
            .connections
            .iter()
            .any(|c| c.server.eq_ignore_ascii_case(&conn));
        let in_custom = data
            .connections
            .iter()
            .any(|c| c.server.eq_ignore_ascii_case(&conn));
        if !in_defaults && !in_custom {
            data.connections.push(IdentityConnection {
                label: connection_label.unwrap_or_else(|| "Local".to_string()),
                server: conn,
                is_custom: false,
            });
            added_connection = true;
        }
    }

    if let Some(imp) = imposter {
        let in_defaults = defaults
            .imposters
            .iter()
            .any(|i| i.eq_ignore_ascii_case(&imp));
        let in_custom = data
            .imposters
            .iter()
            .any(|i| i.eq_ignore_ascii_case(&imp));
        if !in_defaults && !in_custom {
            data.imposters.push(imp);
            added_imposter = true;
        }
    }

    if added_connection || added_imposter {
        let json = serde_json::to_string_pretty(&data)
            .map_err(|e| format!("Serialization error: {e}"))?;
        write_atomic_json(&custom_path, &json)?;
    }

    Ok(SaveResult {
        added_connection,
        added_imposter,
    })
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteResult {
    deleted_user: bool,
    deleted_connection: bool,
    deleted_imposter: bool,
}

#[tauri::command]
pub async fn delete_custom_entry(
    user: Option<String>,
    connection: Option<String>,
    imposter: Option<String>,
) -> Result<DeleteResult, String> {
    let custom_path = crate::state::paths::data_file("assumeIdentity.json");

    if !custom_path.exists() {
        return Ok(DeleteResult {
            deleted_user: false,
            deleted_connection: false,
            deleted_imposter: false,
        });
    }

    let contents =
        std::fs::read_to_string(&custom_path).map_err(|e| format!("Read error: {e}"))?;
    let mut data: CustomData = serde_json::from_str(&contents)
        .map_err(|e| format!("Custom config {} is malformed: {e}", custom_path.display()))?;

    let mut deleted_user = false;
    let mut deleted_connection = false;
    let mut deleted_imposter = false;

    if let Some(username) = user {
        let before = data.users.len();
        data.users
            .retain(|u| !u.username.eq_ignore_ascii_case(&username));
        deleted_user = data.users.len() < before;
    }

    if let Some(server) = connection {
        let before = data.connections.len();
        data.connections
            .retain(|c| !c.server.eq_ignore_ascii_case(&server));
        deleted_connection = data.connections.len() < before;
    }

    if let Some(imp) = imposter {
        let before = data.imposters.len();
        data.imposters
            .retain(|i| !i.eq_ignore_ascii_case(&imp));
        deleted_imposter = data.imposters.len() < before;
    }

    if deleted_user || deleted_connection || deleted_imposter {
        let json = serde_json::to_string_pretty(&data)
            .map_err(|e| format!("Serialization error: {e}"))?;
        write_atomic_json(&custom_path, &json)?;
    }

    Ok(DeleteResult {
        deleted_user,
        deleted_connection,
        deleted_imposter,
    })
}

/// Explicitly pin a searched user to the distributable favorites list. Replaces
/// the old auto-save-on-assume behavior now that the user picker is a live DB
/// search — only users the operator deliberately pins are kept. Returns `true`
/// if a new favorite was written, `false` if it already existed under `label`.
#[tauri::command]
pub async fn pin_favorite(username: String, label: String) -> Result<bool, String> {
    let username = username.trim().to_string();
    if username.is_empty() {
        return Err("Username cannot be empty".into());
    }
    let label = {
        let l = label.trim();
        if l.is_empty() {
            "Other".to_string()
        } else {
            l.to_string()
        }
    };

    let custom_path = crate::state::paths::data_file("assumeIdentity.json");
    let mut data: CustomData = if custom_path.exists() {
        let contents =
            std::fs::read_to_string(&custom_path).map_err(|e| format!("Read error: {e}"))?;
        serde_json::from_str(&contents)
            .map_err(|e| format!("Custom config {} is malformed: {e}", custom_path.display()))?
    } else {
        CustomData::default()
    };

    let already_favorite = data.users.iter().any(|u| {
        u.username.eq_ignore_ascii_case(&username) && u.label.eq_ignore_ascii_case(&label)
    });
    if already_favorite {
        // Already an explicit favorite — nothing to do.
        return Ok(false);
    }

    data.users.push(IdentityUser {
        username,
        label,
        is_custom: true,
    });

    let json =
        serde_json::to_string_pretty(&data).map_err(|e| format!("Serialization error: {e}"))?;
    write_atomic_json(&custom_path, &json)?;
    Ok(true)
}

/// Remove a favorite from view. Deletes the matching entry from the custom
/// Users list outright and strips its `LastUsed` stamp. Idempotent — safe to
/// call on an already-removed entry.
#[tauri::command]
pub async fn remove_favorite(label: String, username: String) -> Result<(), String> {
    let label = label.trim().to_string();
    let username = username.trim().to_string();
    if label.is_empty() || username.is_empty() {
        return Err("label and username are required".into());
    }
    let key = fav_key(&label, &username);

    let custom_path = crate::state::paths::data_file("assumeIdentity.json");
    let mut data: CustomData = if custom_path.exists() {
        let contents =
            std::fs::read_to_string(&custom_path).map_err(|e| format!("Read error: {e}"))?;
        serde_json::from_str(&contents)
            .map_err(|e| format!("Custom config {} is malformed: {e}", custom_path.display()))?
    } else {
        CustomData::default()
    };

    // Drop the matching custom user and its recency timestamp. The LastUsed
    // strip matters beyond tidiness: get_identity_data's migration treats any
    // orphaned LastUsed stamp as "re-pin this retired default", so leaving the
    // stamp behind would resurrect an entry the operator just unpinned.
    data.users.retain(|u| {
        !(u.label.eq_ignore_ascii_case(&label) && u.username.eq_ignore_ascii_case(&username))
    });
    data.last_used.remove(&key);

    let json =
        serde_json::to_string_pretty(&data).map_err(|e| format!("Serialization error: {e}"))?;
    write_atomic_json(&custom_path, &json)?;
    Ok(())
}

/// Stamp a favorite's `LastUsed` to "now" after a successful assume. This is
/// what drives recency-based ordering: the next [`get_identity_data`] sees the
/// updated timestamp and floats this favorite to #1. Safe to call even when
/// the (label, username) isn't currently a favorite — the entry just sits in
/// `LastUsed` and becomes active if the user later pins that pair.
#[tauri::command]
pub async fn mark_favorite_used(label: String, username: String) -> Result<(), String> {
    let label = label.trim().to_string();
    let username = username.trim().to_string();
    if label.is_empty() || username.is_empty() {
        return Err("label and username are required".into());
    }
    let key = fav_key(&label, &username);

    let custom_path = crate::state::paths::data_file("assumeIdentity.json");
    let mut data: CustomData = if custom_path.exists() {
        let contents =
            std::fs::read_to_string(&custom_path).map_err(|e| format!("Read error: {e}"))?;
        serde_json::from_str(&contents)
            .map_err(|e| format!("Custom config {} is malformed: {e}", custom_path.display()))?
    } else {
        CustomData::default()
    };

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    data.last_used.insert(key, now_ms);

    let json =
        serde_json::to_string_pretty(&data).map_err(|e| format!("Serialization error: {e}"))?;
    write_atomic_json(&custom_path, &json)?;
    Ok(())
}

