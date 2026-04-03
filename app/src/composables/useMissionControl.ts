import { ref } from "vue";
import { getClaudeSessions, hideWindow, type ClaudeSession } from "@/lib/tauri";

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

export function useMissionControl() {
  if (!initialized) {
    initialized = true;
    startPolling();

    window.addEventListener("blur", () => {
      if (!pinned.value) {
        dismiss();
      }
    });
  }

  function dismiss() {
    // Reset any future interactive state here
    hideWindow();
  }

  function togglePin() {
    pinned.value = !pinned.value;
    writePinned(pinned.value);
  }

  return { pinned, sessions, loading, error, dismiss, togglePin };
}
