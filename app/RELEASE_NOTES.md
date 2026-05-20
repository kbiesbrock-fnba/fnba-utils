# Release Notes

## v1.3.2 — 2026-05-20

Bug-fix pass from code-review + simplify:

- **Worktree sessions: empty stats / dead reattach.** `OwnedSession.cwd` now stores the worktree path (the cwd claude actually runs in) so `get_session_detail` and the reattach path hash to the right project bucket and find the JSONL.
- **Worktree directory leaked on Kill.** `stop_claude_session` no longer pre-empts the drain thread's worktree cleanup; it captures the OwnedSession entry and runs `git worktree remove` itself.
- **Recents dropdown errored on blur.** The inline `setTimeout` in the launcher template was undefined in Vue's compiled context — moved to a `<script setup>` function.
- **Disconnect → reattach race.** Each `ClaudeIoSession` now carries a monotonic generation tag; the drain thread's EOF cleanup only removes the entry if the live generation still matches, preventing a stale drain from evicting a freshly-attached session.

Tightening:

- Dropped dead code (`_pid_alive`, orphan `SessionDetailActivity.vue`, dead params on `fetchDetail`, empty blur listener).
- `hashStr` extracted to `app/src/lib/hash.ts` and shared.
- `build_spawn_cmd` + `build_resume_cmd` unified.
- `tmux_session_alive` is now `pub(crate)`, reused by `get_session_detail`.
- Tmux liveness probe (`list_live_tmux_sessions`) cached with a 2-second TTL — Mission Control's 3 s poll no longer forks a process every tick.
- Patched a small `windows_path_to_wsl` fallthrough where a UNC path without a distro segment would incorrectly try the drive-letter branch.

## v1.3.1 — 2026-05-20

Bug-fix: `get_session_detail` was probing the captured (bash) PID for liveness, which dies the moment the panel closes. Now uses `tmux has-session`, matching the MC list. Restored sessions no longer render "Session has ended."

## v1.3.0 — 2026-05-20

**Terminal is always on; closing the panel disconnects (doesn't kill).**

The "Open Chat" / "Close" toggle is gone. The session-detail panel now always renders Header → Stats → Terminal → Actions for any alive session. Open a session, you're already in it.

Closing the window (X, or Win+Shift+C to hide the whole MC group) now **disconnects** our PTY but leaves the tmux session and claude inside it running — so reopening the panel attaches you right back where you were. The only way to end a session is the explicit Kill action in the panel actions row (or `/exit` from inside claude).

Under the hood: new `disconnect_session` Tauri command (drops PTY without killing tmux), and the PTY drain's EOF cleanup now probes `tmux has-session` to distinguish "claude actually died" from "we disconnected on purpose."

## v1.2.0 — 2026-05-20

**Chat panel is now a real terminal.**

The bubble-rendering UI is gone. The chat panel embeds an `xterm.js` terminal that mirrors the underlying tmux session byte-for-byte. Permission prompts, slash-command menus, the live cursor — everything claude shows is visible, and your keystrokes go straight to the PTY. Multi-line paste, special characters, arrow keys, Ctrl-C all behave exactly like a real terminal because they ARE going through one.

Side effect: this fixes the silent-send bug (the `tmux paste-buffer` path didn't reliably submit some content) and the invisible-prompt bug (claude waiting on a `UserAsk` was hidden because it wasn't in the JSONL). The Stop button now sends Ctrl-C via tmux for the same reason it always did.

Stats (tokens, cost, message count) still come from the JSONL tail and remain visible in the session-detail info view.

## v1.1.0 — 2026-05-19

**Mission Control: launch and manage your own Claude sessions.**

### New: Launch a Claude session from the palette

`Win+Shift+F` → "New Claude Session". Pick a working directory (with autocomplete of recent projects), optionally seed an initial prompt, and you're in chat — no terminal hop, no `cd ...`, no typing `claude`. Sessions launch inside a tmux session named `claude-<id>` so you can attach from anywhere else in WSL to follow along (see the new "tmux attach" button in the session header — one click copies the attach command).

### Mission Control is now exclusive to MC-launched sessions

Previously Mission Control scanned every WSL `.claude` dir and showed every running session it could find. That was noisy, and the "control existing sessions" goal it implied isn't deliverable until Anthropic ships an attach API. The new model: **MC tracks only what MC launched.** External claude processes (IntelliJ plugin, plain WSL terminals) no longer appear.

This also unblocks state we own — sessions persist across Tauri restart, can be re-labeled, and will eventually have history + resume.

### Chat panel upgrades

- **Stop button** while a turn is running (sends Ctrl-C without killing the process).
- **Up/Down arrow** in the input recalls prior prompts from the current session.
- **Editable session label** — click the title in the session-detail header to rename.
- **Token + estimated cost** display in stats (USD estimate at Sonnet rates; pricing table in `lib/pricing.ts`).
- **Trust-warning banner** when `~/.claude.json` couldn't be pre-accepted (no more silently-dropped first messages).
- **5-second startup watchdog** with a Retry button if Claude doesn't emit anything.
- **Worktree mode**: optional checkbox to launch into a fresh `git worktree add` under `.worktrees/<short>`. Cleaned up on close (best-effort; surfaces a warning if the worktree is dirty).

### Plumbing

- Backend `send_claude_message` now drives Claude via `tmux load-buffer | paste-buffer`, which delivers multi-line input atomically and is visible to every attached tmux client.
- Removed the parallel-resume + bracketed-paste pipeline (and ~430 lines of related dead code) since MC owns its sessions outright now.
- New `OwnedSessionsState` persists to `~/.claude/fnba-mc/owned-sessions.json` and dedupes via PID liveness on load.

### Coming soon

- One-keystroke launch into the last-used project (global hotkey).
- A project registry / picker (currently MRU-only).
- `@`-autocomplete and clickable file paths in the chat panel.
- Notifications when a session goes idle or is waiting on a permission prompt.
- Persistent session history with resume.

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
