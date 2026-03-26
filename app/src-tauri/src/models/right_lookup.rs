use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RightInfo {
    pub right_id: i32,
    pub right_name: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RightAssociate {
    pub assoc_id: i32,
    pub nickname: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
}
