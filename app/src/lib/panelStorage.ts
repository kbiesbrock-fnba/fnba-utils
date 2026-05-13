// Cross-window panel state shared via localStorage.
//
// IMPORTANT: the `kind:hash` window-label format is also assumed by
// `app/src-tauri/src/lib.rs` (Win+Shift+C hide path) and
// `app/src-tauri/capabilities/default.json` (window glob). If you change
// the prefix or separator, update those as well.

export type PanelKind = "sql-query" | "session-detail";

export interface SqlPanelPayload {
  server: string;
  label: string;
}

export interface DetailPanelPayload {
  sessionId: string;
  cwd: string;
  pid: number;
}

export type PinnedPanel =
  | ({ kind: "sql-query" } & SqlPanelPayload)
  | ({ kind: "session-detail" } & DetailPanelPayload);

export const PINNED_PANELS_KEY = "fnba-utils:pinned-panels";
export const LAST_FOCUSED_KEY = "fnba-utils:mc-last-focused-window";

export function readBool(key: string): boolean {
  try {
    return localStorage.getItem(key) === "true";
  } catch {
    return false;
  }
}

export function writeBool(key: string, v: boolean) {
  try {
    localStorage.setItem(key, String(v));
  } catch {
    /* ignore */
  }
}

function isPinnedPanel(p: unknown): p is PinnedPanel {
  if (!p || typeof p !== "object") return false;
  const k = (p as { kind?: unknown }).kind;
  if (k === "sql-query") {
    const q = p as { server?: unknown; label?: unknown };
    return typeof q.server === "string" && typeof q.label === "string";
  }
  if (k === "session-detail") {
    const d = p as { sessionId?: unknown; cwd?: unknown; pid?: unknown };
    return (
      typeof d.sessionId === "string" &&
      typeof d.cwd === "string" &&
      typeof d.pid === "number"
    );
  }
  return false;
}

export function readPinnedPanels(): PinnedPanel[] {
  try {
    const raw = localStorage.getItem(PINNED_PANELS_KEY);
    if (!raw) return [];
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(isPinnedPanel);
  } catch {
    return [];
  }
}

export function writePinnedPanels(list: PinnedPanel[]) {
  try {
    localStorage.setItem(PINNED_PANELS_KEY, JSON.stringify(list));
  } catch {
    /* ignore */
  }
}

function panelIdentityMatches(a: PinnedPanel, b: PinnedPanel): boolean {
  if (a.kind !== b.kind) return false;
  if (a.kind === "sql-query") {
    return a.server === (b as Extract<PinnedPanel, { kind: "sql-query" }>).server;
  }
  return (
    a.sessionId ===
    (b as Extract<PinnedPanel, { kind: "session-detail" }>).sessionId
  );
}

export function isPanelPinned(panel: PinnedPanel): boolean {
  return readPinnedPanels().some((p) => panelIdentityMatches(p, panel));
}

export function setPanelPinned(panel: PinnedPanel, pinned: boolean) {
  const list = readPinnedPanels();
  const idx = list.findIndex((p) => panelIdentityMatches(p, panel));
  if (pinned && idx < 0) {
    list.push(panel);
  } else if (!pinned && idx >= 0) {
    list.splice(idx, 1);
  } else {
    return;
  }
  writePinnedPanels(list);
}

export function readHashParams(): URLSearchParams {
  const hash = window.location.hash;
  const q = hash.indexOf("?");
  return new URLSearchParams(q >= 0 ? hash.slice(q + 1) : "");
}

export function rememberWindowFocus(label: string) {
  window.addEventListener("focus", () => {
    try {
      localStorage.setItem(LAST_FOCUSED_KEY, label);
    } catch {
      /* ignore */
    }
  });
}

export function readLastFocused(): string | null {
  try {
    return localStorage.getItem(LAST_FOCUSED_KEY);
  } catch {
    return null;
  }
}
