import { ref } from "vue";
import {
  getClaudeSessions,
  getConnectionStatuses,
  hideWindow,
  isTauri,
  type ClaudeSession,
  type ConnectionStatus,
} from "@/lib/tauri";

const PINNED_KEY = "fnba-utils:mission-control-pinned";
const CONNECTIONS_COLLAPSED_KEY = "fnba-utils:mc-connections-collapsed";
const SESSIONS_COLLAPSED_KEY = "fnba-utils:mc-sessions-collapsed";
const POLL_INTERVAL = 3000;
const CONNECTIONS_POLL_INTERVAL = 30000;
const BLUR_SUPPRESS_MS = 300;

function readBool(key: string): boolean {
  try {
    return localStorage.getItem(key) === "true";
  } catch {
    return false;
  }
}

function writeBool(key: string, v: boolean) {
  try {
    localStorage.setItem(key, String(v));
  } catch {
    // ignore
  }
}

const pinned = ref(readBool(PINNED_KEY));
const sessions = ref<ClaudeSession[]>([]);
const loading = ref(true);
const error = ref<string | null>(null);
const selectedPid = ref<number | null>(null);

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
    const next = await getClaudeSessions();
    // Skip reactive update if nothing changed
    if (JSON.stringify(next) !== JSON.stringify(sessions.value)) {
      sessions.value = next;
    }
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
    const next = await getConnectionStatuses();
    if (JSON.stringify(next) !== JSON.stringify(connectionStatuses.value)) {
      connectionStatuses.value = next;
    }
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

// --- Side window helpers (session-detail, sql-query, etc.) ---

async function showSideWindow(label: string) {
  if (!isTauri) return;

  const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const { PhysicalPosition } = await import("@tauri-apps/api/dpi");

  const win = await WebviewWindow.getByLabel(label);
  if (!win) return;

  const mcWin = getCurrentWindow();
  const [mcPos, mcSize] = await Promise.all([
    mcWin.outerPosition(),
    mcWin.outerSize(),
  ]);

  await win.setPosition(new PhysicalPosition(mcPos.x + mcSize.width + 8, mcPos.y));
  await win.show();
  await win.setFocus();
}

async function hideSideWindow(label: string) {
  if (!isTauri) return;
  const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
  const win = await WebviewWindow.getByLabel(label);
  if (win) await win.hide();
}

async function openSideWindowWithEvent(
  label: string,
  eventName: string,
  payload: Record<string, unknown>,
) {
  suppressBlur = true;
  try {
    await showSideWindow(label);
  } catch (e) {
    console.error(`[mc] show ${label} failed`, e);
  }
  setTimeout(() => { suppressBlur = false; }, BLUR_SUPPRESS_MS);

  if (isTauri) {
    const { emit } = await import("@tauri-apps/api/event");
    await emit(eventName, payload);
  }
}

async function selectSession(session: ClaudeSession) {
  selectedPid.value = session.pid;
  await openSideWindowWithEvent("session-detail", "session-selected", {
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
      if (!pinned.value && !suppressBlur) {
        dismiss();
      }
    });

    // Listen for session-killed events from detail window
    if (isTauri) {
      import("@tauri-apps/api/event").then(({ listen }) => {
        listen<{ pid: number }>("session-killed", () => {
          selectedPid.value = null;
          fetchSessions();
        });
      });
    }
  }

  function dismiss() {
    hideSideWindow("session-detail");
    hideSideWindow("sql-query");
    hideWindow();
  }

  async function selectConnection(status: ConnectionStatus) {
    // Write to localStorage so sql-query window can read on init
    // (before the event listener is ready)
    try {
      localStorage.setItem(
        "fnba-utils:sql-query-connection",
        JSON.stringify({ server: status.server, label: status.label }),
      );
    } catch {
      // ignore
    }

    await openSideWindowWithEvent("sql-query", "connection-selected", {
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
    sessionsCollapsed,
    dismiss,
    togglePin,
    selectSession,
    toggleSessionsCollapsed,
    connectionStatuses,
    connectionsLoading,
    connectionsCollapsed,
    toggleConnectionsCollapsed,
    refreshConnections,
    selectConnection,
  };
}
