import { ref } from "vue";
import {
  getSessionDetail,
  killSession,
  stopClaudeSession,
  openInExplorer,
  isTauri,
  type SessionDetail,
} from "@/lib/tauri";
import {
  isPanelPinned,
  readHashParams,
  rememberWindowFocus,
  setPanelPinned,
  type PinnedPanel,
} from "@/lib/panelStorage";

const params = readHashParams();
const initialSessionId = params.get("sessionId") ?? "";
const initialCwd = params.get("cwd") ?? "";
const parsedPid = Number.parseInt(params.get("pid") ?? "", 10);
const initialPid = Number.isFinite(parsedPid) && parsedPid > 0 ? parsedPid : 0;
const hasInitial = !!initialSessionId && !!initialCwd && initialPid > 0;

const detail = ref<SessionDetail | null>(null);
const loading = ref(false);
const error = ref<string | null>(null);

function ownPanel(): PinnedPanel {
  return {
    kind: "session-detail",
    sessionId: initialSessionId,
    cwd: initialCwd,
    pid: initialPid,
  };
}

const pinned = ref(hasInitial ? isPanelPinned(ownPanel()) : false);

/** Whether the chat pane is open (Claude SDK side-car running). */
const chatActive = ref(false);

let listening = false;

function togglePin() {
  if (!hasInitial) return;
  pinned.value = !pinned.value;
  setPanelPinned(ownPanel(), pinned.value);
}

async function startListening() {
  // First call wires up window-level listeners (idempotent thereafter).
  // Initial fetch must still run on every call so a re-mount picks up data.
  if (!listening) {
    listening = true;

    if (isTauri) {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      rememberWindowFocus(getCurrentWindow().label);
    }

    window.addEventListener("blur", async () => {
      // Don't auto-hide when chat is active — user is interacting
      if (!pinned.value && !chatActive.value) {
        if (isTauri) {
          const { getCurrentWindow } = await import("@tauri-apps/api/window");
          await getCurrentWindow().hide();
        }
      }
    });
  }

  if (hasInitial && !detail.value) {
    fetchDetail(initialSessionId, initialCwd, initialPid);
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

function openChat() {
  if (!detail.value) return;
  error.value = null;
  chatActive.value = true;
}

async function closeChat() {
  if (!detail.value) return;
  await stopClaudeSession(detail.value.sessionId).catch(() => {});
  chatActive.value = false;
}

function onChatClosed() {
  chatActive.value = false;
  if (detail.value) {
    fetchDetail(detail.value.sessionId, detail.value.cwd, detail.value.pid);
  }
}

function onChatError(msg: string) {
  error.value = msg;
  chatActive.value = false;
}

async function kill() {
  if (!detail.value) return;
  const pid = detail.value.pid;
  try {
    // Tear down the chat sidecar (and its PTY drain + tail threads) before the
    // window is destroyed. Otherwise the window goes away mid-IPC and the
    // ChatPane's onUnmounted cleanup races destroy() — leaking the sidecar.
    if (chatActive.value) await closeChat();

    await killSession(pid);

    // PID is gone — drop the pinned descriptor since it would no longer
    // resolve to a real session on next restore.
    if (pinned.value) togglePin();
    detail.value = null;

    if (isTauri) {
      const { emit } = await import("@tauri-apps/api/event");
      await emit("session-killed", { pid });
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().destroy();
    }
  } catch (e) {
    error.value = String(e);
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
  return {
    detail,
    loading,
    error,
    pinned,
    chatActive,
    kill,
    openCwd,
    copyInfo,
    fetchDetail,
    togglePin,
    openChat,
    closeChat,
    onChatClosed,
    onChatError,
  };
}
