import { ref } from "vue";
import {
  getSessionDetail,
  killSession,
  sendSessionPrompt,
  openInExplorer,
  isTauri,
  type SessionDetail,
} from "@/lib/tauri";

const PINNED_KEY = "fnba-utils:session-detail-pinned";

const detail = ref<SessionDetail | null>(null);
const loading = ref(false);
const error = ref<string | null>(null);
const pinned = ref(localStorage.getItem(PINNED_KEY) === "true");
const sending = ref(false);

let listening = false;
let pollTimer: ReturnType<typeof setInterval> | null = null;

function togglePin() {
  pinned.value = !pinned.value;
  try { localStorage.setItem(PINNED_KEY, String(pinned.value)); } catch { /* ignore */ }
}

async function startListening() {
  if (listening) return;
  listening = true;

  window.addEventListener("blur", async () => {
    if (!pinned.value) {
      if (isTauri) {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        await getCurrentWindow().hide();
      }
    }
  });

  if (isTauri) {
    const { listen } = await import("@tauri-apps/api/event");
    await listen<{ sessionId: string; cwd: string; pid: number }>(
      "session-selected",
      (event) => {
        fetchDetail(
          event.payload.sessionId,
          event.payload.cwd,
          event.payload.pid,
        );
      },
    );
  }
}

async function fetchDetail(sessionId: string, cwd: string, pid: number) {
  loading.value = true;
  error.value = null;
  try {
    detail.value = await getSessionDetail(sessionId, cwd, pid);
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

async function kill() {
  if (!detail.value) return;
  const pid = detail.value.pid;
  await killSession(pid);
  detail.value = null;

  if (isTauri) {
    const { emit } = await import("@tauri-apps/api/event");
    await emit("session-killed", { pid });
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().hide();
  }
}

async function openCwd() {
  if (!detail.value) return;
  await openInExplorer(detail.value.cwd);
}

function copyInfo(): string {
  if (!detail.value) return "";
  const d = detail.value;
  const lines = [
    `Session: ${d.name ?? d.sessionId}`,
    `PID: ${d.pid}`,
    `CWD: ${d.cwd}`,
    d.gitBranch ? `Branch: ${d.gitBranch}` : null,
    `Status: ${d.status}`,
    `Messages: ${d.stats.messageCount} (${d.stats.userMessageCount} user, ${d.stats.assistantMessageCount} assistant)`,
    `Tokens: ${d.stats.totalInputTokens.toLocaleString()} in / ${d.stats.totalOutputTokens.toLocaleString()} out`,
    `Subagents: ${d.subagents.length}`,
  ].filter(Boolean);
  const text = lines.join("\n");
  navigator.clipboard.writeText(text);
  return text;
}

function stopPolling() {
  if (pollTimer) { clearInterval(pollTimer); pollTimer = null; }
}

function startPolling() {
  stopPolling();
  const start = Date.now();
  pollTimer = setInterval(async () => {
    if (!detail.value) { stopPolling(); return; }
    if (Date.now() - start > 60000) { stopPolling(); return; }
    await fetchDetail(detail.value.sessionId, detail.value.cwd, detail.value.pid);
    if (detail.value?.status === "idle") stopPolling();
  }, 3000);
}

async function sendPrompt(text: string) {
  if (!detail.value) return;
  sending.value = true;
  error.value = null;
  try {
    await sendSessionPrompt(detail.value.sessionId, detail.value.cwd, text);
    // Start polling to show conversation updates
    startPolling();
  } catch (e) {
    error.value = String(e);
  } finally {
    sending.value = false;
  }
}

export function useSessionDetail() {
  startListening();
  return { detail, loading, error, pinned, sending, kill, openCwd, copyInfo, fetchDetail, togglePin, sendPrompt };
}
