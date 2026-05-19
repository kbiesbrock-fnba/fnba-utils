use std::process::Command;

fn main() {
    let count = Command::new("git")
        .args(["rev-list", "--count", "HEAD"])
        .output()
        .ok()
        .and_then(|o| if o.status.success() { String::from_utf8(o.stdout).ok() } else { None })
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "0".to_string());

    println!("cargo:rustc-env=APP_VERSION={}+{}", env!("CARGO_PKG_VERSION"), count);
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    tauri_build::build()
}
