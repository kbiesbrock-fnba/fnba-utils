import { ref } from "vue";
import {
  getClaudeSessions,
  getConnectionStatuses,
  hideWindow,
  isTauri,
  type ClaudeSession,
  type ConnectionStatus,
} from "@/lib/tauri";
import {
  readBool,
  writeBool,
  readPinnedPanels,
  readLastFocused,
  rememberWindowFocus,
  type DetailPanelPayload,
  type PanelKind,
  type PinnedPanel,
  type SqlPanelPayload,
} from "@/lib/panelStorage";

const PINNED_KEY = "fnba-utils:mission-control-pinned";
const CONNECTIONS_COLLAPSED_KEY = "fnba-utils:mc-connections-collapsed";
const SESSIONS_COLLAPSED_KEY = "fnba-utils:mc-sessions-collapsed";
const POLL_INTERVAL = 3000;
const CONNECTIONS_POLL_INTERVAL = 30000;
const BLUR_SUPPRESS_MS = 300;

const PANEL_DEFAULTS: Record<PanelKind, Record<string, unknown>> = {
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
    width: 440,
    height: 640,
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

function hashStr(s: string): string {
  // Combined djb2 + FNV-1a 32-bit → ~64-bit effective hash. djb2 alone collides
  // around 65k items; doubling makes the birthday threshold 2^32 — out of reach
  // for the small set of sessions/connections we ever label.
  let h1 = 0;
  let h2 = 0x811c9dc5;
  for (let i = 0; i < s.length; i++) {
    const c = s.charCodeAt(i);
    h1 = ((h1 << 5) - h1 + c) | 0;
    h2 = Math.imul(h2 ^ c, 16777619);
  }
  return (h1 >>> 0).toString(36) + (h2 >>> 0).toString(36);
}

function panelKeyFor(kind: PanelKind, payload: SqlPanelPayload | DetailPanelPayload): string {
  return kind === "sql-query"
    ? (payload as SqlPanelPayload).server
    : (payload as DetailPanelPayload).sessionId;
}

function panelLabelFor(kind: PanelKind, key: string): string {
  return `${kind}:${hashStr(key)}`;
}

function panelUrlFor(
  kind: PanelKind,
  payload: SqlPanelPayload | DetailPanelPayload,
): string {
  const params = new URLSearchParams();
  for (const [k, v] of Object.entries(payload)) {
    params.set(k, String(v));
  }
  return `index.html#${kind}?${params.toString()}`;
}

function payloadOf(panel: PinnedPanel): SqlPanelPayload | DetailPanelPayload {
  if (panel.kind === "sql-query") {
    return { server: panel.server, label: panel.label };
  }
  return { sessionId: panel.sessionId, cwd: panel.cwd, pid: panel.pid };
}

const pinned = ref(readBool(PINNED_KEY));
const sessions = ref<ClaudeSession[]>([]);
const loading = ref(true);
const error = ref<string | null>(null);
const selectedPid = ref<number | null>(null);
const expandedPid = ref<number | null>(null);

const connectionStatuses = ref<ConnectionStatus[]>([]);
const connectionsLoading = ref(true);
const connectionsCollapsed = ref(readBool(CONNECTIONS_COLLAPSED_KEY));
const sessionsCollapsed = ref(readBool(SESSIONS_COLLAPSED_KEY));

let pollTimer: ReturnType<typeof setInterval> | null = null;
let connectionsPollTimer: ReturnType<typeof setInterval> | null = null;
let initialized = false;
let suppressBlur = false;

async function fetchSessions() {
  try {
    sessions.value = await getClaudeSessions();
    error.value = null;
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

function startPolling() {
  if (pollTimer) return;
  fetchSessions();
  pollTimer = setInterval(fetchSessions, POLL_INTERVAL);
}

async function fetchConnectionStatuses() {
  try {
    connectionStatuses.value = await getConnectionStatuses();
  } catch (e) {
    console.error("Connection status fetch failed:", e);
  } finally {
    connectionsLoading.value = false;
  }
}

function startConnectionsPolling() {
  if (connectionsPollTimer) return;
  fetchConnectionStatuses();
  connectionsPollTimer = setInterval(fetchConnectionStatuses, CONNECTIONS_POLL_INTERVAL);
}

// --- Dynamic side-panel windows (one per opener) ---

async function listSidePanels() {
  const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
  const all = await WebviewWindow.getAll();
  return all.filter(
    (w) =>
      w.label.startsWith("sql-query:") ||
      w.label.startsWith("session-detail:"),
  );
}

async function computePanelPosition(
  win: import("@tauri-apps/api/webviewWindow").WebviewWindow,
) {
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const { PhysicalPosition } = await import("@tauri-apps/api/dpi");

  const mcWin = getCurrentWindow();
  const [mcPos, mcSize, panelSize] = await Promise.all([
    mcWin.outerPosition(),
    mcWin.outerSize(),
    win.outerSize(),
  ]);

  // Bottom-align with MC so taller panels grow upward, not behind taskbar.
  const baseX = mcPos.x + mcSize.width + 8;
  const baseY = mcPos.y + mcSize.height - panelSize.height;

  // Cascade horizontally past any other visible panel.
  const others = (await listSidePanels()).filter((w) => w.label !== win.label);
  const probes = await Promise.all(
    others.map(async (w) => {
      if (!(await w.isVisible())) return null;
      const [wpos, wsize] = await Promise.all([w.outerPosition(), w.outerSize()]);
      return { x: wpos.x, w: wsize.width };
    }),
  );
  const occupied = probes.filter((p): p is { x: number; w: number } => p !== null);

  let x = baseX;
  const step = panelSize.width + 8;
  while (occupied.some((o) => x < o.x + o.w && x + panelSize.width > o.x)) {
    x += step;
  }

  return new PhysicalPosition(x, baseY);
}

async function openOrFocusPanel(
  kind: PanelKind,
  payload: SqlPanelPayload | DetailPanelPayload,
  options: { focus?: boolean } = {},
) {
  if (!isTauri) return;
  const focus = options.focus ?? true;

  const key = panelKeyFor(kind, payload);
  const label = panelLabelFor(kind, key);

  const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
  let win = await WebviewWindow.getByLabel(label);

  suppressBlur = true;
  try {
    if (!win) {
      const url = panelUrlFor(kind, payload);
      win = new WebviewWindow(label, { ...PANEL_DEFAULTS[kind], url });
      // Tauri's `once` auto-unlistens on first fire, so the firing listener
      // is fine — but the OTHER one never fires and leaks for the lifetime
      // of the window. Capture both unlisten handles and clean up the loser.
      // A 5s timeout protects against the case where neither event fires
      // (hung webview init); both listeners get cleaned up regardless.
      let resolved: (() => void) | null = null;
      let rejected: ((e: unknown) => void) | null = null;
      const ready = new Promise<void>((res, rej) => {
        resolved = res;
        rejected = rej;
      });
      const [unCreated, unError] = await Promise.all([
        win.once("tauri://created", () => resolved!()),
        win.once<string>("tauri://error", (e) => rejected!(e.payload)),
      ]);
      let timeoutId: ReturnType<typeof setTimeout> | null = null;
      const timed = new Promise<void>((_, rej) => {
        timeoutId = setTimeout(() => rej(new Error("window create timeout")), 5000);
      });
      try {
        await Promise.race([ready, timed]);
        unError(); // created fired; error listener is still attached
      } catch (e) {
        unCreated();
        unError();
        throw e;
      } finally {
        if (timeoutId) clearTimeout(timeoutId);
      }
    }
    const pos = await computePanelPosition(win);
    await win.setPosition(pos);
    await win.show();
    if (focus) await win.setFocus();
  } catch (e) {
    console.error(`[mc] openOrFocusPanel failed for ${label}`, e);
  }
  setTimeout(() => {
    suppressBlur = false;
  }, BLUR_SUPPRESS_MS);
}

async function hideAllSidePanels() {
  if (!isTauri) return;
  const panels = await listSidePanels();
  await Promise.all(panels.map((w) => w.hide()));
}

async function restorePinnedSidePanels() {
  if (!isTauri) return;
  const list = readPinnedPanels();
  if (list.length === 0) return;

  suppressBlur = true;
  try {
    for (const panel of list) {
      await openOrFocusPanel(panel.kind, payloadOf(panel), { focus: false });
    }

    const lastFocused = readLastFocused();

    const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
    const { getCurrentWindow } = await import("@tauri-apps/api/window");

    if (lastFocused && lastFocused !== "mission-control") {
      const target = await WebviewWindow.getByLabel(lastFocused);
      if (target && (await target.isVisible())) {
        await target.setFocus();
      } else {
        await getCurrentWindow().setFocus();
      }
    } else {
      await getCurrentWindow().setFocus();
    }
  } catch (e) {
    console.error("[mc] restorePinnedSidePanels failed", e);
  }
  setTimeout(() => {
    suppressBlur = false;
  }, BLUR_SUPPRESS_MS);
}

function toggleSessionExpand(session: ClaudeSession) {
  expandedPid.value = expandedPid.value === session.pid ? null : session.pid;
}

async function openSessionDetail(session: ClaudeSession) {
  selectedPid.value = session.pid;
  await openOrFocusPanel("session-detail", {
    sessionId: session.sessionId,
    cwd: session.cwd,
    pid: session.pid,
  });
}

export function useMissionControl() {
  if (!initialized) {
    initialized = true;
    startPolling();
    startConnectionsPolling();

    window.addEventListener("blur", () => {
      if (pinned.value || suppressBlur) return;
      // A click into an open sub-panel also blurs MC. Defer one tick so
      // OS focus is observable, then bail if any sub-panel now has focus
      // — that blur was internal to the MC group.
      setTimeout(async () => {
        if (isTauri) {
          try {
            const panels = await listSidePanels();
            const focused = await Promise.all(panels.map((w) => w.isFocused()));
            if (focused.some(Boolean)) return;
          } catch {
            // fall through to dismiss
          }
        }
        dismiss();
      }, 50);
    });

    rememberWindowFocus("mission-control");

    if (isTauri) {
      import("@tauri-apps/api/event").then(({ listen }) => {
        listen<{ pid: number }>("session-killed", () => {
          selectedPid.value = null;
          fetchSessions();
        });
        listen("mc-shown", () => {
          restorePinnedSidePanels();
        });
      });
    }
  }

  function dismiss() {
    hideAllSidePanels();
    hideWindow();
  }

  async function selectConnection(status: ConnectionStatus) {
    await openOrFocusPanel("sql-query", {
      server: status.server,
      label: status.label,
    });
  }

  function togglePin() {
    pinned.value = !pinned.value;
    writeBool(PINNED_KEY, pinned.value);
  }

  function toggleConnectionsCollapsed() {
    connectionsCollapsed.value = !connectionsCollapsed.value;
    writeBool(CONNECTIONS_COLLAPSED_KEY, connectionsCollapsed.value);
  }

  function toggleSessionsCollapsed() {
    sessionsCollapsed.value = !sessionsCollapsed.value;
    writeBool(SESSIONS_COLLAPSED_KEY, sessionsCollapsed.value);
  }

  function refreshConnections() {
    fetchConnectionStatuses();
  }

  return {
    pinned,
    sessions,
    loading,
    error,
    selectedPid,
    expandedPid,
    sessionsCollapsed,
    dismiss,
    togglePin,
    toggleSessionExpand,
    openSessionDetail,
    toggleSessionsCollapsed,
    connectionStatuses,
    connectionsLoading,
    connectionsCollapsed,
    toggleConnectionsCollapsed,
    refreshConnections,
    selectConnection,
  };
}
