# Release Notes

<!--
  Accumulate notes for the next release under [Unreleased]. At release time,
  rename that heading to `## vX.Y.Z — YYYY-MM-DD` and add a fresh empty
  [Unreleased] above it. CI publishes every version section added since the
  previous tag as the GitHub Release body; the [Unreleased] section is skipped.
-->

## [Unreleased]

- **PII-protection toasts now actually appear.** When a copy is detected as sensitive and the clipboard is swapped for safe test data, Windows now pops a "Clipboard protected" notification telling you what was found and how to recover the original (`Win+V`, then `Ctrl+Shift+Enter`). These were already wired up but never showed on the portable build because Windows won't surface a toast for an app it doesn't recognize; the app now registers itself with Windows on startup so the notifications come through. The toast is now raised by the clipboard daemon (which does the actual protection), so it fires even when the main FNBA Utils window is closed.
- **The clipboard manager reopens at the top of the list.** Opening it again no longer leaves you scrolled where you were last time — it lands on the freshest entry, ready to paste.
- **Clipboard timestamps show the actual copy time once they're over an hour old.** Recent entries still read `just now` / `12m`; anything older shows the wall-clock time it was copied (e.g. `2:34 PM`, or `May 26 2:34 PM` for earlier days) instead of a vague `3h` / `2d`. Re-copying the same text continues to move that one entry to the top with a refreshed time rather than adding a duplicate.
- **Fixed PII detection rewriting the clipboard in a loop.** Because the safe test data swapped in can itself look like PII (a test SSN is still formatted like an SSN), the protected value was being re-scanned and re-substituted over and over — thrashing the clipboard and firing a stream of notifications. The app's own clipboard writes — both the automatic protection and pasting an entry from the clipboard manager — are now reliably recognized and skipped on re-capture, so a detected copy is protected exactly once and pasting the original from history is no longer re-obfuscated.
- **The standup Teams post is now a compact markdown summary.** It used to render as a tall multi-column card; it now posts a slim message — a header line with the date and totals, then each status group as a bold heading with its issues as bulleted `KEY` links and story points. Same content, far less vertical space in the channel.

## v1.12.6 — 2026-05-26

Follow-ups to the v1.12.5 storage + daemon work:

- **`Win+V` / `Win+Shift+V` reliably opens the clipboard window.** Opening the Pinned view (`Win+Shift+V`) with no pinned entries could leave the window in a state where the next `Win+V` appeared to do nothing until you triggered another window. The window now always comes to the front.
- **The clipboard daemon shows as "FNBA Clipd" in Task Manager**, as a distinct standalone process rather than being grouped under "FNBA Utils". A freshly built/installed daemon now also reliably replaces an older one still running from a previous launch.
- **About FNBA Utils now lists the clipboard daemon version** (tray → About). The daemon carries its own version, independent of the app.

## v1.12.5 — 2026-05-26

