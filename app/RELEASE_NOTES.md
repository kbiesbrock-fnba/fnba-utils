# Release Notes

## v1.7.0 — 2026-05-20

### Standup panel: wider rows, sub-rows for checklist items, polish

- **Panel width 600 → 1200**, min width 480.
- **Row layout rebuilt** on CSS subgrid: `[checkbox] [KEY+badge] [type pill] [checklist-icon] [summary…] [priority+due] [drag-handle]`. Drag handle owns its own rightmost column and no longer drifts left when other meta is absent.
- **Story points badge** replaces `KEY (N)` parens. Each integer value gets its own hand-picked color, alternating between deep and bright tones so adjacent numbers differ in lightness as well as hue (royal blue, bright cyan, deep teal, bright green, deep lime, bright yellow, deep amber, bright orange, deep red, bright pink, deep fuchsia, bright lavender, deep indigo for the Fibonacci scale + neighbors). Text color flips light/dark per chip so contrast holds.
- **Smart Checklist sub-rows.** Any issue with content in `cf[13097]` shows a collapsible "Toggle Smart Checklist" row with a `done/total` count pill. Expanded, each item is a 22px sub-row in the same subgrid with a tree-glyph indent and the checkbox state from Jira; headers render in uppercase. Sub-rows inherit the parent's status-stripe color (dimmer) and the parent's strikethrough/dim when checked off.
- **Full-task panel sections (Description, Specification) are collapsible buttons** with rotating chevrons. Default collapsed. Real `<button>` elements with `aria-expanded` + visible focus rings — tab to focus, Enter/Space to toggle. Same accessibility pattern applied to the panel's checklist toggle.

### Pipeline

The Smart Checklist field is now fetched in the initial JQL query (the panel pull), not just the per-issue detail fetch:

- `fetch_issues` includes `customfield_13097` in `fields`.
- New `extract_checklist_text` helper unwraps bare strings, `{ "v": "..." }` envelopes, `{ "text": "..." }`, ADF documents, and falls back to a raw-JSON dump for diagnostics.
- Parsed items + raw text persist into `run_snapshot.checklist_text` (new column, best-effort `ALTER TABLE` migration).
- `report_from_snapshot` reparses the stored text on read so historical runs render sub-rows too.
- `ChecklistItem` + `parse_checklist` moved from `commands/standup.rs` to `models/standup.rs` so both layers share.

### Refactor

- New `StandupTaskRow.vue` extracted from the duplicated bug/task row markup in `StandupPanelApp.vue`. One component renders both the main row and N checklist sub-rows.

## v1.6.1 — 2026-05-20

- **Standup panel default size** bumped from 360×640 to **600×960** (~2.5× area). Min size set to 360×480 so you can shrink it back if needed.
- **Resize handles wired** on all four edges and corners. The frameless panel uses Tauri's `startResizeDragging` since `decorations: false` strips the native OS edge hit zones. 6px edge zones, 12×12 corner zones.
- **Checklist diagnostic fallback.** `IssueDetail` now also returns `checklistRaw` (the raw field content from Jira before our parser touches it). When parsing yields zero items but the field had content, the full-task view shows a "Checklist (raw)" block with the literal text — so we can see the exact format Smart Checklist sends and fix the parser instead of guessing.
- **Parser broadened** to handle Railsware-style `*x` / `*X` / `*~` prefixes, `#`-style headers, and bare bullets with no space — in addition to the previously-supported markdown task syntax and `*` / `+` / `-` legacy bullets.

## v1.6.0 — 2026-05-20

### Standup panel polish + Smart Checklist support

Tighter panel, longer labels, and full-task view now mirrors what's in Jira:

