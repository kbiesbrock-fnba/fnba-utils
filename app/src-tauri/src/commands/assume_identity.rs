use crate::models::identity::{AssumeIdentityResult, IdentityData, IdentityUser};
use std::collections::BTreeMap;
use std::process::Command;

const DEFAULT_DATA: &str = include_str!("../../data/identity-defaults.json");

#[derive(serde::Deserialize)]
struct DefaultsFile {
    imposter: String,
    #[serde(rename = "defaultUsers")]
    default_users: Vec<RawUser>,
    #[serde(rename = "defaultConnections")]
    default_connections: Vec<String>,
}

#[derive(serde::Deserialize)]
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

/// Deduplicate users: one entry per username, all labels joined with " | "
fn build_user_map(users: &[RawUser]) -> Vec<IdentityUser> {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for u in users {
        let labels = map.entry(u.username.clone()).or_default();
        if !u.label.is_empty() && !labels.contains(&u.label) {
            labels.push(u.label.clone());
        }
    }
    map.into_iter()
        .map(|(username, labels)| IdentityUser {
            username,
            labels: labels.join(" | "),
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
        users: build_user_map(&all_users),
        connections: all_connections,
    })
}

#[tauri::command]
pub async fn execute_assume_identity(
    user: String,
    connection: String,
) -> Result<AssumeIdentityResult, String> {
    // Find the json wrapper script relative to the exe or via env
    let script_path = find_script_path()?;

    let output = Command::new("C:\\Program Files\\PowerShell\\7\\pwsh.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-File",
            &script_path,
            &user,
            &connection,
        ])
        .output()
        .map_err(|e| format!("Failed to launch pwsh.exe: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "pwsh.exe exited with {}: {}{}",
            output.status,
            stderr.trim(),
            if stdout.trim().is_empty() {
                String::new()
            } else {
                format!("\n{}", stdout.trim())
            }
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str::<AssumeIdentityResult>(stdout.trim())
        .map_err(|e| format!("Failed to parse script output: {e}\nRaw: {stdout}"))
}

fn find_script_path() -> Result<String, String> {
    // Check environment variable first (for dev)
    if let Ok(path) = std::env::var("FNBA_UTILS_SCRIPT_DIR") {
        let p = std::path::PathBuf::from(&path)
            .join("assumeIdentity")
            .join("assumeIdentity-json.ps1");
        if p.exists() {
            return Ok(p.to_string_lossy().to_string());
        }
    }

    // Check relative to executable (for production bundle)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join("scripts").join("assumeIdentity-json.ps1");
            if p.exists() {
                return Ok(p.to_string_lossy().to_string());
            }
        }
    }

    Err("Could not find assumeIdentity-json.ps1. Set FNBA_UTILS_SCRIPT_DIR to the repo root.".to_string())
}
