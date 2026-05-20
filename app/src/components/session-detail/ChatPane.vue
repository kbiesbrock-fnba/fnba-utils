<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick } from "vue";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import {
  startClaudeSession,
  disconnectSession,
  writeSessionPty,
  resizeSessionPty,
  interruptClaudeSession,
  onClaudeEvent,
  onClaudeSessionClosed,
  type ClaudeEvent,
  type ClaudeEventEnvelope,
} from "@/lib/tauri";

const props = defineProps<{ sessionId: string; cwd: string }>();
const emit = defineEmits<{ closed: []; error: [msg: string] }>();

/**
 * The chat panel is an embedded xterm.js terminal mirroring the underlying
 * tmux session. Anything claude shows — text, permission prompts, slash-command
 * pickers, the live cursor — is rendered here byte-for-byte. User input goes
 * directly to the PTY via `writeSessionPty`, which means special keys, escape
 * sequences, multi-line paste, and Ctrl-C all behave exactly like a real
 * terminal.
 *
 * No JSONL→bubble translation, no input-disabling state machine. Claude's own
 * TUI is the source of truth for what's on screen.
 */

const termContainer = ref<HTMLDivElement | null>(null);
let term: Terminal | null = null;
let fitAddon: FitAddon | null = null;
let unlistenEvent: (() => void) | null = null;
let unlistenClosed: (() => void) | null = null;
let resizeObserver: ResizeObserver | null = null;

/** Banner state: a few system events still merit a visible notice above the terminal. */
const trustWarning = ref<string | null>(null);
const startupState = ref<"connecting" | "ready" | "stalled" | "error">("connecting");
let startupWatchdog: ReturnType<typeof setTimeout> | null = null;

function handleEvent(envelope: ClaudeEventEnvelope) {
  if (envelope.sessionId !== props.sessionId) return;
  const ev = envelope.event;

  // Any event proves the session is alive.
  if (startupState.value === "connecting" || startupState.value === "stalled") {
    startupState.value = "ready";
    if (startupWatchdog !== null) {
      clearTimeout(startupWatchdog);
      startupWatchdog = null;
    }
  }

  switch (ev.type) {
    case "pty": {
      // The main path: write claude's TUI output straight to the terminal.
      const text = (ev as { text: string }).text;
      term?.write(text);
      return;
    }
    case "raw": {
      // Malformed JSONL — surface as terminal text so it's at least visible.
      const text = (ev as { text: string }).text;
      term?.write(`\x1b[2m${text}\x1b[0m\r\n`);
      return;
    }
    case "system": {
      const sub = (ev as { subtype?: string }).subtype;
      if (sub === "trust-warning") {
        trustWarning.value = String(
          (ev as { text?: string }).text ?? "Workspace trust not pre-accepted.",
        );
      }
      // Other system events (init, api_retry, worktree-cleanup-failed,
      // send-failed) are intentionally swallowed — the terminal will show the
      // user-relevant version of all of those.
      return;
    }
    // assistant / user / result events come from the JSONL tail and are
    // already being rendered by claude itself into the PTY stream. The stats
    // panel still consumes them via get_session_detail's parse_conversation
    // path. Ignore here to avoid double rendering.
    case "assistant":
    case "user":
    case "result":
    case "stderr":
      return;
    default:
      return;
  }
}

async function interrupt() {
  try {
    await interruptClaudeSession(props.sessionId);
  } catch (e) {
    console.warn("[chat] interrupt failed", e);
  }
}

async function retryStart() {
  startupState.value = "connecting";
  if (startupWatchdog !== null) clearTimeout(startupWatchdog);
  try {
    await startClaudeSession(props.sessionId, props.cwd);
  } catch (e) {
    startupState.value = "error";
    emit("error", String(e));
    return;
  }
  startupWatchdog = setTimeout(() => {
    if (startupState.value === "connecting") startupState.value = "stalled";
  }, 5000);
}

/** Push current xterm geometry to the backend PTY. Called on mount and resize. */
async function pushSize() {
  if (!term || !fitAddon) return;
  fitAddon.fit();
  const { cols, rows } = term;
  try {
    await resizeSessionPty(props.sessionId, cols, rows);
  } catch (e) {
    console.warn("[chat] resize failed", e);
  }
}

