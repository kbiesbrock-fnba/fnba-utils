import { ref } from "vue";
import {
  getClaudeSessions,
  hideWindow,
  isTauri,
  type ClaudeSession,
} from "@/lib/tauri";

const PINNED_KEY = "fnba-utils:mission-control-pinned";
const POLL_INTERVAL = 3000;

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

const pinned = ref(readPinned());
const sessions = ref<ClaudeSession[]>([]);
const loading = ref(false);
const error = ref<string | null>(null);
const selectedPid = ref<number | null>(null);

let pollTimer: ReturnType<typeof setInterval> | null = null;
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

  return {
    pinned,
    sessions,
    loading,
    error,
    selectedPid,
    dismiss,
    togglePin,
    selectSession,
  };
}
