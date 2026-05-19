# FNBA Utils

A Raycast/Spotlight-style command palette for FNBA developers. Two global hotkeys, no installer, no admin rights, no passwords prompted.

For what's new per version, see `RELEASE_NOTES.md` next to this file.

## Requirements

- Windows 10 or 11 (Webview2 is built in).
- FNBA network access on TCP 1433 to the SQL Server hosts (VPN if remote).
- A Windows domain login — the app uses SSPI / integrated auth, so no passwords are stored or asked for.

## How to run

1. Unzip the portable folder anywhere (Desktop, Documents, `%LOCALAPPDATA%`, etc.). No installer, no admin rights needed.
2. Double-click `fnba-utils.exe`. A window opens; you can close it — the app keeps running in the background.
3. Global hotkeys (registered automatically while the app is running):
   - **`Win+Shift+F`** — open the command palette (Assume Identity, Right Lookup).
   - **`Win+Shift+C`** — open Mission Control _(experimental)_.
4. To launch on every login, drop a shortcut to `fnba-utils.exe` into:

   ```
   shell:startup
   ```

   (Paste that into the Run dialog — `Win+R` — to open the startup folder.)

## What's built in

The three FNBA SQL servers and the default staff list are baked into the exe, so the app works out of the box:

- **Local** — `dsqlaleroy.fnba-dev.network`
- **Development** — `meleagris.fnba.com`
- **Staging** — `caster.fnba.com`

Plus ~36 default users (developers / QA / etc.) selectable from the picker. Your own Windows username is always available as an "imposter" (the identity you authenticate as before assuming someone else's).

## Custom users / connections / imposters

Create this file to add extras beyond the defaults:

```
%USERPROFILE%\.assumeIdentity.json
```

That resolves to e.g. `C:\Users\jsmith\.assumeIdentity.json`. In Notepad, **File > Save As** with quotes around the filename so Windows doesn't append `.txt`.

All three sections are optional:

```json
{
  "Imposters": ["username1", "username2"],
  "Users": [
    { "label": "QA Lead", "username": "qauser1" },
    { "label": "DBA",     "username": "dba1" }
  ],
  "Connections": [
    { "label": "My Sandbox", "server": "sandbox.fnba.com" },
    "another-server.fnba.com"
  ]
}
```

A ready-to-edit template ships next to this README: `example.assumeIdentity.json`.

**Behavior:**

- Entries merge with the built-in defaults (they don't replace them).
- Duplicates are skipped case-insensitively (by username / server).
- Custom entries are flagged in the UI and can be deleted from the picker — that writes back to your `.assumeIdentity.json`.
- Changes are picked up on next launch of the app.

## Mission Control _(experimental)_

`Win+Shift+C` opens a separate window that monitors active Claude Code sessions under `~/.claude/projects/` (both Windows and WSL homes). The chat panel spawns `claude --resume` via WSL, so this feature only works if you already have Claude Code installed inside WSL.

This feature is a work in progress; behavior and layout may change between builds. If you don't use Claude Code, just ignore this window — the rest of the app works fully on its own.

## Troubleshooting

**Hotkey doesn't fire.** Another app is holding `Win+Shift+F`. Close conflicts or open the visible window manually from the taskbar.

**"Connection failed" in Assume Identity.** Confirm VPN is up and you can reach the target SQL host on port 1433. From PowerShell:

```powershell
Test-NetConnection meleagris.fnba.com -Port 1433
```

**Anti-virus quarantine.** The exe is unsigned. If endpoint security removes it, restore from quarantine or ask IT to whitelist `com.fnba.utils`.

**App doesn't start / immediate crash.** Run the exe from PowerShell so you can read any error output before the window exits.

## Version

The exact build identifier (`<base>+<commit-count>`) is visible at:

- The tray icon tooltip (hover the FNBA Utils icon).
- The tray context menu's **About** entry.
- The palette status bar (bottom-left of the Win+Shift+F window).

Identifier: `com.fnba.utils`
