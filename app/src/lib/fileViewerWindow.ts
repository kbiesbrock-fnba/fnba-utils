// Spawns File Viewer windows — the unified home for both the JSON and
// Markdown viewers. Replaces jsonViewerWindow.ts + markdownViewerWindow.ts:
// one spawn/restore module, one registry (fileViewerRegistry.ts), one route
// (index.html#file-viewer). FileViewerApp.vue reads the registry's `kind` at
// mount to decide which body (JsonViewerApp / MarkdownViewerApp) to render.
//
// Window LABELS deliberately stay kind-prefixed (`json-viewer:<ts>-<seq>` /
// `markdown-viewer:<ts>-<seq>`) rather than collapsing to a single
// `file-viewer:` prefix. The Win+Shift+J global-shortcut handler in
// src-tauri/src/lib.rs hardcodes
// `l.starts_with("json-viewer:") || l.starts_with("markdown-viewer:")` to
// decide switcher-vs-spawn-fresh — that Rust code is out of scope for this
// change, so the label scheme it depends on is preserved exactly. Everything
// else about the two viewers (registry, spawn/restore module, route, palette
// commands) is unified.

import { rectVisibleOnAnyMonitor } from "./windowBounds";
import {
  seedEntry,
  touchEntry,
  saveState,
  saveWin,
  readRegistry,
  type FileViewerEntry,
  type ViewerKind,
} from "./fileViewerRegistry";

export type { ViewerKind } from "./fileViewerRegistry";

const BASE_OPTIONS: Record<string, unknown> = {
  url: "index.html#file-viewer",
  width: 1000,
  height: 700,
  minWidth: 600,
  minHeight: 400,
  resizable: true,
  decorations: false,
  shadow: true,
  transparent: false,
  backgroundColor: "#1e1e1e",
  visible: true,
  alwaysOnTop: false,
  skipTaskbar: false,
};

const TITLES: Record<ViewerKind, string> = {
  json: "JSON Viewer",
  markdown: "Markdown Viewer",
};

function labelPrefix(kind: ViewerKind): string {
  return kind === "markdown" ? "markdown-viewer:" : "json-viewer:";
}

function isViewerLabel(label: string): boolean {
  return label.startsWith("json-viewer:") || label.startsWith("markdown-viewer:");
}

// Monotonic suffix so two spawns in the same millisecond can't collide.
let seq = 0;

export interface OpenFileViewerOptions {
  kind: ViewerKind;
  /** JSON: raw text handed off via a pending-blob localStorage key. Markdown: initial doc body. */
  content?: string;
  /** Markdown only: binds the window to a real file on disk (Open / Save flow). */
  filePath?: string;
}

/**
 * Create and focus a brand-new File Viewer window with a unique label.
 *
 * JSON: `content`, if given, is stashed in localStorage
 * (`fnba-utils:file-viewer-pending`) and picked up by the new window on mount
 * — used by the palette's "Open in JSON Viewer" soft command to seed the
 * window with a pasted blob.
 *
 * Markdown: `content`/`filePath`, if given, are written to disk and
 * pre-seeded into the registry BEFORE the window is created — the new window
 * hydrates from its registry entry on mount (no pending-localStorage handoff
 * needed).
 */
export async function openNewFileViewerWindow(opts: OpenFileViewerOptions): Promise<void> {
  const { kind, content, filePath } = opts;
  const label = `${labelPrefix(kind)}${Date.now()}-${seq++}`;
  seedEntry(label, kind);

  if (kind === "markdown") {
    if ((content != null && content.trim() !== "") || filePath) {
      try {
        const { writeMarkdownDoc } = await import("./tauri");
        const docPath = await writeMarkdownDoc(label, content ?? "");
        const seedMode = content && content.trim() ? "preview" : "edit";
        saveState(label, { docPath, mode: seedMode, filePath: filePath ?? null, dirty: false });
        touchEntry(label, (content ?? "").replace(/\s+/g, " ").trim().slice(0, 60));
      } catch {
        // best-effort; window still opens empty
      }
    }
  } else if (content != null) {
    try {
      localStorage.setItem("fnba-utils:file-viewer-pending", content);
    } catch {
      // ignore
    }
  }

  const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
  const win = new WebviewWindow(label, { ...BASE_OPTIONS, title: TITLES[kind] });
  // visible:true shows it on creation; just pull focus once it's ready.
  win.once("tauri://created", () => {
    void win.setFocus().catch(() => {});
  });
}

/**
 * Reopen every File Viewer window that has a registry entry but no live
 * window. Called once at app startup to restore windows killed by a
 * recompile, quit, or crash. Windows the user explicitly closed are not in
 * the registry (removeEntry clears them) so they are not restored.
 *
 * Runs the one-time legacy-registry migration sweep (see
 * `migrateLegacyRegistries` below) first, so entries it moves into the
 * unified registry are picked up by the very same restore pass below.
 */
