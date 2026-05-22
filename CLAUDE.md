# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

fnba-utils is a monorepo containing shell extensions for FNBA development and a Tauri v2 desktop app. The desktop app is a Raycast/Spotlight-style command palette (Win+Shift+F) built with Vue 3 + TypeScript on the frontend and Rust on the backend.

## Build & Dev Commands

### Desktop app (UI only, no Rust needed)
```bash
cd app && docker compose up     # serves at localhost:5173 with mock Tauri API
```

### Desktop app (native, requires Windows Rust toolchain)
```bash
cd app && bash scripts/dev.sh   # builds Rust + launches Tauri dev window
```

### Type-check & build frontend
```bash
cd app && npm run build         # vue-tsc --noEmit && vite build
```

### Build Rust backend only
```bash
cd app/src-tauri && cargo build
```

## Architecture

### Tauri bridge pattern
`app/src/lib/tauri.ts` is the single gateway between frontend and backend. It detects whether it's running inside Tauri or a browser and routes `invoke()` calls accordingly:
- **Tauri mode**: forwards to real Rust commands via `@tauri-apps/api/core`
- **Browser mode**: uses `mockInvoke()` with realistic sample data for UI development without Rust

All Tauri command types (request/response interfaces) are defined in this file. The mock layer must stay in sync with the Rust command signatures.

### Command structure
Each command (e.g., Assume Identity) follows this pattern:
- **Rust**: `src-tauri/src/commands/<name>.rs` -- Tauri `#[tauri::command]` handlers, registered in `lib.rs`
- **Vue**: `src/components/<name>/` -- step-based UI components
- **Composable**: `src/composables/use<Name>.ts` -- shared reactive state + business logic
- **Command entry**: `src/commands/<name>.ts` + registered in `src/commands/index.ts`

### Assume Identity flow
The primary command. Steps: user picker -> connection picker -> confirm -> executing -> result/error. The composable (`useAssumeIdentity.ts`) manages step transitions and state. The Rust backend connects to SQL Server via `tiberius` with Windows SSPI auth and executes a single consolidated SQL batch that checks current state, conditionally runs the `logincheck.fnba.assumeIdentity` stored proc, and returns before/after snapshots.

### Mission Control + Claude session model
A separate `Win+Shift+C` window tracks Claude Code sessions launched **from** this app. External claude processes (IntelliJ plugin, plain WSL terminals) are not surfaced — MC is intentionally scoped to its own sessions.

- **Spawn shape**: `wsl.exe --cd <cwd> -e bash -ilc "cd <cwd> && tmux new-session -d -s claude-<uuid> 'claude --session-id <uuid> ...' && tmux attach -t claude-<uuid>"` inside a `portable_pty` PTY. tmux is required (see `app/src-tauri/src/commands/claude_io.rs`); the wrapper lets external terminals `tmux attach -t claude-<id>` to co-drive the session.
- **Terminal UI**: `ChatPane.vue` embeds `xterm.js`. Keystrokes → `write_session_pty`; PTY drain bytes → `pty` events → `xterm.write`. No bubble UI; no JSONL→DOM translation. Stats panel still reads JSONL for token/cost counts.
- **Disconnect vs Kill**: closing the panel calls `disconnect_session` (drops PTY, keeps tmux alive — session resumable). Explicit Kill calls `stop_claude_session` (kill tmux, remove from state, remove worktree). Drain-thread EOF cleanup probes `tmux has-session` to distinguish the two cases. Each `ClaudeIoSession` carries a generation tag so a stale drain can't evict a newer attach.
- **Persistent state** in `app/src-tauri/src/state/`:
  - `owned_sessions.rs` — `OwnedSession { session_id, cwd, pid, label, claude_home, worktree_path, tmux_session, generation }`, persisted to `~/.claude/fnba-mc/owned-sessions.json`. Liveness derived from `tmux list-sessions` (cached 2 s).
  - `projects.rs` — `Project { cwd, displayName, pinned, lastUsedAt, notes }`, persisted to `~/.claude/fnba-mc/projects.json`. Drives the launcher's pinned+MRU autocomplete and the `Win+Shift+N` MRU hotkey. `start_new_claude_session` calls `record_project_used` on every successful spawn.
- **Global shortcuts** (registered in `lib.rs` via `tauri_plugin_global_shortcut` / `RegisterHotKey`):
  - `Win+Shift+F` — command palette
  - `Win+Shift+C` — Mission Control panel
  - `Win+Shift+N` — launch a session in the most-recently-used project (emits `mc-mru-launch`, handled in `useMissionControl.ts`)
  - `Ctrl+Shift+Tab` — cycle focus through open `session-detail:*` windows (pure Rust, sorts by label hash for stable order)
- **Low-level keyboard hook** (`app/src-tauri/src/clipboard/hotkey.rs`, `WH_KEYBOARD_LL`): intercepts `Win+V` and `Win+Shift+V` before shell dispatch and swallows the keystroke with `LRESULT(1)`. Used instead of `RegisterHotKey` because the Windows shell already owns `Win+V` and corporate DLP agents commonly claim `Win+Shift+V`, both of which make `RegisterHotKey` fail. Shift-held selects the initial filter (`Some("pinned")`); no Shift = full history (`None`).

### Data sources
- `data/identity-defaults.json` -- default users/connections, embedded into Rust binary at compile time via `include_str!`
- `~/.assumeIdentity.json` -- user-added custom entries, merged at runtime
- `~/.claude/fnba-mc/owned-sessions.json` -- MC's persisted Claude session registry
- `~/.claude/fnba-mc/projects.json` -- MC's project registry (pinned + MRU)
- `localStorage` -- a few small UI prefs (chat debug toggle, panel pin state). Recent projects moved to the backend registry above.

### Shell extensions
`bashrc.d/` contains shell functions sourced via the root `.bashrc`. These are standalone bash scripts, not part of the Tauri app.
