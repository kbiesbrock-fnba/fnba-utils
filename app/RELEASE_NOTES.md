# Release Notes

## v1.0.0 — 2026-05-19

The first formal release of FNBA Utils.

FNBA Utils is a Raycast-style command palette for FNBA developers. It exists to compress the small, recurring annoyances of daily FNBA dev — `assumeIdentity` stored-proc calls, rights and permissions lookups, juggling parallel Claude Code sessions — into a single hotkey.

Versioning starts here because the app is now being used by more than its author. The intent for the 1.x line is unapologetically **developer quality of life**: no installer, no admin rights, no passwords, no config sprawl. Defaults work out of the box; per-user extras merge in from `~/.assumeIdentity.json`.

### Keyboard shortcuts

- **`Win+Shift+F`** → [Assume Identity](#assume-identity)
- **`Win+Shift+F`** → [Right Lookup](#right-lookup)
- **`Win+Shift+C`** → [Mission Control](#mission-control) _(experimental)_

### Assume Identity

Pick a user and SQL Server, see the before/after of `logincheck.fnba.assumeIdentity`, and optionally save new users or connections back to your config.

### Right Lookup

Search rights and associates across any FNBA SQL server. Recently-used rights pin to the top.

### Mission Control

**Experimental — work in progress.** Floating monitor of active Claude Code sessions, with an embedded chat panel for `claude --resume` and a SQL query side-panel. The window itself is labelled _(experimental)_ next to its title as a reminder. The feature is functional but actively evolving; expect behavior, layout, and the resume-chat plumbing to change between builds. If something breaks you can dismiss the window with the same shortcut and the rest of the app is unaffected.

### Distribution

- Single portable `.exe`. Drop it anywhere; no admin install, no Webview2 hassles (it ships with Windows).
- Integrated auth (SSPI) — your Windows domain login is what the SQL connections use. No passwords stored, no passwords prompted.
- The build's exact version and short commit identifier are visible in the tray **About** dialog and the palette status bar, so "which build do you have?" is always answerable.