export async function restoreFileViewerWindows(): Promise<void> {
  try {
    await migrateLegacyRegistries();

    const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
    const registry = readRegistry();
    const allWindows = await WebviewWindow.getAll();
    const liveLabels = new Set(allWindows.map((w) => w.label));

    for (const [label, entry] of Object.entries(registry)) {
      if (!isViewerLabel(label)) continue;
      if (liveLabels.has(label)) continue;

      // Build window options, overlaying saved geometry when available. A saved
      // position that no longer lands on any attached monitor (undocked /
      // monitor removed) is dropped so the window centers on-screen instead of
      // opening off on a detached display; size + pin are still honored.
      const opts: Record<string, unknown> = { ...BASE_OPTIONS, title: TITLES[entry.kind ?? "json"] };
      if (entry.win) {
        const onScreen = await rectVisibleOnAnyMonitor(entry.win);
        Object.assign(
          opts,
          onScreen ? { x: entry.win.x, y: entry.win.y } : { center: true },
          {
            width: entry.win.width,
            height: entry.win.height,
            alwaysOnTop: entry.win.pinned,
          },
        );
      }

      // Reuse the saved label so the window hydrates itself from the same
      // registry entry on mount (label is the key into the localStorage registry).
      const win = new WebviewWindow(label, opts);

      if (entry.win?.maximized) {
        win.once("tauri://created", () => {
          void win.maximize().catch(() => {});
          // Do NOT setFocus — don't steal focus from whatever the user was doing.
        });
      }
      // No setFocus for restored windows.
    }
  } catch {
    // Restoration is best-effort; never crash the app over it.
  }
}

// --- One-time migration: fold the two legacy per-kind registries (from
// before JSON Viewer + Markdown Viewer were unified) into the new one. Runs
// before the normal restore pass above so migrated entries get respawned in
// the very same sweep — no separate spawn logic needed here. Deletes both
// legacy keys once done (even if there was nothing to migrate) so this is
// provably one-shot: every call after the first sees both keys already gone
// and returns immediately.
const LEGACY_JSON_KEY = "fnba-utils:json-viewer-registry";
const LEGACY_MARKDOWN_KEY = "fnba-utils:markdown-viewer-registry";

interface LegacyEntry {
  focusedAt?: number;
  preview?: string;
  state?: Record<string, unknown>;
  win?: FileViewerEntry["win"];
}

function parseLegacy(raw: string | null): Record<string, LegacyEntry> {
  if (!raw) return {};
  try {
    return JSON.parse(raw) as Record<string, LegacyEntry>;
  } catch {
    return {};
  }
}

async function migrateLegacyRegistries(): Promise<void> {
  let legacyJsonRaw: string | null;
  let legacyMdRaw: string | null;
  try {
    legacyJsonRaw = localStorage.getItem(LEGACY_JSON_KEY);
    legacyMdRaw = localStorage.getItem(LEGACY_MARKDOWN_KEY);
  } catch {
    return; // localStorage unavailable — nothing we can do
  }
  // Already migrated (or a fresh install that never had the legacy keys).
  if (legacyJsonRaw == null && legacyMdRaw == null) return;

  try {
    const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
    const allWindows = await WebviewWindow.getAll();
    const liveLabels = new Set(allWindows.map((w) => w.label));

    const migrateOne = (label: string, entry: LegacyEntry, kind: ViewerKind) => {
      // A live window still using the old label scheme means it's running
      // pre-upgrade code — shouldn't happen in practice (the whole app
      // reloads together), but skip it defensively rather than risk a
      // double-spawn under the same label.
      if (liveLabels.has(label)) return;
      seedEntry(label, kind);
      touchEntry(label, entry.preview ?? "");
      if (entry.state) saveState(label, entry.state as unknown as FileViewerEntry["state"]);
      if (entry.win) saveWin(label, entry.win);
    };

    for (const [label, entry] of Object.entries(parseLegacy(legacyJsonRaw))) {
      migrateOne(label, entry, "json");
    }
    for (const [label, entry] of Object.entries(parseLegacy(legacyMdRaw))) {
      migrateOne(label, entry, "markdown");
    }
  } finally {
    // Delete both legacy keys unconditionally so the sweep is provably
    // one-shot even if something above threw partway through.
    try {
      localStorage.removeItem(LEGACY_JSON_KEY);
    } catch {
      // ignore
    }
    try {
      localStorage.removeItem(LEGACY_MARKDOWN_KEY);
    } catch {
      // ignore
    }
  }
}
