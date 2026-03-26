use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct IdentityUser {
    pub username: String,
    pub labels: String,
}

#[derive(Serialize, Deserialize)]
pub struct IdentityData {
    pub imposter: String,
    pub users: Vec<IdentityUser>,
    pub connections: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct IdentityState {
    pub acting_as_login: String,
    pub acting_as_name: String,
    pub password: String,
    pub changed_at: String,
    pub on_host: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssumeIdentityResult {
    pub server: String,
    pub login: String,
    pub before: Option<IdentityState>,
    pub after: Option<IdentityState>,
    pub password_changed: bool,
    pub already_assuming: bool,
    pub message: Option<String>,
}
