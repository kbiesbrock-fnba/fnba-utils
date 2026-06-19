/// Shared WSL ↔ Windows path conversion utilities.
///
/// These functions are the single authoritative implementation; `claude_io.rs`
/// and `mission_control.rs` both delegate here rather than duplicating logic.

// ─── New helpers added for the soft-command path layer ────────────────────────

use std::sync::OnceLock;
use std::sync::Mutex;

/// Fetch the WSL `$HOME` directory via the persistent wsl helper shell.
/// Result is cached in a `OnceLock` — the first call pays the WSL round-trip;
/// subsequent calls return the cached value instantly. Returns `None` if the
/// helper fails (e.g. WSL not installed or not running).
pub fn wsl_home() -> Option<String> {
    static CACHE: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    let mu = CACHE.get_or_init(|| Mutex::new(None));
    let mut guard = mu.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        // NOTE: the script's output MUST end with a newline. `wsl_helper::run_script`
        // detects completion by matching a sentinel line, so output without a
        // trailing `\n` lands on the same line as the sentinel — the match never
        // fires and the read blocks forever. `printf '%s\n'`, not `printf '%s'`.
        if let Ok(out) = crate::state::wsl_helper::run_script("printf '%s\\n' \"$HOME\"") {
            let trimmed = out.trim().to_string();
            if !trimmed.is_empty() {
                *guard = Some(trimmed);
            }
        }
    }
    guard.clone()
}

// ─── Existing helpers (callers must not change) ───────────────────────────────

/// Translate a WSL path like `/mnt/c/dev/foo.ts` to `C:\dev\foo.ts`. Pure
/// Linux paths (e.g. `/home/<u>/...`) become UNC (`\\wsl.localhost\Ubuntu\...`).
/// Already-Windows paths pass through unchanged.
pub fn wsl_path_to_windows(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("/mnt/") {
        if let Some((drive, tail)) = rest.split_once('/') {
            if drive.len() == 1 && drive.chars().all(|c| c.is_ascii_alphabetic()) {
                return format!(
                    "{}:\\{}",
                    drive.to_uppercase(),
                    tail.replace('/', "\\"),
                );
            }
        } else if rest.len() == 1 {
            return format!("{}:\\", rest.to_uppercase());
        }
    }
    if path.starts_with('/') {
        return format!(r"\\wsl.localhost\Ubuntu{}", path.replace('/', "\\"));
    }
    // Looks like a Windows path already, or relative — pass through.
    path.to_string()
}

/// Translate a Windows path like `C:\dev\fnba-utils` to `/mnt/c/dev/fnba-utils`.
/// UNC paths under `\\wsl.localhost\Ubuntu\...` become `/...`. Already-Linux
/// paths pass through unchanged.
pub fn windows_path_to_wsl(path: &str) -> String {
    let s = path.replace('\\', "/");
    if let Some(rest) = s.strip_prefix("//wsl.localhost/") {
        // //wsl.localhost/<distro>/<rest> → /<rest>. If no rest, return root.
        return match rest.split_once('/') {
            Some((_distro, tail)) => format!("/{tail}"),
            None => "/".to_string(),
        };
    }
    // Detect "X:/..." drive prefix.
    if let Some((drive, rest)) = s.split_once(":/") {
        if drive.len() == 1 && drive.chars().all(|c| c.is_ascii_alphabetic()) {
            return format!("/mnt/{}/{rest}", drive.to_lowercase());
        }
    }
    s
}