All persistent state files now live under `%LOCALAPPDATA%\fnba-utils\` (config, standup DB, identity overrides, MC sessions, MC projects, clipboard DB). On first launch the app migrates files from their old locations (`~/.fnba-utils/`, `~/.assumeIdentity.json`, `~/.claude/fnba-mc/`, the exe's `resources/` sibling, `%APPDATA%\fnba-utils\`) into the new directory — no manual data move required. Claude Code's own `~/.claude/` directory is left untouched.

A new ⚙ settings button in the command palette's top-right opens that folder in Explorer, so finding `config.yaml` or `assumeIdentity.json` for hand-editing is a single click.

The clipboard daemon (`fnba-clipd.exe`) now embeds its own Windows version info, so Task Manager labels it **FNBA Clipd** instead of inheriting **FNBA Utils** from the main process.

Two corporate-machine regressions fixed: `Win+V` and `Win+Shift+V` work again on machines where a DLP/EDR agent owns `Win+Shift+F/N/C/D` (the LL keyboard hook is now installed before the failure-prone `RegisterHotKey` calls, and each registration is best-effort instead of aborting the rest of setup), and SQL connections to FNBA servers no longer fail with "self-signed certificate" in release builds — `trust_cert()` is unconditional again (matches the corporate cert reality).

## v1.12.4 — 2026-05-26

Per-window code splitting + vendor chunking: each Tauri window (Mission Control, Standup, Clipboard Manager, SQL Query, Session Detail, Issue Detail, command palette) now loads only its own Vue component instead of the full 552 kB bundle every window inherited before. `@xterm/*`, `@tauri-apps/*`, and Vue itself sit in named long-lived vendor chunks so they're cached across windows. First-open latency drops correspondingly.

## v1.12 — 2026-05-22

### Clipboard Manager: PII protection

The clipboard daemon scans every captured text/HTML entry for SSNs, Luhn-validated cards, ABA-validated bank account + routing numbers, emails, phones, and DOBs. Sensitive matches are flagged and the daemon picks one Test User from a curated pool to back a substituted projection alongside the original. Account numbers, DOBs, and unpunctuated SSNs require a nearby keyword ("account", "DOB", "SSN", "routing") to fire, so digit runs in source code don't false-positive.

- **Default paste is safe**: pressing `Enter` on a sensitive clipboard entry pastes the substituted ("test user") version. Muscle-memory `Win+V` → `Enter` can't leak a real SSN into Teams / Jira / a browser by accident.
- **Auto-protect the OS clipboard**: as soon as the daemon detects PII in a fresh copy, it rewrites the OS clipboard with the substituted text — so a direct `Ctrl+V` (without ever opening Win+V) also pastes the safe version. A Windows toast lists the detected categories each time.
- **Paste original is deliberate**: `Ctrl+Shift+Enter` (or right-click → "Paste original") pastes the captured original, gated by a single-use reveal token. `Ctrl+Alt+Enter` does the same as copy-only.
- **Sticky Test User per record**: multi-PII entries (SSN + DOB + email of one fake person) use the same Test User's fields across all matches so the substituted text stays coherent.
- **10 Test Users seeded on first run**: identities with fake-but-format-valid SSNs (900-block, IRS-reserved), Luhn-valid issuer test cards, ABA-valid routing, 555-010 fictional phones, and `@test.fnba.local` emails. Editable from the settings cog (⚙ → Test Users…).
- **Test Users panel** lets you add / edit / disable / delete identities. Each user carries an SSN, DOB, email, phone, address, account #, routing #, and any number of cards (number / expiry / CVV).
- **Mask-style fallback** when the Test User pool is empty or a field is blank: keep-last-4 obfuscation (`***-**-6789`, `**** **** **** 1111`, `k****@fnba.com`). Still flagged sensitive, still reveal-gated.
- **List view never leaks**: previews for sensitive rows show the obfuscated text. The original only crosses the IPC bridge with an explicit reveal-token + `pasteOriginal=true`.
- **Right-click context menu** on any clipboard row: Paste / Paste original / Copy / Copy original / Pin / Delete. Sensitive entries list the detected PII categories as chips in the detail pane.
- **`Win+Shift+V` jumps to Pinned**: same window, positioning, prior-foreground capture, and show+focus semantics as `Win+V` — only the initial filter differs. Both chords ride a single `WH_KEYBOARD_LL` hook that runs before hotkey dispatch and swallows the keystroke with `LRESULT(1)`, the only approach that survives corporate DLP / EDR agents which claim `Win+Shift+V` via `RegisterHotKey`.

## v1.11 — 2026-05-22

### Clipboard Manager: Win+V

New palette command **Clipboard** plus a dedicated `Win+V` global shortcut opens a Raycast-style clipboard history window. Replaces the native Windows Clipboard (Win+V) with: unlimited SQLite-backed history, fuzzy search, source-app attribution, image thumbnails, pinned entries, and sensitivity-aware masking.

- **Captures text, HTML, and images** via a hidden Win32 `AddClipboardFormatListener` window — event-driven, no polling. Image entries are stored as PNG with a 256 px thumbnail; HTML entries keep both the fragment and a plain-text fallback for search.
- **Sensitivity-aware**: entries flagged by source apps (1Password, KeePass, Bitwarden — anything that sets `ExcludeClipboardContentFromMonitoring`, `CanIncludeInClipboardHistory`, or `CanUploadToCloudClipboard`) are stored but rendered as `••••••` and require an explicit reveal-token round-trip before they can be pasted.
- **Paste back into the prior app**: pressing Enter restores the foreground window that was active before the launcher and synthesizes Ctrl+V via `SendInput`. Ctrl+Enter sets the clipboard without auto-pasting.
- **Replaces the native Windows clipboard history**: hooks `Win+V` via `WH_KEYBOARD_LL` and swallows it before the shell sees it, so the fnba-utils window opens instead of the OS popup. Hotkey is always *show + focus + select-all-in-search*, never toggle — pressing it while open re-focuses the search.
- **Capture runs as a separate daemon (`fnba-clipd.exe`)**: a tiny background process owns the clipboard listener + SQLite writes and is registered under `HKCU\…\Run` on first fnba-utils launch, so history keeps accruing even when fnba-utils itself is closed. fnba-utils owns the search/display UI and reads from the shared DB at `%LOCALAPPDATA%\fnba-utils\clipboard.db`. The daemon is singleton-protected (TCP port 53217 bind) so duplicate launches are no-ops.
- **Fuzzy search** in the clipboard list — skim/fzf-style ranking (`fuzzy-matcher` crate), so subsequence and typo matches surface naturally. Pinned entries float to the top; pool capped at the 5 000 most-recent rows so ranking stays fast at any history size.
- **Dedupe by content hash** — repeating a copy bumps the existing row's timestamp instead of creating duplicates. Pinned entries always sort first and are never auto-pruned.
- **Keyboard**: `↑/↓` navigate, `PageUp/Down` jump, `Enter` paste, `Ctrl+Enter` copy only, `P` pin, `Del` remove, `/` focus search, `Esc` hide.

## v1.10 — 2026-05-21

### Mission Control: every tmux session, one panel

The session list covers **every tmux session on the host** — including IntelliJ terminals that auto-attach via `tmux new -A -s "$(basename "$PWD")"` — so a single panel covers "what am I running."

- **Source badges.** Each row is tagged `MC` (spawned by Mission Control), `claude` (claude is running inside an external tmux session — detected by `pane_current_command`, with a `ps` follow-up for `node`-wrapped invocations), or `tmux` (plain shells, vim, etc.).
- **Current program column.** The compact row shows the foreground command of the active pane (e.g. `vim`, `bash`, `claude`). Expanded view adds `attached`, `windows`, and `current path`.
- **Source filter chips.** `All / MC / claude / tmux` chips above the list, single-select with per-chip counts. Persisted in localStorage (`fnba-utils:mc-source-filter`).
- **Attach to external sessions.** Clicking any tmux row opens the detail panel and the xterm pane runs `tmux attach -t <name>` — read/write, just like an MC session. Closing the panel disconnects (does not kill); the originating IntelliJ terminal keeps the session alive.
- **No-kill safety.** External sessions can be observed and attached but never killed from MC. The `Kill` action only tears down sessions MC created.
- **Manual refresh** on the Tmux Sessions section header and the Connections panel. Click the icon: it spins for at least 400 ms (so very fast refreshes are still visible) and then briefly flashes a green check on success or a red X on failure for ~1.5 s before reverting. Implemented as a reusable `RefreshButton` that takes a single `:on-refresh` async handler.
- **Hide errored connections by default.** The Connections panel hides any servers whose probe returned an error, so a healthy list isn't crowded out by red rows when the VPN is off or a server is down. A small eye icon appears in the panel header whenever errored connections exist — click to toggle them back on. The choice persists across restarts; the panel's total count still reflects every configured connection.

### Backend

- New `state::tmux_sessions` module batches the probe into two `wsl.exe -e tmux` calls (`list-sessions` + `list-panes -a`) with a 2 s TTL cache.
- New `state::wsl_helper` module owns a persistent `wsl.exe bash` subprocess with auto-respawn on broken pipe / WSL shutdown. All probes run as one batched script (`tmux list-sessions` + `tmux list-panes -a` + a single `ps` over every pane pid), so refresh latency is under 200 ms even cold.
- `OwnedSessions` shares the `tmux_sessions` cache, so MC's two callers pay for one probe per refresh. Background polling benefits too.
- New Tauri command `attach_tmux_session(name, cwd)` reuses the existing PTY plumbing under a synthetic session id `tmux:<name>` so `write_session_pty` / `resize_session_pty` / `disconnect_session` route without modification.

## v1.9 — 2026-05-21

### Standup feature: Jira fetch, Teams post, always-on-top panel

Opt-in via `~/.fnba-utils/config.yaml` (no file or `standup.enabled: false` keeps the feature invisible). New palette command **Standup** pulls Jira issues and posts a redesigned Adaptive Card to Teams; **Win+Shift+D** opens a persistent always-on-top panel with the live work list. Double-clicking a row opens a full-task window that lazy-fetches description, specification, and Smart Checklist.

The Standup palette command is preview-first: opening it auto-fetches your Jira list and renders the report immediately. From the preview you can either **Refresh** the data or **Post to Teams**. After a successful post, the configured `teams_channel_url` opens so Teams pops to the channel.

### Panel

- Wide-grid layout (1200×960 default, resizable) on CSS subgrid: `[checkbox] [KEY+badge] [type pill] [checklist-icon] [summary…] [priority+due] [drag-handle]`. Every row 32 px; drag handle pinned to the rightmost column.
- **Story points badge** with per-value coloring — alternating deep / bright tones so adjacent point values are unmistakable.
- **Bugs section** at top; everything else below in a single ordered list (status → priority → due date), reorderable by drag.
- **Checkboxes mark items done** (line-through, dimmed); "show completed" toggle in header surfaces them back.
- **Smart Checklist sub-rows** behind a "Toggle Smart Checklist" row with a done/total count pill. Headers, markdown task syntax, and Railsware/Titanium legacy bullets all parsed. Reads from `cf[13097]`.

### Full-task window

- Issue metadata (status, priority, due, assignee, reporter, labels, dates).
- **Description** + **Specification** sections collapsible behind real `<button>` toggles with rotating chevrons; tabbable with visible focus rings, Enter/Space to expand.
- Smart Checklist rendered with progress pill (`done/total`).
- Specification field resolves by display name (`spec_field_name`, default `"Specification Details"`) via `/rest/api/3/field`, cached per session.

### Storage

All state is local to the install: SQLite at `<exe-dir>/resources/standup.db` captures every run, snapshot, hidden-state, manual-order, and checklist text. Migrations are best-effort `ALTER TABLE`s so existing DBs upgrade in place.

### Config

```yaml
standup:
  enabled: true
  jira_email: kevin.biesbrock@fnba.com
  jira_api_token: "..."
  jira_domain: fnba.atlassian.net
  teams_webhook_url: "..."
  teams_channel_url: "msteams:/l/channel/..."
  spec_field_name: "Specification Details"
```

## v1.8 — 2026-05-20

**Persistent session history + Resume.**

- Dead sessions archive to a `history` list in `~/.claude/fnba-mc/owned-sessions.json` (capped at 200, newest first). Detected via the existing tmux-liveness sweep.
- Mission Control has a collapsible **History** section below the live sessions. Each row shows label / session-id / cwd / "ended N ago" plus two actions:
  - **Resume** — re-spawns `claude --resume <id>` in the original cwd, registers a new live entry (same `session_id`, fresh pid + tmux), and opens its session-detail panel.
  - **Forget** — drops the entry from history permanently.
- New Tauri commands: `list_session_history`, `forget_session_history`, `resume_owned_session`.
- New composable `useSessionHistory` + reuses the existing `build_tmux_claude_cmd(cwd, sid, "--resume")` flag path.

## v1.7 — 2026-05-20

**Click any file path in the terminal to open it in IntelliJ.**

The terminal recognizes file-path tokens claude prints (`/mnt/c/...`, `~/...`, `./relative`, `C:\...`, with optional `:LINE:COL` suffix) and underlines them on hover. Click → opens in IntelliJ if `idea64.exe` is on PATH, otherwise hands off to `explorer.exe` for the default app. WSL paths are translated to Windows form before launching.

## v1.6 — 2026-05-20

**Roomier defaults + hoverable resize handles.**

- Session-detail panels open at 880×760 — a comfortable terminal-reading size. Both the palette launcher and Mission Control's "New Claude Session" command pull this from a single `PANEL_DEFAULTS` source so the two paths can't drift.
- Mission Control opens at 480 wide and is resizable; minimum 320×400.
- `ResizeHandles` overlay component renders 8 invisible drag zones around each frameless window (4 edges + 4 corners). Edge grab zones are 10 px; corners 20×20. The OS cursor changing to a resize arrow is the only affordance — no visual overlay. Imports are static so `startResizeDragging` lands inside the original mousedown event tick (Tauri requires the hand-off in the same task). Handles bind using `@tauri-apps/api/window` v2's `ResizeDirection` as string literals (the type is a type-only string union, not a runtime enum).

### Session-detail polish

- Newly-launched sessions render the live terminal immediately after spawn. `get_session_detail` treats live `ClaudeIoState` PTY ownership as proof-of-life first; tmux probe is the fallback path for sessions restored after a Tauri restart (where io_state is empty).
- The detail-window precondition only requires `sessionId`; `portable_pty::Child::process_id` returns `None` on Windows often enough that a pid-based check would be unreliable.

### Internals

- `PANEL_DEFAULTS` and the panel-window helpers (`panelLabelFor`, `panelUrlFor`, `payloadOf`, `panelKeyFor`) live in `lib/panels.ts`; both `useMissionControl` and `NewSessionCommand` import from there.
- xterm.js + PTY lifecycle live in a `useTerminal` composable. `ChatPane.vue` is ~100 lines of template + styles; terminal construction, resize observer, claude-event subscription, startup watchdog, and disconnect-on-unmount live in `app/src/composables/useTerminal.ts`.

## v1.5 — 2026-05-20

**Notifications + cycling focus across session-detail panels.**

Two system toasts (Windows notification surface):

- **Permission-prompt detected.** Backend scans the live PTY output for known prompt patterns ("Do you want to allow...", "❯ 1. Yes", etc.) and fires a "Claude is waiting" toast when one appears AND no MC window is focused. Especially valuable for plan-mode + acceptEdits work where claude silently sits at a decision.
- **Busy → Idle.** Mission Control's polling tracks per-session status; on a Busy→Idle transition with no MC window focused, fires "Claude finished: <label>". Catches the case of "I started a long task and walked away."

Both notifications are suppressed when any MC window has focus — the assumption is you're already watching. Pattern list lives in `app/src-tauri/src/commands/claude_io.rs::PERMISSION_PROMPT_PATTERNS`; revisit when claude's wording changes.

**`Ctrl+Shift+Tab` cycles focus through open session-detail panels.** Stable order (by panel label hash). For juggling 3+ live sessions without reaching for the mouse.

### Other

- Mission Control's blur-hide respects per-panel pinning. Click the ⭐ on a session-detail panel and it stays visible when MC itself hides on blur. Win+Shift+C still hides everything regardless (explicit dismiss gesture); reopening MC restores pinned panels.
- The permission-prompt scanner's sliding window in `claude_io.rs` snaps `String::drain` to the next char boundary before draining, so multi-byte glyphs in claude's TUI output (e.g. "❯" is 3 bytes) can't trigger a panic.
- The `@tauri-apps/plugin-notification` npm dependency is paired with the Rust crate (`useNotifications.ts` imports it on the JS side).
- `get_session_detail` returns `SessionStatus::Dead` when `tmux has-session` reports the session is gone.

## v1.4 — 2026-05-20

**Project registry + zero-keystroke launch.**

### Win+Shift+N — launch into your most-recent project

The global shortcut spawns a Claude session in whatever project you last launched, with no prompt and no clicks. Mission Control surfaces showing the new session-detail panel attached to it.

### Pinned + MRU project list

The launcher's autocomplete is backed by a persistent registry (`~/.claude/fnba-mc/projects.json`). Every launch records the project; pinned ones stick at the top of the list. Click the star next to any recent entry to pin/unpin. Pinned entries sort alphabetically; unpinned by recency.

### Plumbing

- New `state/projects.rs` (Project struct + ProjectsState) and `commands/projects.rs` (list / add / update / remove / record_project_used).
- `start_new_claude_session` records every successful spawn server-side, so the registry stays accurate across Tauri restarts.
- `useProjects` composable on the frontend; `useNewClaudeSession` backs its MRU with the registry.

## v1.3 — 2026-05-20

**Terminal is always on; closing the panel disconnects (doesn't kill).**

The session-detail panel always renders Header → Stats → Terminal → Actions for any alive session. Open a session, you're already in it.

Closing the window (X, or Win+Shift+C to hide the whole MC group) **disconnects** the PTY but leaves the tmux session and claude inside it running — so reopening the panel attaches you right back where you were. The only way to end a session is the explicit Kill action in the panel actions row (or `/exit` from inside claude).

Under the hood: `disconnect_session` Tauri command drops the PTY without killing tmux, and the PTY drain's EOF cleanup probes `tmux has-session` to distinguish "claude actually died" from "we disconnected on purpose." `get_session_detail` uses `tmux has-session` for liveness, matching the MC list.

### Worktree hardening

- `OwnedSession.cwd` stores the worktree path (the cwd claude actually runs in) so `get_session_detail` and the reattach path hash to the right project bucket and find the JSONL.
- `stop_claude_session` captures the OwnedSession entry and runs `git worktree remove` itself, so the drain thread doesn't race with the cleanup.

### Race fixes & tightening

- Each `ClaudeIoSession` carries a monotonic generation tag; the drain thread's EOF cleanup only removes the entry if the live generation still matches, preventing a stale drain from evicting a freshly-attached session.
- Recents dropdown blur handler lives in `<script setup>` (Vue's compiled template context doesn't expose `setTimeout` directly).
- `hashStr` lives in `app/src/lib/hash.ts` and is shared.
- `build_spawn_cmd` + `build_resume_cmd` unified.
- `tmux_session_alive` is `pub(crate)`, reused by `get_session_detail`.
- Tmux liveness probe (`list_live_tmux_sessions`) is cached with a 2-second TTL, so Mission Control's 3 s poll doesn't fork a process every tick.
- `windows_path_to_wsl` handles a UNC path without a distro segment correctly.

## v1.2 — 2026-05-20

**Chat panel is a real terminal.**

The chat panel embeds an `xterm.js` terminal that mirrors the underlying tmux session byte-for-byte. Permission prompts, slash-command menus, the live cursor — everything claude shows is visible, and your keystrokes go straight to the PTY. Multi-line paste, special characters, arrow keys, Ctrl-C all behave exactly like a real terminal because they ARE going through one.

Stats (tokens, cost, message count) come from the JSONL tail and remain visible in the session-detail info view. The Stop button sends Ctrl-C via tmux.

## v1.1 — 2026-05-19

**Mission Control: launch and manage your own Claude sessions.**

### New: Launch a Claude session from the palette

`Win+Shift+F` → "New Claude Session". Pick a working directory (with autocomplete of recent projects), optionally seed an initial prompt, and you're in chat — no terminal hop, no `cd ...`, no typing `claude`. Sessions launch inside a tmux session named `claude-<id>` so you can attach from anywhere else in WSL to follow along (see the "tmux attach" button in the session header — one click copies the attach command).

### Mission Control is exclusive to MC-launched sessions

**MC tracks only what MC launched.** External claude processes (IntelliJ plugin, plain WSL terminals) don't appear. Sessions persist across Tauri restart, can be re-labeled, and have history + resume support.

### Chat panel

- **Stop button** while a turn is running (sends Ctrl-C without killing the process).
- **Up/Down arrow** in the input recalls prior prompts from the current session.
- **Editable session label** — click the title in the session-detail header to rename.
- **Token + estimated cost** display in stats (USD estimate at Sonnet rates; pricing table in `lib/pricing.ts`).
- **Trust-warning banner** when `~/.claude.json` can't be pre-accepted, so first messages aren't silently dropped.
- **5-second startup watchdog** with a Retry button if Claude doesn't emit anything.
- **Worktree mode**: optional checkbox to launch into a fresh `git worktree add` under `.worktrees/<short>`. Cleaned up on close (best-effort; surfaces a warning if the worktree is dirty).

### Plumbing

- Backend `send_claude_message` drives Claude via `tmux load-buffer | paste-buffer`, which delivers multi-line input atomically and is visible to every attached tmux client.
- MC owns its sessions outright — no parallel-resume + bracketed-paste pipeline.
- `OwnedSessionsState` persists to `~/.claude/fnba-mc/owned-sessions.json` and dedupes via PID liveness on load.

## v1.0 — 2026-05-19

The first formal release of FNBA Utils.

FNBA Utils is a Raycast-style command palette for FNBA developers. It exists to compress the small, recurring annoyances of daily FNBA dev — `assumeIdentity` stored-proc calls, rights and permissions lookups, juggling parallel Claude Code sessions — into a single hotkey.

Versioning starts here because the app is being used by more than its author. The intent for the 1.x line is unapologetically **developer quality of life**: no installer, no admin rights, no passwords, no config sprawl. Defaults work out of the box; per-user extras merge in from `~/.assumeIdentity.json`.

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
