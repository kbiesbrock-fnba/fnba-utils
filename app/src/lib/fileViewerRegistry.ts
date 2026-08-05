// Unified MRU registry for open File Viewer windows — both the JSON and
// Markdown kinds. Replaces the former jsonViewerRegistry.ts +
// markdownViewerRegistry.ts with a single localStorage key; entries carry a
// `kind` discriminant plus each kind's own (unchanged) `state` shape. Stored
// in localStorage so it is shared across webviews (same origin). The switcher
// intersects this with the live window list, so stale entries (from a crash,
// etc.) are harmless.
//
// NOTE: a multi-MB pasted JSON blob stored in `state.input` can exceed the
// localStorage quota. The try/catch wrappers mean persistence silently degrades
// (the viewer keeps working; state just won't survive a restart) — it never
// breaks the viewer.

const STORAGE_KEY = "fnba-utils:file-viewer-registry";

export type ViewerKind = "json" | "markdown";

export interface JsonViewerState {
  input: string;
  diffInput: string;
  mode: string;
  formatStyle: string;
  sortKeys: boolean;
  search: string;
}

export interface MarkdownViewerState {
  docPath: string | null;
  mode: string;
  filePath?: string | null;
  dirty?: boolean;
  diskMtimeMs?: number | null;
  diskSize?: number | null;
}

export interface FileViewerEntry {
  kind: ViewerKind;
  focusedAt: number;
  preview: string;
  state?: JsonViewerState | MarkdownViewerState;
  win?: {
    x: number;
    y: number;
    width: number;
    height: number;
    pinned: boolean;
    maximized: boolean;
  };
}

type Registry = Record<string, FileViewerEntry>;

function readRegistryRaw(): Registry {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return {};
    return JSON.parse(raw) as Registry;
  } catch {
    return {};
  }
}

function writeRegistry(reg: Registry): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(reg));
  } catch {
    // ignore — quota or private-browsing restriction
  }
}

/**
 * Seed a brand-new entry with its `kind` before any other write happens for
 * this label. Called once at window-creation time (before the WebviewWindow
 * itself is constructed) so `kind` is guaranteed present by the time the new
 * window's first paint runs FileViewerApp's dispatch logic.
 */
export function seedEntry(label: string, kind: ViewerKind): void {
  const reg = readRegistryRaw();
  const existing = reg[label];
  reg[label] = {
    ...existing,
    kind,
    focusedAt: existing?.focusedAt ?? Date.now(),
    preview: existing?.preview ?? "",
  };
  writeRegistry(reg);
}

/** Update or insert an entry; preserves existing state/win/preview/kind unless overridden. */
export function touchEntry(label: string, preview?: string): void {
  const reg = readRegistryRaw();
  const existing = reg[label];
  reg[label] = {
    ...existing,
    // Defensive fallback only — in practice seedEntry always runs first, so
    // `existing.kind` is already set by the time touchEntry is ever called.
    kind: existing?.kind ?? "json",
    focusedAt: Date.now(),
    preview: preview !== undefined ? preview : (existing?.preview ?? ""),
  };
  writeRegistry(reg);
}

/** Persist viewer state (read-modify-write; preserves win/kind and other fields). */
export function saveState(label: string, state: FileViewerEntry["state"]): void {
  const reg = readRegistryRaw();
  const existing = reg[label] ?? { kind: "json" as const, focusedAt: Date.now(), preview: "" };
  reg[label] = { ...existing, state };
  writeRegistry(reg);
}

/** Persist window geometry/pin/maximized (read-modify-write; preserves state/kind and other fields). */
export function saveWin(label: string, win: FileViewerEntry["win"]): void {
  const reg = readRegistryRaw();
  const existing = reg[label] ?? { kind: "json" as const, focusedAt: Date.now(), preview: "" };
  reg[label] = { ...existing, win };
  writeRegistry(reg);
}

/** Remove an entry (called on intentional window close). */
export function removeEntry(label: string): void {
  const reg = readRegistryRaw();
  delete reg[label];
  writeRegistry(reg);
}

/** Read the full registry (safe default: empty object). */
export function readRegistry(): Registry {
  return readRegistryRaw();
}
