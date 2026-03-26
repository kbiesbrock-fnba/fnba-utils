use crate::db;
use crate::models::identity::{AssumeIdentityResult, IdentityData, IdentityState, IdentityUser};

const DEFAULT_DATA: &str = include_str!("../../../../data/identity-defaults.json");

#[derive(serde::Deserialize)]
struct DefaultsFile {
    imposter: String,
    #[serde(rename = "defaultUsers")]
    default_users: Vec<RawUser>,
    #[serde(rename = "defaultConnections")]
    default_connections: Vec<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct RawUser {
    label: String,
    username: String,
}

#[derive(serde::Deserialize)]
struct CustomData {
    #[serde(rename = "CustomUsers", default)]
    custom_users: Vec<RawUser>,
    #[serde(rename = "CustomConnections", default)]
    custom_connections: Vec<String>,
}

/// Return one entry per label+username pair (no deduplication).
fn build_user_list(users: &[RawUser]) -> Vec<IdentityUser> {
    users
        .iter()
        .map(|u| IdentityUser {
            username: u.username.clone(),
            label: u.label.clone(),
        })
        .collect()
}

#[tauri::command]
pub async fn get_identity_data() -> Result<IdentityData, String> {
    let defaults: DefaultsFile =
        serde_json::from_str(DEFAULT_DATA).map_err(|e| format!("Failed to parse defaults: {e}"))?;

    let mut all_users = defaults.default_users;
    let mut all_connections = defaults.default_connections;

    // Merge custom data from ~/.assumeIdentity.json
    if let Some(home) = dirs::home_dir() {
        let custom_path = home.join(".assumeIdentity.json");
        if custom_path.exists() {
            if let Ok(contents) = std::fs::read_to_string(&custom_path) {
                if let Ok(custom) = serde_json::from_str::<CustomData>(&contents) {
                    all_users.extend(custom.custom_users);
                    for conn in custom.custom_connections {
                        if !all_connections.iter().any(|c| c.eq_ignore_ascii_case(&conn)) {
                            all_connections.push(conn);
                        }
                    }
                }
            }
        }
    }

    Ok(IdentityData {
        imposter: defaults.imposter,
        users: build_user_list(&all_users),
        connections: all_connections,
    })
}

#[tauri::command]
pub async fn execute_assume_identity(
    user: String,
    connection: String,
) -> Result<AssumeIdentityResult, String> {
    let defaults: DefaultsFile =
        serde_json::from_str(DEFAULT_DATA).map_err(|e| format!("Failed to parse defaults: {e}"))?;
    let imposter = &defaults.imposter;
    let login = format!("FNBA\\{imposter}");

    let mut client = db::connect(&connection).await?;

    match db::assume_identity(&mut client, imposter, &user).await? {
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
                message: Some(if password_changed {
                    "Identity switched successfully.".into()
                } else {
                    "WARNING: Password did not change - the identity switch may have failed."
                        .into()
                }),
            })
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct CustomDataWrite {
    #[serde(rename = "CustomUsers", default)]
    custom_users: Vec<RawUser>,
    #[serde(rename = "CustomConnections", default)]
    custom_connections: Vec<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
    added_user: bool,
    added_connection: bool,
}

#[tauri::command]
pub async fn save_custom_entry(
    user: Option<String>,
    connection: Option<String>,
) -> Result<SaveResult, String> {
    let home = dirs::home_dir().ok_or("Could not determine home directory")?;
    let custom_path = home.join(".assumeIdentity.json");

    let mut data: CustomDataWrite = if custom_path.exists() {
        let contents =
            std::fs::read_to_string(&custom_path).map_err(|e| format!("Read error: {e}"))?;
        serde_json::from_str(&contents).unwrap_or_default()
    } else {
        CustomDataWrite::default()
    };

    let mut added_user = false;
    let mut added_connection = false;

    // Load defaults to check against both lists
    let defaults: DefaultsFile =
        serde_json::from_str(DEFAULT_DATA).map_err(|e| format!("Failed to parse defaults: {e}"))?;

    if let Some(username) = user {
        let in_defaults = defaults
            .default_users
            .iter()
            .any(|u| u.username.eq_ignore_ascii_case(&username));
        let in_custom = data
            .custom_users
            .iter()
            .any(|u| u.username.eq_ignore_ascii_case(&username));
        if !in_defaults && !in_custom {
            data.custom_users.push(RawUser {
                label: "Custom".to_string(),
                username,
            });
            added_user = true;
        }
    }

    if let Some(conn) = connection {
        let in_defaults = defaults
            .default_connections
            .iter()
            .any(|c| c.eq_ignore_ascii_case(&conn));
        let in_custom = data
            .custom_connections
            .iter()
            .any(|c| c.eq_ignore_ascii_case(&conn));
        if !in_defaults && !in_custom {
            data.custom_connections.push(conn);
            added_connection = true;
        }
    }

    if added_user || added_connection {
        let json = serde_json::to_string_pretty(&data)
            .map_err(|e| format!("Serialization error: {e}"))?;
        std::fs::write(&custom_path, json).map_err(|e| format!("Write error: {e}"))?;
    }

    Ok(SaveResult {
        added_user,
        added_connection,
    })
}

