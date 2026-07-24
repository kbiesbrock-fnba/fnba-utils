use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// On-disk YAML config. Lives at `%LOCALAPPDATA%\fnba-utils\config.yaml`.
/// Missing file = empty config (all features off). This is the entire opt-in mechanism.
#[derive(Debug, Default, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub standup: Option<StandupConfig>,
    #[serde(default)]
    pub sql_library: Option<SqlLibraryConfig>,
}

/// Optional `sql_library:` section pointing the SQL Query panel at a folder of
/// `.sql` files. Read-only from the app (config.yaml is never written back), so
/// the user edits it by hand and hits Refresh in the panel to pick up changes.
#[derive(Debug, Default, Deserialize, Clone)]
pub struct SqlLibraryConfig {
    /// Root directory. Typically a WSL UNC path
    /// (`\\wsl$\Ubuntu\home\you\dev\sql`); posix/`~` forms are normalized to a
    /// Windows-reachable path by the sql_library commands.
    #[serde(default)]
    pub root: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct StandupConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub jira_email: Option<String>,
    #[serde(default)]
    pub jira_api_token: Option<String>,
    #[serde(default = "default_jira_domain")]
    pub jira_domain: String,
    #[serde(default)]
    pub teams_webhook_url: Option<String>,
    #[serde(default)]
    pub teams_channel_url: Option<String>,
    /// Display name of a Jira custom field to show as "Specification" in the
    /// expanded panel view. The Rust side looks this up by name via /rest/api/3/field
    /// and caches the resolved ID. Leave unset to disable.
    #[serde(default = "default_spec_field_name")]
    pub spec_field_name: Option<String>,
}

fn default_spec_field_name() -> Option<String> {
    Some("Specification Details".to_string())
}

fn default_jira_domain() -> String {
    "fnba.atlassian.net".to_string()
}

impl AppConfig {
    /// Read and parse the config file. Returns `Default::default()` on missing file or parse error
    /// (with a warning logged to stderr). Never panics — config errors must not block app startup.
    pub fn load() -> Self {
        let path = match Self::config_path() {
            Some(p) => p,
            None => return Self::default(),
        };
        let raw = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    eprintln!("config: could not read {}: {}", path.display(), e);
                }
                return Self::default();
            }
        };
        match serde_yaml::from_str::<AppConfig>(&raw) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("config: parse error in {}: {}", path.display(), e);
                Self::default()
            }
        }
    }

    pub fn config_path() -> Option<PathBuf> {
        Some(crate::state::paths::data_file("config.yaml"))
    }

    /// The configured SQL-library root, trimmed and non-empty, or `None`.
    pub fn sql_library_root(&self) -> Option<String> {
        self.sql_library
            .as_ref()
            .and_then(|s| s.root.clone())
            .map(|r| r.trim().to_string())
            .filter(|r| !r.is_empty())
    }

    /// True only when standup is explicitly enabled AND both credentials are present.
    pub fn standup_ready(&self) -> bool {
        match &self.standup {
            Some(s) => {
                s.enabled
                    && s.jira_email.as_deref().is_some_and(|v| !v.is_empty())
                    && s.jira_api_token.as_deref().is_some_and(|v| !v.is_empty())
            }
            None => false,
        }
    }
}

/// Redacted view sent to the frontend. Never includes the API token or webhook URL.
#[derive(Debug, Serialize)]
pub struct AppConfigView {
    pub standup: StandupConfigView,
}

#[derive(Debug, Serialize)]
pub struct StandupConfigView {
    pub enabled: bool,
    pub has_credentials: bool,
    pub jira_domain: String,
    pub teams_configured: bool,
    pub config_path: Option<String>,
}

impl AppConfigView {
    pub fn from_config(cfg: &AppConfig) -> Self {
        let s = cfg.standup.as_ref();
        Self {
            standup: StandupConfigView {
                enabled: cfg.standup_ready(),
                has_credentials: s.is_some_and(|c| {
                    c.jira_email.as_deref().is_some_and(|v| !v.is_empty())
                        && c.jira_api_token.as_deref().is_some_and(|v| !v.is_empty())
                }),
                jira_domain: s
                    .map(|c| c.jira_domain.clone())
                    .unwrap_or_else(default_jira_domain),
                teams_configured: s
                    .and_then(|c| c.teams_webhook_url.as_deref())
                    .is_some_and(|v| !v.is_empty()),
                config_path: AppConfig::config_path()
                    .map(|p| p.to_string_lossy().to_string()),
            },
        }
    }
}

#[tauri::command]
pub async fn get_app_config(
    state: tauri::State<'_, AppConfig>,
) -> Result<AppConfigView, String> {
    Ok(AppConfigView::from_config(&state))
}
