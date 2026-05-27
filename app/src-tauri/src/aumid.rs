//! Register an AppUserModelID (AUMID) so Windows toast notifications fire from
//! the portable (un-installed) build.
//!
//! `tauri-plugin-notification` stamps every toast with the bundle identifier
//! (`com.fnba.utils`) as its `System.AppUserModel.ID` for any build *except*
//! one launched straight out of `…\target\debug` / `…\target\release` (see the
//! plugin's `desktop.rs`). For those raw dev exes it omits the id and
//! `notify-rust` falls back to the always-registered PowerShell AUMID, so
//! toasts appear. But our shipped artifact is a portable zip — `tauri build
//! --no-bundle` produces a bare exe with no MSI/NSIS installer, so no Start
//! Menu shortcut ever registers `com.fnba.utils` with the shell. Windows then
//! refuses to surface a toast for an unknown AUMID and `Toast::show()` fails
//! silently — which is why the PII-protection toast never appeared on a
//! portable install.
//!
//! Fix: on startup, register the AUMID the way an installer would — author a
//! Start Menu shortcut whose `System.AppUserModel.ID` property is
//! `com.fnba.utils`, and pin the running process to the same id via
//! `SetCurrentProcessExplicitAppUserModelID`. This is the standard
//! registration path for unpackaged Win32 apps. Idempotent: the shortcut is
//! authored only when missing, so steady-state startups just set the process
//! id and return.

#[cfg(windows)]
pub fn ensure_registered() {
    win::ensure_registered();
}

#[cfg(not(windows))]
pub fn ensure_registered() {}

/// Raise the "Clipboard protected" toast after a copy was detected as PII and
/// swapped for safe test data. `kinds` is a human-readable list (e.g.
/// "ssn, card"). Called from the daemon — the process that actually performs
/// the capture + substitution and is always running.
#[cfg(windows)]
pub fn show_pii_protected(kinds: &str) {
    win::show_pii_protected(kinds);
}

#[cfg(not(windows))]
pub fn show_pii_protected(_kinds: &str) {}

#[cfg(windows)]
mod win {
    use std::path::PathBuf;

    use windows::core::{Interface, GUID, HSTRING, PROPVARIANT};
    use windows::Win32::Foundation::BOOL;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::PropertiesSystem::{IPropertyStore, PROPERTYKEY};
    use windows::Win32::UI::Shell::{
        IShellLinkW, SetCurrentProcessExplicitAppUserModelID, ShellLink,
    };

    /// AUMID that must match `tauri.conf.json`'s `identifier` — the plugin
    /// uses that exact string as the toast's `System.AppUserModel.ID`.
    const APP_ID: &str = "com.fnba.utils";

    /// Display name for the Start Menu entry / toast attribution.
    const SHORTCUT_NAME: &str = "FNBA Utils";

    // PKEY_AppUserModel_ID — {9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3}, pid 5.
    // Defined inline rather than relying on a crate re-export so the exact key
    // is unambiguous.
    const PKEY_APPUSERMODEL_ID: PROPERTYKEY = PROPERTYKEY {
        fmtid: GUID::from_u128(0x9F4C2855_9F79_4B39_A8D0_E1D42DE1D5F3),
        pid: 5,
    };

    pub fn ensure_registered() {
        // Pin the process to our AUMID first — cheap, no COM, and the bit that
        // routes this process's toasts (the plugin stamps the same id on each
        // toast). Must happen before any window/notification work.
        unsafe {
            if let Err(e) = SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(APP_ID)) {
                eprintln!("aumid: SetCurrentProcessExplicitAppUserModelID failed: {e}");
            }
        }

        let Some(lnk) = shortcut_path() else {
            eprintln!("aumid: APPDATA not set; cannot author Start Menu shortcut");
            return;
        };
        if lnk.exists() {
            return; // already registered by a previous launch
        }
        let Ok(exe) = std::env::current_exe() else {
            eprintln!("aumid: current_exe() failed; skipping shortcut");
            return;
        };

        let lnk_s = lnk.to_string_lossy().into_owned();
        let exe_s = exe.to_string_lossy().into_owned();
        let workdir_s = exe.parent().map(|p| p.to_string_lossy().into_owned());

        // Author the shortcut on a dedicated STA thread so this code fully owns
        // its COM apartment lifecycle and never collides with COM state Tauri
        // may have set up on the main thread.
        let _ = std::thread::Builder::new()
            .name("fnba-aumid".into())
            .spawn(move || unsafe {
                let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                if let Err(e) = author_shortcut(&lnk_s, &exe_s, workdir_s.as_deref()) {
                    eprintln!("aumid: failed to author Start Menu shortcut: {e}");
                }
                if hr.is_ok() {
                    CoUninitialize();
                }
            });
    }

    fn shortcut_path() -> Option<PathBuf> {
        let appdata = std::env::var_os("APPDATA")?;
        let mut p = PathBuf::from(appdata);
        p.push(r"Microsoft\Windows\Start Menu\Programs");
        p.push(format!("{SHORTCUT_NAME}.lnk"));
        Some(p)
    }

    unsafe fn author_shortcut(
        lnk_path: &str,
        exe_path: &str,
        work_dir: Option<&str>,
    ) -> windows::core::Result<()> {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;

        let exe_h = HSTRING::from(exe_path);
        link.SetPath(&exe_h)?;
        // Use the exe's own embedded icon so the toast/Start Menu show our icon.
        link.SetIconLocation(&exe_h, 0)?;
        if let Some(wd) = work_dir {
            link.SetWorkingDirectory(&HSTRING::from(wd))?;
        }
        link.SetDescription(&HSTRING::from("FNBA Utils command palette"))?;

        // Stamp the AUMID onto the shortcut's property store — this is what
        // registers `com.fnba.utils` with the shell as a toast-capable app.
        // windows 0.58 has no `InitPropVariantFromString`, so build the string
        // PROPVARIANT directly. `From<&str>` yields a VT_BSTR, which the shell
        // link's property store accepts for System.AppUserModel.ID.
        let store: IPropertyStore = link.cast()?;
        let pv = PROPVARIANT::from(APP_ID);
        store.SetValue(&PKEY_APPUSERMODEL_ID, &pv)?;
        store.Commit()?;

        let persist: IPersistFile = link.cast()?;
        persist.Save(&HSTRING::from(lnk_path), BOOL(1))?;
        Ok(())
    }

    pub fn show_pii_protected(kinds: &str) {
        use tauri_winrt_notification::Toast;
        let body = format!(
            "Detected PII ({kinds}). Clipboard replaced with safe test data. \
             Win+V then Ctrl+Shift+Enter for the original."
        );
        if let Err(e) = Toast::new(APP_ID)
            .title("Clipboard protected")
            .text1(&body)
            .show()
        {
            eprintln!("aumid: PII toast failed to show: {e}");
        }
    }
}
