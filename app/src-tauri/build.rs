use std::env;
use std::process::Command;

// Mirrors scripts/version.mjs: <base>-<commit-count>-g<short-sha>[+dirty]
// `base` comes from CARGO_PKG_VERSION (Cargo.toml). git info comes from the
// workspace working tree. Both sides must produce the same string so the
// tray About dialog and the palette display agree.
fn compute_version() -> String {
    let base = env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".to_string());

    let count = Command::new("git")
        .args(["rev-list", "--count", "HEAD"])
        .output()
        .ok()
        .and_then(|o| if o.status.success() { String::from_utf8(o.stdout).ok() } else { None })
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "0".to_string());

    let sha = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| if o.status.success() { String::from_utf8(o.stdout).ok() } else { None })
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let dirty = match Command::new("git")
        .args(["diff-index", "--quiet", "HEAD", "--"])
        .status()
    {
        Ok(s) if s.success() => "",
        _ => "+dirty",
    };

    format!("{}-{}-g{}{}", base, count, sha, dirty)
}

fn main() {
    let version = compute_version();
    println!("cargo:rustc-env=APP_VERSION={}", version);
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/index");
    tauri_build::build()
}
