// MRU registry for open Markdown Viewer windows. Stored in localStorage so it
// is shared across webviews (same origin). The switcher intersects this with the
// live window list, so stale entries (from a crash, etc.) are harmless.
//
// NOTE: the document body is stored on disk (see writeMarkdownDoc/readMarkdownDoc);
// only the file path is kept here. The try/catch wrappers mean persistence
// silently degrades — it never breaks the viewer.

const STORAGE_KEY = "fnba-utils:markdown-viewer-registry";

export interface MarkdownViewerEntry {
  focusedAt: number;
  preview: string;
  state?: {
    docPath: string | null;
    mode: string;
    filePath?: string | null;
    dirty?: boolean;
    diskMtimeMs?: number | null;
    diskSize?: number | null;
  };
  win?: {
    x: number;
    y: number;
    width: number;
    height: number;
    pinned: boolean;
    maximized: boolean;
  };
}

type Registry = Record<string, MarkdownViewerEntry>;

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

/** Update or insert an entry; preserves existing state/win/preview unless overridden. */
export function touchEntry(label: string, preview?: string): void {
  const reg = readRegistryRaw();
  const existing = reg[label];
  reg[label] = {
    ...existing,
    focusedAt: Date.now(),
    preview: preview !== undefined ? preview : (existing?.preview ?? ""),
  };
  writeRegistry(reg);
}

/** Persist viewer state (read-modify-write; preserves win and other fields). */
export function saveState(label: string, state: MarkdownViewerEntry["state"]): void {
  const reg = readRegistryRaw();
  const existing = reg[label] ?? { focusedAt: Date.now(), preview: "" };
  reg[label] = { ...existing, state };
  writeRegistry(reg);
}

/** Persist window geometry/pin/maximized (read-modify-write; preserves state and other fields). */
export function saveWin(label: string, win: MarkdownViewerEntry["win"]): void {
  const reg = readRegistryRaw();
  const existing = reg[label] ?? { focusedAt: Date.now(), preview: "" };
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