- **Row height locked at 32px** (min/max/height all 32px). Rows stay the same size whether the panel is short or stretched tall.
- **Priority pill shows the full label** ("High", "Medium", "Lowest") instead of a single letter. Subgrid sizing absorbs the wider pill without disrupting alignment.
- **Spec block removed from the inline expansion.** Hitting the row now shows description only. The full-task window (double-click) still surfaces specification + checklist.
- **Smart Checklist** (Titanium plugin) is read from custom field `cf[13097]` (hard-coded — that's the FNBA-internal field ID) and rendered in the full-task window as a read-only checkbox list with a `checked/total` progress count. Supports header lines (`>`), markdown task syntax (`- [ ]` / `- [x]`), and legacy `*` / `+` / `-` prefixes.

### Config

The spec field is still resolved by display name (`spec_field_name`, default `"Specification Details"`) and cached per session.

## v1.5.0 — 2026-05-20

### Standup panel fixes + lazy description/spec

Quality-of-life pass on the panel row interactions and detail content:

- **"Open in Jira" now actually opens Jira.** WebView2 silently ignored `<a target="_blank">`; added `tauri-plugin-shell` and route external clicks through `shell.open()`. The capability is scoped to `*.atlassian.net` / `*.atlassian.com` so it can't be used to open arbitrary URLs.
- **"View full task" double-click is now reliable.** First-paint race fixed via a localStorage handoff: the panel writes the requested key to `fnba-utils:issue-detail-pending`, shows the window, then emits. The detail window reads the pending key on mount and listens for the event for subsequent updates.
- **Drag handle is visible by default** (0.55 opacity, full color, accent blue on hover) instead of only appearing on row hover.
- **More breathing room around the checkbox** (24px column, 10px column gap).
- **"Task" pill suppressed.** The default-everything-else type is uninformative, so the pill only renders for Bug / Story / Sub-task / Epic / Incident / etc. Grid subgrid still aligns rows.
- **Inline expanded view now shows description and specification.** On row expand, the panel lazy-fetches the full issue via `get_issue_detail` and caches per-key in memory so re-expanding the same row is instant. Both blocks live in a new `IssueRowDetail.vue` component (extracted from the duplicated markup that had grown between v1.4 sections).

### Spec field lookup

The "Specification" block reads from a custom Jira field whose name is configurable in `~/.fnba-utils/config.yaml`:

```yaml
standup:
  spec_field_name: "Specification Details"   # default
```

The Rust side resolves the display name → custom-field ID once per session by hitting `/rest/api/3/field`, caches the ID, and includes it in subsequent `get_issue_detail` requests. Set to empty/null to disable. The block renders ADF-flattened plain text just like description.

## v1.4.0 — 2026-05-20

### Standup row redesign

The panel row is now built on CSS subgrid so every row's columns line up across the list, regardless of which decorations are present:

- **Layout**: `[checkbox] [KEY (pts)] [type pill] [summary…] [priority · due · drag]`. Story points moved into parentheses next to the key — the dedicated points column on the far right is gone.
- **Issue type pill** colored by type — bug red, story green, task blue, epic purple — even though bugs already live in their own section, the pill stays for consistency and quick scanning.
- **Drag handle moved to the right edge** (`⋮⋮`), opacity 0 until you hover the row, then fades to ~60%. Hovering the handle itself goes to 100%.
- **Summary** stretches into the leftover space and ellipses cleanly; a `title` attribute exposes the full text on hover.
- **Status stripe** on the left edge of each row, colored by the issue's status group, replaces the implicit "no headings means no status indicator" gap from v1.3.

### Click + double-click

- **Single click on a row** expands an inline detail block underneath with status name, priority name, type, absolute due date, points, and two action links (open in Jira / view full task).
- **Double click on a row** opens a new always-on-top window with the full Jira task — assignee, reporter, labels, created/updated timestamps, and the full description (Atlassian Document Format flattened to plain text on the Rust side). The window reuses one instance: each double-click swaps the contents via the `issue-detail-open` event.

### Under the hood

- New Rust command `get_issue_detail(key)` hits `/rest/api/3/issue/{key}` with `fields=summary,status,priority,duedate,issuetype,customfield_10028,description,assignee,reporter,labels,created,updated`. Description is parsed from ADF on the Rust side so the frontend renders plain text inside the existing CSP.
- New Tauri window `issue-detail` (560×720, centered, resizable, not always-on-top so you can drag it off your work surface).
- Panel and inline detail share helpers for due-date / priority / type formatting.

## v1.3.0 — 2026-05-20

### Standup panel redesign

The panel is now structured around how you actually work the list, not how Jira groups statuses.

- **Bugs section at the top** with a single "🐞 Bugs" heading. Everything else flows below as one unheaded list.
- **Default sort** is status (In Progress → Review → To Do → Attention → Done), then priority (Highest → Lowest), then earliest due date.
- **Drag-and-drop reorder** within each section. Grab the `⋮⋮` handle on the left of any row. Manual order persists in SQLite — a reset button in the header throws it away if you want to fall back to the default sort.
- **Checkboxes replace the X**: items behave like a todo list. Check an item and the whole row gets a line-through; uncheck to bring it back. The "show completed" toggle in the header reveals items you've checked off.

### Jira fields

The fetch now pulls `priority`, `duedate`, and `issuetype` in addition to the existing fields. Each row shows:

- A priority pill (`H` / `M` / `L`) colored by tier.
- A due-date pill — "today", "tomorrow", "2d late", or a `5/22` short date — colored if soon or overdue.
- Story points right-aligned (unchanged from v1.2).

### Storage

- `issue_state` gains a `manual_order INTEGER NULL` column.
- `run_snapshot` gains `priority`, `priority_rank`, `due_date`, `issue_type`, `is_bug` columns.
- Best-effort `ALTER TABLE` migrations run on startup, so existing v1.2 databases pick up the new schema without losing history.

The Teams card still groups by status — that's the public artifact and didn't need this kind of personal customization.

## v1.2.0 — 2026-05-20

### Standup panel (opt-in)

Adds an always-on-top **Standup** panel, toggled with **`Win+Shift+D`**. Shows the most recent Jira fetch (grouped by status, with story points + a per-group total), with a refresh button that re-pulls from Jira without posting to Teams.

Each issue row has a hide button; dismissed items disappear from the panel but are preserved in SQLite. A toggle in the header reveals them again, and a counter shows how many are currently hidden.

The panel also exposes a **history drawer** — every `run_standup` invocation is recorded with its issue snapshot, so you can scroll back through past runs. Future versions will add "view this past snapshot inline."

The pin button (top-right) keeps the panel open when focus leaves; unpinned, it hides on blur like Mission Control.

State storage is local to the installation: `<exe-dir>/resources/standup.db` (SQLite) and `<exe-dir>/resources/standup-last-run.json`. The portable exe remains self-contained — copy the exe and its `resources/` sibling directory together to move installs.

Same opt-in gate as v1.1: `standup.enabled: true` in `~/.fnba-utils/config.yaml`. Without it, the panel window is never created and `Win+Shift+D` is a no-op.

## v1.1.0 — 2026-05-20

### Standup (opt-in)

New palette command **Standup** that pulls your assigned Jira issues and posts a redesigned Adaptive Card to a Teams channel. Two actions: **Fetch & Post to Teams** (full standup workflow) and **Fetch & Preview Only** (inline preview, no Teams post). The palette entry shows the last successful run time so you don't accidentally double-post.

The Teams card is grouped by status (In Progress / In Review / To Do / Done This Week), with per-issue rows that show the status emoji, key, summary, status name, and story points right-aligned. Each group header shows the issue count and total points.

**This feature is opt-in.** It is hidden from the palette entirely unless you create `~/.fnba-utils/config.yaml` with:

```yaml
standup:
  enabled: true
  jira_email: your.email@fnba.com
  jira_api_token: "..."          # https://id.atlassian.com/manage-profile/security/api-tokens
  jira_domain: fnba.atlassian.net
  teams_webhook_url: "..."       # optional; without it, Teams post is skipped
```

Credentials live in the YAML in plain text (same security posture as the original standup repo's `.env`). If the config file is absent or `enabled` is `false`, no Standup command appears and no credentials are read.

A persistent always-on-top "Today" panel — Jira tasks plus locally hidden/shown items, with history — is planned as a follow-up. For now, the command's inline preview is the only UI.

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
