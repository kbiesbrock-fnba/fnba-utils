import { ref } from "vue";
import {
  getSessionDetail,
  killSession,
  openInExplorer,
  isTauri,
  type SessionDetail,
} from "@/lib/tauri";

const detail = ref<SessionDetail | null>(null);
const loading = ref(false);
const error = ref<string | null>(null);

let listening = false;

async function startListening() {
  if (listening) return;
  listening = true;

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

export function useSessionDetail() {
  startListening();
  return { detail, loading, error, kill, openCwd, copyInfo, fetchDetail };
}
