# Plan: Soft Commands Phase 2 — Filesystem Path Actions

## Goal and Scope

When the user pastes or types a filesystem path into the command palette (Windows-style
`C:\dev\foo`, WSL-style `/mnt/c/dev/foo`, or bare POSIX `/home/user/bar`), the palette
offers contextual one-shot actions: open in Explorer, open in editor (IntelliJ → Explorer
fallback), run in terminal (with `cd` pre-loaded), open in Notepad++, and copy the path.
Rows are file-vs-folder aware; a nonexistent path still surfaces a minimal row set so
the user can open the parent in Explorer.

**In scope:** `isPath` detection, four action rows, a new `stat_path` Rust command, a
new `open_in_notepadpp` Rust command, and wiring the `run_in_terminal` registration gap
(`terminal.rs` exists but is not yet registered in `lib.rs`).

**Out of scope:** path completion/autocomplete while typing, multi-path handling,
drag-and-drop, UNC network shares as a primary surface.

---

## Architecture Decision: Sync vs Async

### The problem

`buildSoftCommands(query)` is called inside a Vue `computed()` — it must return
synchronously. A filesystem `stat` call is async (any Tauri invoke is a `Promise`).
Enriching rows with live file/folder metadata therefore cannot happen inside the
`computed` directly.

### Decision: action-time stat (synchronous row build; async validation at click)

Build path rows **synchronously and unconditionally** from the regex alone, exactly as
all other soft-command branches do. Each action's `action` callback calls `stat_path`
at the moment the user presses Enter/clicks, then branches on the result:

- If the path exists as a file → open in editor / Notepad++ as a file.
- If the path exists as a directory → open in Explorer directly; "open in editor" opens
  Explorer because IntelliJ does not handle a bare folder from the CLI reliably.
- If the path does not exist → actions that require existence are skipped (the callback
  returns early with a no-op or copies the path as-is).

**Why this beats a debounced async ref:**

1. No changes to `usePalette.ts` or `filteredCommands` — `computed()` stays synchronous.
2. No reactive ref churn on every keystroke (stat fires only when the user commits).
3. The "wrong rows for a nonexistent path" problem is negligible: the path regex is a
   positive match, and a missing path is a corner case (user copy-pasted a stale path).
4. Debounced async would require either a separate `ref<PathStat | null>` managed
   outside the `computed`, or converting `filteredCommands` to an `async computed` — both
   are significant surgery to `usePalette.ts` for modest UX gain.

The only downside is that all four rows appear even for a nonexistent path. Each row
action performs the stat and reacts appropriately, so the user gets clear feedback (the
terminal action opens a terminal cd-ing to the closest existing parent; Explorer opens
the parent folder). This is the same pattern used by all existing soft-command action
callbacks (e.g., URL open calls `openExternal` which may 404; JSON open may throw for
malformed content — both are caught in the `runOrSelect` `.catch` in `usePalette.ts:77`).

---

## File-by-File Change List

### 1. `app/src/lib/patterns.ts` — no changes needed

`isPath` (line 39) already covers the three path forms:

```
const PATH_RE = /^(?:[A-Za-z]:[/\\]|\/)\S*/;
```

`C:\…` and `C:/…` match via the `[A-Za-z]:[/\\]` branch; `/mnt/c/…` and bare POSIX
paths match via `\/`. Export it if it is not already (`isPath` is exported at line 39 —
confirmed).

One risk: bare `/` (or a single slash from a mistype) matches. Mitigate by requiring at
least one more character after the root — update PATH_RE to:

```ts
// Before:
const PATH_RE = /^(?:[A-Za-z]:[/\\]|\/)\S*/;

// After (require at least one non-whitespace char after the root separator):
const PATH_RE = /^(?:[A-Za-z]:[/\\]\S+|\/\S+)/;
```

This drops bare `C:\` (drive root only) and bare `/` as matches. Both are uncommon
in practice and their inclusion would fire the path branch on almost any single-slash
query. Drive roots (`C:\`) are still reachable: typing `C:\` followed by any character
matches. If bare drive roots are desired later, the regex can be relaxed.

### 2. `app/src/lib/tauri.ts` — add two bindings

After the existing `openInExplorer` binding (line 888):

```ts
export interface PathStat {
  exists: boolean;
  isFile: boolean;
  isDir: boolean;
  /** Canonical Windows form, e.g. "C:\dev\foo". Empty string if path does not exist. */
  windowsPath: string;
  /** Canonical WSL form, e.g. "/mnt/c/dev/foo". Empty string if path does not exist. */
  wslPath: string;
}

