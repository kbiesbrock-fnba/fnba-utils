// MRU registry for open JSON Viewer windows. Stored in localStorage so it is
// shared across webviews (same origin). The switcher intersects this with the
// live window list, so stale entries (from a crash, etc.) are harmless.
//
// NOTE: a multi-MB pasted blob stored in `state.input` can exceed the
// localStorage quota. The try/catch wrappers mean persistence silently degrades
// (the viewer keeps working; state just won't survive a restart) — it never
// breaks the viewer.

const STORAGE_KEY = "fnba-utils:json-viewer-registry";

export interface JsonViewerEntry {
  focusedAt: number;
  preview: string;
  state?: {
    input: string;
    diffInput: string;
    mode: string;
    formatStyle: string;
    sortKeys: boolean;
    search: string;
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

type Registry = Record<string, JsonViewerEntry>;

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
export function saveState(label: string, state: JsonViewerEntry["state"]): void {
  const reg = readRegistryRaw();
  const existing = reg[label] ?? { focusedAt: Date.now(), preview: "" };
  reg[label] = { ...existing, state };
  writeRegistry(reg);
}

/** Persist window geometry/pin/maximized (read-modify-write; preserves state and other fields). */
export function saveWin(label: string, win: JsonViewerEntry["win"]): void {
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
