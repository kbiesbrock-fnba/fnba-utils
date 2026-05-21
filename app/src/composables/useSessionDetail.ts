import { ref } from "vue";
import {
  getSessionDetail,
  killSession,
  stopClaudeSession,
  openInExplorer,
  isTauri,
  isTmuxSessionId,
  tmuxNameFromSessionId,
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
// sessionId is the only required param; pid/cwd are nice-to-have hints for
// pin restoration and the legacy kill flow. `portable_pty::Child::process_id`
// returns Option<u32> and is often None on Windows, so we'd otherwise reject
// freshly-launched sessions whose URL carries pid=0.
const hasInitial = !!initialSessionId;

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

    // Terminal is always live in the panel; we don't auto-hide on blur
    // anymore. Pin still has its original sticky-panel semantic.
  }

  if (hasInitial && !detail.value) {
    await fetchDetail(initialSessionId);
  }
}

function synthesizeTmuxDetail(sessionId: string): SessionDetail {
  const name = tmuxNameFromSessionId(sessionId);
  return {
    pid: initialPid,
    sessionId,
    cwd: initialCwd || "/",
    startedAt: Date.now(),
    kind: "tmux",
    name,
    entrypoint: "tmux",
    isAlive: true,
    gitBranch: null,
    status: "unknown",
    stats: {
      messageCount: 0,
      userMessageCount: 0,
      assistantMessageCount: 0,
      totalInputTokens: 0,
      totalOutputTokens: 0,
    },
    recentMessages: [],
    subagents: [],
    label: name,
    worktreePath: null,
  };
}

async function fetchDetail(sessionId: string) {
  loading.value = true;
  error.value = null;
  try {
    // External tmux attaches don't have an MC-managed JSONL, so we synthesize
    // a stripped-down detail locally instead of calling the backend (which
    // would error: "not tracked by Mission Control"). The terminal pane still
    // attaches via attachTmuxSession and works normally.
    if (isTmuxSessionId(sessionId)) {
      detail.value = synthesizeTmuxDetail(sessionId);
    } else {
      detail.value = await getSessionDetail(sessionId);
    }
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

async function onChatClosed() {
  if (detail.value) {
    return fetchDetail(detail.value.sessionId);
  }
}

function onChatError(msg: string) {
  error.value = msg;
}

async function kill() {
  if (!detail.value) return;
  const pid = detail.value.pid;
  const sid = detail.value.sessionId;
  try {
    // External tmux sessions belong to whoever spawned them (typically an
    // IntelliJ terminal). We never tear those down from MC — just close the
    // window. The user can kill the tmux session from the owning terminal.
    if (!isTmuxSessionId(sid)) {
      // Tear down the chat sidecar (PTY + tmux + state) BEFORE destroying
      // the window. Otherwise the window goes away mid-IPC and the
      // ChatPane's onUnmounted cleanup races destroy().
      await stopClaudeSession(sid).catch(() => {});
      await killSession(pid).catch(() => {});
    }

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
    kill,
    openCwd,
    copyInfo,
    fetchDetail,
    togglePin,
    onChatClosed,
    onChatError,
  };
}
