use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RightInfo {
    pub right_id: i32,
    pub right_name: String,
}

/// A person in the FNBA directory. Shared by both the Right Lookup command
/// (holders of a right / rights of an associate) and Assume Identity (live
/// user search). The extra `login`/`job_title`/`department` fields let any
/// person row be assumed (we resolve their Windows login) and let the
/// Assume Identity "pin to favorites" prompt prefill a role label.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RightAssociate {
    pub assoc_id: i32,
    pub nickname: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    /// Bare Windows username to assume (the `associate_login.domain_username`
    /// with any leading `DOMAIN\` stripped). `None` when the associate has no
    /// login row.
    pub login: Option<String>,
    /// `associate.job_title` — the preferred prefill for the favorite label.
    pub job_title: Option<String>,
    /// `department.name` — fallback prefill when `job_title` is empty.
    pub department: Option<String>,
}