onMounted(async () => {
  await nextTick();
  if (!termContainer.value) return;

  term = new Terminal({
    fontFamily:
      'Cascadia Code, Cascadia Mono, Consolas, "Courier New", monospace',
    fontSize: 12,
    lineHeight: 1.15,
    cursorBlink: true,
    cursorStyle: "block",
    scrollback: 5000,
    allowProposedApi: true,
    theme: {
      // Match the panel's dark backdrop. The default xterm black is too dark
      // against the subtle window border and looks like a hole.
      background: "#0b1116",
      foreground: "#e6edf3",
      cursor: "#60a5fa",
      cursorAccent: "#0b1116",
      selectionBackground: "rgba(96, 165, 250, 0.35)",
    },
  });
  fitAddon = new FitAddon();
  term.loadAddon(fitAddon);
  term.open(termContainer.value);
  fitAddon.fit();

  // Every keystroke (including Ctrl-C, arrow keys, paste, Enter) gets forwarded
  // as-is. xterm's onData already handles things like bracketed-paste wrapping
  // when the terminal advertises support; claude/tmux will receive the
  // appropriate sequences.
  term.onData((data) => {
    writeSessionPty(props.sessionId, data).catch((e) =>
      console.warn("[chat] PTY write failed", e),
    );
  });

  // Resize: when the container changes size, refit + push new dims to PTY.
  resizeObserver = new ResizeObserver(() => {
    pushSize();
  });
  resizeObserver.observe(termContainer.value);

  // Wire up backend event listeners BEFORE calling startClaudeSession so we
  // don't drop the first pty events that flow when the workers warm up.
  unlistenEvent = await onClaudeEvent(handleEvent);
  unlistenClosed = await onClaudeSessionClosed((ev) => {
    if (ev.sessionId !== props.sessionId) return;
    term?.write(`\r\n\x1b[33m[session closed (exit ${ev.exitCode})]\x1b[0m\r\n`);
    emit("closed");
  });

  // start_claude_session is idempotent — if workers are already running
  // (freshly spawned by start_new_claude_session in this Tauri lifetime),
  // returns Ok with no-op. Otherwise re-attaches to the tmux session.
  try {
    await startClaudeSession(props.sessionId, props.cwd);
  } catch (e) {
    startupState.value = "error";
    emit("error", String(e));
    return;
  }

  // Push initial size to the PTY now that the terminal is open and laid out.
  await pushSize();

  // Watchdog: if no pty bytes arrive in 5s, give the user a Retry option.
  startupWatchdog = setTimeout(() => {
    if (startupState.value === "connecting") startupState.value = "stalled";
  }, 5000);

  term.focus();
});

onUnmounted(() => {
  unlistenEvent?.();
  unlistenClosed?.();
  if (startupWatchdog !== null) clearTimeout(startupWatchdog);
  resizeObserver?.disconnect();
  resizeObserver = null;
  // Window-close path: disconnect our PTY but leave tmux + claude running so
  // the session is still resumable from this panel later (or from any
  // external `tmux attach`). Explicit kill is done via Kill action.
  disconnectSession(props.sessionId).catch(() => {});
  term?.dispose();
  term = null;
  fitAddon = null;
});
</script>

<template>
  <div class="chat-pane">
    <div v-if="trustWarning" class="chat-banner chat-banner-warn">
      <span>{{ trustWarning }}</span>
      <button class="chat-banner-close" @click="trustWarning = null">×</button>
    </div>
    <div v-if="startupState === 'stalled'" class="chat-banner chat-banner-info">
      <span>No output yet. Claude may still be starting.</span>
      <button class="chat-banner-action" @click="retryStart">Retry</button>
    </div>
    <div v-if="startupState === 'connecting'" class="chat-banner chat-banner-info chat-banner-quiet">
      <span>Starting Claude…</span>
    </div>

    <div ref="termContainer" class="chat-term" />

    <div class="chat-toolbar">
      <button class="chat-stop" title="Interrupt the current turn (Ctrl-C)" @click="interrupt">
        Stop
      </button>
    </div>
  </div>
</template>

<style scoped>
.chat-pane {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
  background: #0b1116;
}

.chat-term {
  flex: 1;
  min-height: 0;
  padding: 4px 6px;
  overflow: hidden;
}

.chat-term :deep(.xterm) {
  height: 100%;
}

.chat-term :deep(.xterm-viewport) {
  background: transparent !important;
}

.chat-toolbar {
  display: flex;
  justify-content: flex-end;
  gap: 6px;
  padding: 6px 10px;
  border-top: 1px solid var(--border-subtle);
  flex-shrink: 0;
  background: var(--bg-primary, #0b1116);
}

.chat-stop {
  padding: 4px 12px;
  font-size: 11px;
  font-weight: 600;
  border-radius: var(--radius-sm);
  border: 1px solid rgba(248, 113, 113, 0.4);
  background: rgba(248, 113, 113, 0.12);
  color: var(--accent-red, #f87171);
  cursor: pointer;
}

.chat-stop:hover {
  background: rgba(248, 113, 113, 0.2);
}

.chat-banner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 6px 12px;
  font-size: 11px;
  line-height: 1.4;
  flex-shrink: 0;
}

.chat-banner-warn {
  background: rgba(251, 191, 36, 0.1);
  border-bottom: 1px solid rgba(251, 191, 36, 0.3);
  color: var(--accent-yellow, #fbbf24);
}

.chat-banner-info {
  background: rgba(96, 165, 250, 0.08);
  border-bottom: 1px solid rgba(96, 165, 250, 0.3);
  color: var(--accent-blue, #60a5fa);
}

.chat-banner-quiet {
  background: transparent;
  border-bottom: 1px solid var(--border-subtle);
  color: var(--text-secondary);
}

.chat-banner-close,
.chat-banner-action {
  flex-shrink: 0;
  padding: 2px 8px;
  border: 1px solid currentColor;
  border-radius: var(--radius-sm);
  background: transparent;
  color: inherit;
  font-size: 10px;
  cursor: pointer;
}

.chat-banner-close {
  padding: 0 6px;
  font-size: 14px;
  line-height: 1;
  border: 0;
}
</style>
