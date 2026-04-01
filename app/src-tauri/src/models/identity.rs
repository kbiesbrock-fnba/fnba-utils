use serde::{Deserialize, Serialize};

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Serialize, Deserialize, Clone)]
pub struct IdentityUser {
    pub username: String,
    pub label: String,
    #[serde(default, rename = "isCustom", skip_serializing_if = "is_false")]
    pub is_custom: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct IdentityConnection {
    pub label: String,
    pub server: String,
    #[serde(default, rename = "isCustom", skip_serializing_if = "is_false")]
    pub is_custom: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct IdentityImposter {
    pub name: String,
    #[serde(default, rename = "isCustom", skip_serializing_if = "is_false")]
    pub is_custom: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityData {
    pub current_user: String,
    pub imposters: Vec<IdentityImposter>,
    pub users: Vec<IdentityUser>,
    pub connections: Vec<IdentityConnection>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
