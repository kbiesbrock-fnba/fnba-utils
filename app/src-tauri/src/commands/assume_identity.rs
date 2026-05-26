use crate::db;
use crate::models::identity::{
    AssumeIdentityResult, IdentityConnection, IdentityData, IdentityImposter, IdentityState,
    IdentityUser,
};

const DEFAULT_DATA: &str = include_str!("../../../../data/identity-defaults.json");

#[derive(serde::Deserialize)]
struct DefaultsFile {
    #[serde(default)]
    imposters: Vec<String>,
    users: Vec<IdentityUser>,
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

    let mut all_users = defaults.users;

    // Merge custom imposters and users from assumeIdentity.json
    {
        let custom_path = crate::state::paths::data_file("assumeIdentity.json");
        if custom_path.exists() {
            let contents = std::fs::read_to_string(&custom_path)
                .map_err(|e| format!("Failed to read {}: {e}", custom_path.display()))?;
            let custom: CustomData = serde_json::from_str(&contents)
                .map_err(|e| format!("Custom config {} is malformed: {e}", custom_path.display()))?;
            for imp in custom.imposters {
                if !imposters
                    .iter()
                    .any(|i| i.name.eq_ignore_ascii_case(&imp))
                {
                    imposters.push(IdentityImposter {
                        name: imp,
                        is_custom: true,
                    });
                }
            }
            for mut user in custom.users {
                user.is_custom = true;
                all_users.push(user);
            }
        }
    }

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
    added_user: bool,
    added_connection: bool,
    added_imposter: bool,
}

#[tauri::command]
pub async fn save_custom_entry(
    user: Option<String>,
    user_label: Option<String>,
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

    let mut added_user = false;
    let mut added_connection = false;
    let mut added_imposter = false;

    // Load defaults to check against both lists
    let defaults: DefaultsFile =
        serde_json::from_str(DEFAULT_DATA).map_err(|e| format!("Failed to parse defaults: {e}"))?;

    if let Some(username) = user {
        let in_defaults = defaults
            .users
            .iter()
            .any(|u| u.username.eq_ignore_ascii_case(&username));
        let in_custom = data
            .users
            .iter()
            .any(|u| u.username.eq_ignore_ascii_case(&username));
        if !in_defaults && !in_custom {
            data.users.push(IdentityUser {
                label: user_label.unwrap_or_else(|| "Other".to_string()),
                username,
                is_custom: false,
            });
            added_user = true;
        }
    }

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

    if added_user || added_connection || added_imposter {
        let json = serde_json::to_string_pretty(&data)
            .map_err(|e| format!("Serialization error: {e}"))?;
        std::fs::write(&custom_path, json).map_err(|e| format!("Write error: {e}"))?;
    }

    Ok(SaveResult {
        added_user,
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
        std::fs::write(&custom_path, json).map_err(|e| format!("Write error: {e}"))?;
    }

    Ok(DeleteResult {
        deleted_user,
        deleted_connection,
        deleted_imposter,
    })
}

