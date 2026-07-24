/// Run a shell command in the user's default Windows Terminal profile.
///
/// Spawns `wsl.exe -e bash -ilc "<command>; exec bash"` with
/// `CREATE_NEW_CONSOLE` so Windows 11 routes the process to a new Windows
/// Terminal window (Ubuntu by default). The child is detached immediately —
/// we don't wait for it or track its exit.
///
/// We do NOT construct a `wt.exe` command line: wt.exe re-parses its own
/// argument string and mangles quotes in the user's command.  Routing through
/// `wsl.exe` avoids that entirely.
#[tauri::command]
pub fn run_in_terminal(command: String) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

        std::process::Command::new("wsl.exe")
            .args(["-e", "bash", "-ilc", &format!("{command}; exec bash")])
            .creation_flags(CREATE_NEW_CONSOLE)
            .spawn()
            .map_err(|e| format!("Failed to launch terminal: {e}"))?;
        Ok(())
    }

    #[cfg(not(windows))]
    {
        let _ = command;
        Err("run_in_terminal is only supported on Windows".to_string())
    }
}
