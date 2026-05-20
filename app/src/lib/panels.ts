// Shared window-creation defaults and label/URL helpers for Mission
// Control's side panels. Both `useMissionControl.ts` (when opening from MC)
// and `NewSessionCommand.vue` (when opening from the palette launcher) read
// from here so the two paths can't drift.

import { hashStr } from "@/lib/hash";
import type {
  DetailPanelPayload,
  PanelKind,
  PinnedPanel,
  SqlPanelPayload,
} from "@/lib/panelStorage";

/** Window options keyed by panel kind. Used as `new WebviewWindow(label, { ...PANEL_DEFAULTS[kind], url })`. */
export const PANEL_DEFAULTS: Record<PanelKind, Record<string, unknown>> = {
  "sql-query": {
    width: 700,
    height: 520,
    minWidth: 400,
    minHeight: 300,
    resizable: false,
    decorations: false,
    shadow: false,
    transparent: true,
    backgroundColor: "#00000000",
    visible: false,
    alwaysOnTop: true,
    skipTaskbar: true,
    title: "SQL Query",
  },
  "session-detail": {
    width: 880,
    height: 760,
    minWidth: 360,
    minHeight: 400,
    resizable: true,
    decorations: false,
    shadow: false,
    transparent: true,
    backgroundColor: "#00000000",
    visible: false,
    alwaysOnTop: true,
    skipTaskbar: true,
    title: "Session Detail",
  },
};

/** Primary key per panel kind — `sessionId` for session-detail, `server` for sql-query. */
export function panelKeyFor(
  kind: PanelKind,
  payload: SqlPanelPayload | DetailPanelPayload,
): string {
  return kind === "sql-query"
    ? (payload as SqlPanelPayload).server
    : (payload as DetailPanelPayload).sessionId;
}

/** Stable window label: `"<kind>:<base36 hash of key>"`. */
export function panelLabelFor(kind: PanelKind, key: string): string {
  return `${kind}:${hashStr(key)}`;
}

/** Build the `index.html#<kind>?<params>` URL the new webview loads. */
export function panelUrlFor(
  kind: PanelKind,
  payload: SqlPanelPayload | DetailPanelPayload,
): string {
  const params = new URLSearchParams();
  for (const [k, v] of Object.entries(payload)) {
    params.set(k, String(v));
  }
  return `index.html#${kind}?${params.toString()}`;
}

/** Convert a PinnedPanel descriptor back into the payload shape openOrFocusPanel expects. */
export function payloadOf(
  panel: PinnedPanel,
): SqlPanelPayload | DetailPanelPayload {
  if (panel.kind === "sql-query") {
    return { server: panel.server, label: panel.label };
  }
  return { sessionId: panel.sessionId, cwd: panel.cwd, pid: panel.pid };
}