export function statPath(path: string): Promise<PathStat> {
  return invoke<PathStat>("stat_path", { path });
}

export function openInNotepadpp(path: string): Promise<void> {
  return invoke<void>("open_in_notepadpp", { path });
}
```

### 3. `app/src/lib/softCommands.ts` — add import and path branch

**Import additions** (top of file, near the existing tauri imports at lines 12-13):

```ts
import { openInExplorer, openPathInEditor, statPath, openInNotepadpp } from "@/lib/tauri";
import { isPath } from "@/lib/patterns";
```

**New helper function** (add before `buildSoftCommands`, around line 148):

```ts
// ─── Path soft commands ───────────────────────────────────────────────────────

function buildPathRows(rawPath: string): PaletteCommand[] {
  return [
    row({
      id: "soft:path:explorer",
      name: "Open in Explorer",
      description: rawPath,
      icon: "📂",
      action: async () => {
        const s = await statPath(rawPath);
        // For a file: open parent folder; for a dir: open it directly.
        // open_in_explorer already handles WSL/Windows path conversion.
        const target = s.exists && s.isFile
          ? rawPath.replace(/[/\\][^/\\]+$/, "") || rawPath
          : rawPath;
        await openInExplorer(target);
      },
    }),
    row({
      id: "soft:path:editor",
      name: "Open in editor",
      description: "IntelliJ → Explorer fallback",
      icon: "✏️",
      action: async () => {
        await openPathInEditor(rawPath);
      },
    }),
    row({
      id: "soft:path:terminal",
      name: "Open terminal here",
      description: rawPath,
      icon: "💻",
      action: async () => {
        const s = await statPath(rawPath);
        // cd to the path if it's a dir; cd to parent if it's a file; fall
        // back to the raw path string if it doesn't exist (user may still
        // want to start a session near a planned location).
        const dir = s.exists
          ? (s.isDir ? s.wslPath || rawPath : (s.wslPath || rawPath).replace(/[/\\][^/\\]+$/, ""))
          : rawPath;
        await runInTerminal(`cd ${JSON.stringify(dir)}`);
      },
    }),
    row({
      id: "soft:path:notepadpp",
      name: "Open in Notepad++",
      description: rawPath,
      icon: "📝",
      action: async () => {
        await openInNotepadpp(rawPath);
      },
    }),
    row({
      id: "soft:path:copy",
      name: "Copy path",
      description: rawPath,
      icon: "📋",
      action: () => copyText(rawPath),
    }),
  ];
}
```

**Branch in `buildSoftCommands`** — insert after the `md ` prefix check (line 189) and
**before** the `URL_RE` check (line 192). This ordering is critical (see Ordering section
below):

```ts
  // --- Filesystem path (C:\…, /mnt/c/…, /path/…) ---
  // Must come after URL/prefix checks and before the markdown sniff.
  if (isPath(q)) {
    return buildPathRows(q);
  }
```

### 4. `app/src-tauri/src/commands/fs.rs` — new file

```rust
//! Filesystem introspection commands for the soft-command layer.
//! No persistent state; no Tauri manages required.

use crate::commands::claude_io::wsl_path_to_windows;

