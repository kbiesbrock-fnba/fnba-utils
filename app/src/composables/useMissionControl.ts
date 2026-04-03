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
const POLL_INTERVAL = 3000;
const CONNECTIONS_POLL_INTERVAL = 30000;

function readPinned(): boolean {
  try {
    return localStorage.getItem(PINNED_KEY) === "true";
  } catch {
    return false;
  }
}

function writePinned(v: boolean) {
  try {
    localStorage.setItem(PINNED_KEY, String(v));
  } catch {
    // ignore
  }
}

function readCollapsed(): boolean {
  try {
    return localStorage.getItem(CONNECTIONS_COLLAPSED_KEY) === "true";
  } catch {
    return false;
  }
}

function writeCollapsed(v: boolean) {
  try {
    localStorage.setItem(CONNECTIONS_COLLAPSED_KEY, String(v));
  } catch {
    // ignore
  }
}

const pinned = ref(readPinned());
const sessions = ref<ClaudeSession[]>([]);
const loading = ref(false);
const error = ref<string | null>(null);
const selectedPid = ref<number | null>(null);

const connectionStatuses = ref<ConnectionStatus[]>([]);
const connectionsLoading = ref(false);
const connectionsCollapsed = ref(readCollapsed());

let pollTimer: ReturnType<typeof setInterval> | null = null;
let connectionsPollTimer: ReturnType<typeof setInterval> | null = null;
let initialized = false;

async function fetchSessions() {
  try {
    loading.value = true;
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
    connectionsLoading.value = true;
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

async function showDetailWindow() {
  if (!isTauri) return;

  const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const { PhysicalPosition } = await import("@tauri-apps/api/dpi");

  const detailWin = await WebviewWindow.getByLabel("session-detail");
  console.log("[mc] detailWin =", detailWin);
  if (!detailWin) return;

  const mcWin = getCurrentWindow();
  const mcPos = await mcWin.outerPosition();
  const mcSize = await mcWin.outerSize();

  // Position detail window to the right of mission control with 8px gap
  const x = mcPos.x + mcSize.width + 8;
  const y = mcPos.y;

  await detailWin.setPosition(new PhysicalPosition(x, y));
  await detailWin.show();
  await detailWin.setFocus();
}

async function hideDetailWindow() {
  if (!isTauri) return;
  const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
  const win = await WebviewWindow.getByLabel("session-detail");
  if (win) await win.hide();
}

async function selectSession(session: ClaudeSession) {
  console.log("[mc] selectSession called", session.pid);
  selectedPid.value = session.pid;
  try {
    await showDetailWindow();
    console.log("[mc] showDetailWindow done");
  } catch (e) {
    console.error("[mc] showDetailWindow failed", e);
  }

  if (isTauri) {
    const { emit } = await import("@tauri-apps/api/event");
    await emit("session-selected", {
      sessionId: session.sessionId,
      cwd: session.cwd,
      pid: session.pid,
    });
    console.log("[mc] event emitted");
  }
}

export function useMissionControl() {
  if (!initialized) {
    initialized = true;
    startPolling();
    startConnectionsPolling();

    window.addEventListener("blur", () => {
      if (!pinned.value) {
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
    hideDetailWindow();
    hideWindow();
  }

  function togglePin() {
    pinned.value = !pinned.value;
    writePinned(pinned.value);
  }

  function toggleConnectionsCollapsed() {
    connectionsCollapsed.value = !connectionsCollapsed.value;
    writeCollapsed(connectionsCollapsed.value);
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
    dismiss,
    togglePin,
    selectSession,
    connectionStatuses,
    connectionsLoading,
    connectionsCollapsed,
    toggleConnectionsCollapsed,
    refreshConnections,
  };
}
