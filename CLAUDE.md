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
The primary command. Steps: user picker -> connection picker -> confirm -> executing -> result/error. The composable (`useAssumeIdentity.ts`) manages step transitions and state. The Rust backend connects to SQL Server via `tiberius` with Windows SSPI auth, runs a pre-flight identity check, then executes the switch via `logincheck.fnba.assumeIdentity` stored proc.

### Data sources
- `assumeIdentity/identity-defaults.json` -- default users/connections, embedded into Rust binary at compile time via `include_str!`
- `~/.assumeIdentity.json` -- user-added custom entries, merged at runtime
- `localStorage` -- recent user tracking (frontend only)

### Shell extensions
`bashrc.d/` contains shell functions sourced via the root `.bashrc`. These are standalone bash scripts, not part of the Tauri app.