/// Translate a Windows path like `C:\dev\foo` to `/mnt/c/dev/foo`.
/// UNC paths under `\\wsl.localhost\Ubuntu\…` become `/…`.
/// Already-WSL paths pass through unchanged.
fn windows_path_to_wsl_local(path: &str) -> String {
    let s = path.replace('\\', "/");
    if let Some(rest) = s.strip_prefix("//wsl.localhost/") {
        return match rest.split_once('/') {
            Some((_distro, tail)) => format!("/{tail}"),
            None => "/".to_string(),
        };
    }
    if let Some((drive, rest)) = s.split_once(":/") {
        if drive.len() == 1 && drive.chars().all(|c| c.is_ascii_alphabetic()) {
            return format!("/mnt/{}/{}", drive.to_lowercase(), rest);
        }
    }
    // Already looks like a WSL/POSIX path, or unrecognized — pass through.
    path.to_string()
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathStat {
    pub exists: bool,
    pub is_file: bool,
    pub is_dir: bool,
    /// Canonical Windows form (empty string when path does not exist).
    pub windows_path: String,
    /// Canonical WSL form (empty string when path does not exist).
    pub wsl_path: String,
}

/// Stat a path (WSL or Windows form). Returns existence + type information
/// plus canonical forms of the path in both representations.
///
/// Resolution strategy: convert to WSL form first, stat on the Linux side
/// (this process runs in WSL), then derive the Windows form via
/// `wsl_path_to_windows`.
#[tauri::command]
pub fn stat_path(path: String) -> PathStat {
    let wsl = windows_path_to_wsl_local(&path);
    match std::fs::metadata(&wsl) {
        Ok(m) => PathStat {
            exists: true,
            is_file: m.is_file(),
            is_dir: m.is_dir(),
            windows_path: wsl_path_to_windows(&wsl),
            wsl_path: wsl,
        },
        Err(_) => PathStat {
            exists: false,
            is_file: false,
            is_dir: false,
            windows_path: String::new(),
            wsl_path: String::new(),
        },
    }
}

/// Open a file or folder in Notepad++. Accepts WSL or Windows paths.
/// Notepad++ is only available when `notepad++.exe` / `notepadpp.exe` is on
/// the Windows PATH. Falls back silently to Explorer on failure so the action
/// never errors visibly.
///
/// Notepad++ workspace open (`--multiInst -nosession <file>`) is used so the
/// file opens in a new instance rather than hijacking an existing session.
#[tauri::command]
pub fn open_in_notepadpp(path: String) -> Result<(), String> {
    let windows = wsl_path_to_windows(&windows_path_to_wsl_local(&path));

    // Try the two most common Notepad++ binary names. Corporate installs vary.
    let launched = ["notepad++.exe", "notepadpp.exe"].iter().any(|bin| {
        std::process::Command::new(bin)
            .args(["--multiInst", "-nosession", &windows])
            .spawn()
            .is_ok()
    });

    if launched {
        return Ok(());
    }

    // Fall back to Explorer (opens with the registered handler or Explorer).
    std::process::Command::new("explorer.exe")
        .arg(&windows)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to open path: {e}"))
}
```

**Note on `wsl_path_to_windows` visibility:** this function is currently a private `fn`
in `claude_io.rs` (line 1501). It must be made `pub` or moved to a shared location before
`fs.rs` can use it. The recommended approach is to move it (and its companion
`windows_path_to_wsl`) to a new `app/src-tauri/src/util/paths.rs` module, then update
`claude_io.rs` and `mission_control.rs` to import from there. This also eliminates the
duplicated Windows→WSL conversion that currently exists independently in both files.
(As a simpler short-term option, just change `wsl_path_to_windows` to `pub(crate)` in
`claude_io.rs` and `use crate::commands::claude_io::wsl_path_to_windows;` in `fs.rs` —
this is lower-risk and defers the refactor.)

### 5. `app/src-tauri/src/commands/mod.rs` — add fs module

Add to the top of the file (after the last `pub mod` line, which is currently `pub mod terminal;` at line 11):

```rust
pub mod fs;
```

### 6. `app/src-tauri/src/lib.rs` — register all three new commands

Three additions to the `invoke_handler!` list (after `commands::markdown_docs::read_markdown_file` at line 466):

```rust
            commands::terminal::run_in_terminal,   // was in terminal.rs but never registered
            commands::fs::stat_path,
            commands::fs::open_in_notepadpp,
```

**Important:** `run_in_terminal` currently lives in `terminal.rs` (which is registered as
a module in `commands/mod.rs` at line 11) but is **absent from the `generate_handler!`
list in `lib.rs`**. The TS binding in `tauri.ts` (line 971) and the Stage 1 `>` prefix
branch in `softCommands.ts` (line 160) both call it — so this is a latent bug that Stage
2 work should fix regardless.

---

## New Rust Commands — Design Summary

### `stat_path(path: String) -> PathStat`

- **Location:** `app/src-tauri/src/commands/fs.rs`
- **Sync or async:** synchronous (`pub fn`, not `pub async fn`) — `std::fs::metadata`
  does not need async.
- **Return type:** `PathStat { exists, is_file, is_dir, windows_path, wsl_path }` all
  camelCase via `#[serde(rename_all = "camelCase")]`.
- **Path handling:** convert input to WSL form first (since the process runs in WSL and
  `std::fs::metadata` uses the Linux VFS), then stat. Derive Windows form via
  `wsl_path_to_windows` from `claude_io.rs`.
- **Errors:** never errors; nonexistent paths return `exists: false` with empty string
  path fields. The frontend never needs to `catch`.

### `open_in_notepadpp(path: String) -> Result<(), String>`

- **Location:** `app/src-tauri/src/commands/fs.rs`
- **Sync:** yes.
- **Binary discovery:** tries `notepad++.exe` then `notepadpp.exe` from PATH; falls back
  to `explorer.exe`. No registry lookup — corporate images vary too much.
- **Args:** `--multiInst -nosession <windows_path>` — opens a fresh instance without
  disturbing any open sessions.
- **Error:** returns `Err` only if Explorer itself fails (effectively never).

### `run_in_terminal` (fix registration gap)

- **Existing implementation:** `app/src-tauri/src/commands/terminal.rs`, fully
  implemented at line 12.
- **Change needed:** add `commands::terminal::run_in_terminal` to `lib.rs`
  `generate_handler!` list. That is the entire fix.

---

## Row Set Per Case

All four non-copy rows are always shown — stat happens at action time, not row-build
time. The action callbacks handle each case as follows:

| Action | Path exists as file | Path exists as dir | Path does not exist |
|--------|--------------------|--------------------|---------------------|
| Open in Explorer | Opens parent folder in Explorer | Opens the folder | Opens nearest existing ancestor (walk up until found) or does nothing |
| Open in editor | `open_path_in_editor` → IntelliJ or Explorer | Same — IntelliJ CLI opens the project; Explorer opens the dir | IntelliJ/Explorer receives the path — IDE will warn about missing file |
| Open terminal here | `cd <parent of file>` in new wt.exe tab | `cd <dir>` | `cd <raw path>` (user lands in a shell where they can mkdir etc.) |
| Open in Notepad++ | Opens file in Notepad++ | Notepad++ opens the directory (shows folder in its tree panel) | Notepad++ receives path; it shows a "file not found" dialog internally |
| Copy path | Always copies the raw query text | Same | Same |

---

## Path Detection Ordering and Ambiguity

### Position in the if-chain

The new `isPath` branch is inserted **after** the explicit prefix checks (`>`, `)`, `md `)
and **before** `URL_RE`. The final order:

1. `>` run-in-terminal prefix
2. `)` time prefix
3. `md ` markdown prefix
4. **`isPath` — new**
5. `URL_RE`
6. `JIRA_KEY_RE`
7. `isJsonText`
8. `=` calculator
9. `looksLikeMarkdown` (catch-all)

### Why before URL_RE

`file://` URLs (`file:///C:/dev/foo`) match `URL_RE` (they are `https?://`-less and do
not start with `www.`, so they would actually fall through `URL_RE` — `URL_RE` requires
`^(https?:\/\/|www\.)`, so `file://` URLs already do NOT match it). However, to be
explicit: if a user types `file:///C:/dev/foo`, it will not match `URL_RE` and will
reach `isPath` where `PATH_RE` also does not match (it starts with `file:///`, not a
drive letter or `/`). No conflict.

A bare Windows path like `C:\dev\foo` could theoretically look like a drive letter to
some parsers. `PATH_RE` is unambiguous: `[A-Za-z]:[/\\]\S+` requires the colon-
backslash/slash sequence that no URL or Jira key or JSON blob starts with.

### False-positive risks

- **Single-character drive letter followed by colon:** `C:` alone does not match the
  updated regex (requires at least one more non-whitespace char after the separator).
- **`/` alone or `//`:** Does not match the updated `\/\S+` form.
- **Short Unix paths like `/tmp`:** Match and surface path rows. This is intentional —
  `/tmp` is a valid path worth opening.
- **Markdown starting with a heading (`# foo`):** starts with `#`, not a path character.
  No conflict.
- **A SQL keyword line like `SELECT /…`:** The `isSql` check is not in `buildSoftCommands`
  at all (Stage 1 did not include it). The path regex requires the path to fill the whole
  query (`^`…), so `SELECT /foo` does not match `isPath`. No conflict.

---

## Edge Cases and Risks

1. **`wsl_path_to_windows` visibility.** It is `fn` (private) in `claude_io.rs`. The
   shortest fix is `pub(crate)`. Do this before building `fs.rs`.

2. **WSL path for a native Windows path typed by the user.** `windows_path_to_wsl_local`
   in `fs.rs` handles `C:\…` → `/mnt/c/…` correctly. Paths already in WSL form
   (`/mnt/c/…`) pass through unchanged. Pure Linux paths (`/home/user/…`) stat correctly
   because the Tauri process runs in WSL.

3. **UNC paths (`\\server\share\…`).** `PATH_RE` does not match `\\` (starts with `\`),
   so they will not trigger path rows. This is acceptable for now.

4. **IntelliJ not installed.** `open_path_in_editor` already gracefully falls back to
   Explorer (line 1461 in `claude_io.rs`). No additional handling needed.

5. **Notepad++ not installed.** `open_in_notepadpp` falls back to Explorer. The action
   row label says "Open in Notepad++" regardless — consider whether to show a "not
   available" description in a future iteration.

6. **`run_in_terminal` on non-Windows.** `terminal.rs` line 26 already returns
   `Err("run_in_terminal is only supported on Windows")`. The TS binding will catch this
   in `usePalette.ts:77` `.catch(e => console.error(...))`.

7. **Terminal `cd` with paths containing spaces or special characters.** The `buildPathRows`
   callback uses `JSON.stringify(dir)` which double-quotes the path. Bash handles this
   correctly. Alternative: use single quotes and escape embedded single quotes — but
   double-quoting is simpler and covers the typical `Program Files` case.

8. **The `runOrSelect` `.finally(() => dismiss())` path.** All action callbacks are
   `async`; `usePalette.ts:77` wraps them in `Promise.resolve(cmd.action()).catch(...)
   .finally(() => dismiss())`. The dismiss fires even if `statPath` is slow (it is a
   local stat, typically < 1 ms). No spinner or loading state is needed.

9. **Path regex and the `>` run-in-terminal prefix.** A path like `> /tmp/foo` (with
   leading `>`) will match the `>` prefix branch first (line 158) and surface a "Run in
   terminal" row with command `/tmp/foo`. This is correct — the user explicitly opted
   for the terminal prefix.

---

## Build and Verify Checklist

- [ ] Change `wsl_path_to_windows` in `claude_io.rs` to `pub(crate)`.
- [ ] Create `app/src-tauri/src/commands/fs.rs` with `stat_path` and `open_in_notepadpp`.
- [ ] Add `pub mod fs;` to `app/src-tauri/src/commands/mod.rs`.
- [ ] Add `commands::terminal::run_in_terminal`, `commands::fs::stat_path`,
      `commands::fs::open_in_notepadpp` to the `generate_handler!` list in `lib.rs`.
- [ ] Add `PathStat` interface, `statPath()`, and `openInNotepadpp()` bindings to
      `app/src/lib/tauri.ts`.
- [ ] Update `PATH_RE` in `app/src/lib/patterns.ts` to require at least one char after
      the root separator.
- [ ] Add `buildPathRows` helper and the `isPath(q)` branch to
      `app/src/lib/softCommands.ts` — insert between `md ` block and URL block.
- [ ] `cd app && npm run build` — must pass `vue-tsc --noEmit`.
- [ ] `cd app/src-tauri && cargo build` — must compile clean.
- [ ] Manual test: type `C:\dev\fnba-utils` in the palette → five rows appear.
  - Enter on "Open in Explorer" → Explorer opens the folder.
  - Enter on "Open in editor" → IntelliJ opens the project (or Explorer if not installed).
  - Enter on "Open terminal here" → a new Windows Terminal tab opens `cd`-ed to the dir.
  - Enter on "Open in Notepad++" → Notepad++ opens (or Explorer fallback).
  - Enter on "Copy path" → path is in clipboard.
- [ ] Manual test: type `/mnt/c/dev/fnba-utils/app/src/lib/softCommands.ts` → same rows,
      "Open terminal here" cds to the parent directory (not the file itself).
- [ ] Manual test: type `/nonexistent/path` → rows appear, actions complete without crash.
- [ ] Manual test: type `> /tmp/foo` → terminal run-in-terminal branch fires (not path branch).
- [ ] Manual test: type `https://example.com` → URL branch fires (not path branch).

---

## Release Notes

Per repo convention, add a recipient-facing entry to `app/RELEASE_NOTES.md` under
`[Unreleased]` when this ships. Do not add it now — the entry belongs at the commit
that merges the working feature, not during planning.
