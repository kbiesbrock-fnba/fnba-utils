// Embed Windows VERSIONINFO so Task Manager labels the daemon "FNBA Clipd"
// instead of inheriting nothing (or, when bundled, the parent's resource).
// This package has no `tauri-build`, so we're the only resource emitter and
// CVTRES won't see a duplicate VERSION resource.

fn main() {
    #[cfg(windows)]
    embed_versioninfo();
}

#[cfg(windows)]
fn embed_versioninfo() {
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".to_string());
    let mut parts = version.split('.').map(|s| s.parse::<u16>().unwrap_or(0));
    let major = parts.next().unwrap_or(0);
    let minor = parts.next().unwrap_or(0);
    let patch = parts.next().unwrap_or(0);

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let rc_path = std::path::Path::new(&out_dir).join("clipd.rc");
    let rc = format!(
        r#"#include <winver.h>

VS_VERSION_INFO VERSIONINFO
FILEVERSION    {major},{minor},{patch},0
PRODUCTVERSION {major},{minor},{patch},0
FILEFLAGSMASK  0x3FL
FILEFLAGS      0x0L
FILEOS         VOS__WINDOWS32
FILETYPE       VFT_APP
FILESUBTYPE    VFT2_UNKNOWN
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904B0"
        BEGIN
            VALUE "CompanyName",      "FNBA\0"
            VALUE "FileDescription",  "FNBA Clipd\0"
            VALUE "FileVersion",      "{version}\0"
            VALUE "InternalName",     "fnba-clipd\0"
            VALUE "OriginalFilename", "fnba-clipd.exe\0"
            VALUE "ProductName",      "FNBA Clipd\0"
            VALUE "ProductVersion",   "{version}\0"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x0409, 0x04B0
    END
END
"#
    );
    std::fs::write(&rc_path, &rc).expect("write clipd.rc");
    embed_resource::compile(&rc_path, embed_resource::NONE);
}
