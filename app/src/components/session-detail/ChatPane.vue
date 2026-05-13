<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick, watch, computed } from "vue";
import {
  startClaudeSession,
  sendClaudeMessage,
  stopClaudeSession,
  onClaudeEvent,
  onClaudeSessionClosed,
  type ClaudeEvent,
  type ClaudeEventEnvelope,
} from "@/lib/tauri";
import { readBool, writeBool } from "@/lib/panelStorage";

const props = defineProps<{ sessionId: string; cwd: string }>();
const emit = defineEmits<{ closed: []; error: [msg: string] }>();

interface ChatTextBlock {
  kind: "text";
  text: string;
}
interface ChatToolBlock {
  kind: "tool";
  id: string;
  name: string;
  input: unknown;
  expanded: boolean;
}
type ChatBlock = ChatTextBlock | ChatToolBlock;

interface ChatMessage {
  id: string;
  role: "user" | "assistant" | "system" | "stderr";
  blocks: ChatBlock[];
  /** Inline status under the message (e.g. token counts, exit code). */
  meta?: string;
  /** Raw event payload for diagnostics. When present, copy emits full JSON instead of the (possibly truncated) rendered text. */
  raw?: unknown;
  /**
   * If true, this bubble is diagnostic noise (raw stdout lines, heartbeats,
   * lifecycle plumbing like "spawned"/"reader_started"). Hidden from the
   * default view and revealed only when the Debug toggle is on. We still
   * append these messages so coalescing logic (e.g. consecutive heartbeats)
   * keeps working — we just filter at render time.
   */
  debug?: boolean;
}

const messages = ref<ChatMessage[]>([]);
const input = ref("");
const sending = ref(false);
const scrollerRef = ref<HTMLElement | null>(null);
const inputRef = ref<HTMLTextAreaElement | null>(null);
let stickToBottom = true;
let unlistenEvent: (() => void) | null = null;
let unlistenClosed: (() => void) | null = null;
let nextMsgId = 1;

/**
 * "Debug" view toggle — when off (default), diagnostic bubbles (raw stdout,
 * heartbeats, spawn/reader lifecycle plumbing) are hidden. Toggle persists in
 * localStorage so it survives across panel opens.
 */
const DEBUG_KEY = "fnba-utils:chat-debug";
const showDebug = ref(readBool(DEBUG_KEY));
watch(showDebug, (v) => writeBool(DEBUG_KEY, v));

const visibleMessages = computed(() =>
  showDebug.value ? messages.value : messages.value.filter((m) => !m.debug),
);

function newId(): string {
  return `m${nextMsgId++}`;
}

function appendMessage(msg: ChatMessage) {
  messages.value.push(msg);
}

/** Convert a Claude stream-json event into chat-friendly blocks; ignore noise. */
function handleEvent(envelope: ClaudeEventEnvelope) {
  if (envelope.sessionId !== props.sessionId) return;
  const ev = envelope.event;

  switch (ev.type) {
    case "system": {
      const sysEv = ev as Record<string, unknown> & { subtype?: string };
      const sub = sysEv.subtype ?? "system";

      // Collapse consecutive api_retry events into one updating bubble.
      const last = messages.value[messages.value.length - 1];
      const lastIsCollapsible =
        last &&
        last.role === "system" &&
        last.blocks.length === 1 &&
        last.blocks[0].kind === "text" &&
        sub === "api_retry" &&
        last.blocks[0].text.startsWith("api_retry ");

      let text: string;
      let stashRaw = false;
      let isDebug = false;
      switch (sub) {
        case "init":
          text = "Session ready";
          stashRaw = true;
          isDebug = true;
          break;
        case "api_retry": {
          // claude's error classifier returns "unknown" for network-level failures;
          // dumping the raw fields is the only way to see what actually happened.
          const { type: _t, subtype: _s, session_id: _sid, uuid: _u, ...rest } = sysEv;
          text = `api_retry ${JSON.stringify(rest).slice(0, 500)}`;
          stashRaw = true;
          break;
        }
        default:
          text = `system: ${sub} ${JSON.stringify(sysEv).slice(0, 300)}`;
          stashRaw = true;
      }

      const rawForCopy: unknown = stashRaw ? sysEv : undefined;

      if (lastIsCollapsible && last.blocks[0].kind === "text") {
        last.blocks[0].text = text;
        last.raw = rawForCopy;
        return;
      }

      appendMessage({
        id: newId(),
        role: "system",
        blocks: [{ kind: "text", text }],
        raw: rawForCopy,
        debug: isDebug,
      });
      return;
    }
    case "assistant": {
      const msg = (ev as Extract<ClaudeEvent, { type: "assistant" }>).message;
      const blocks: ChatBlock[] = [];
      for (const block of msg.content ?? []) {
        if (block.type === "text") {
          blocks.push({ kind: "text", text: (block as { text: string }).text });
        } else if (block.type === "tool_use") {
          const tu = block as { id: string; name: string; input: unknown };
          blocks.push({
            kind: "tool",
            id: tu.id,
            name: tu.name,
            input: tu.input,
            expanded: false,
          });
        }
      }
      // Reply has started — re-enable input even if more events are still coming.
      sending.value = false;
      if (blocks.length === 0) return;
      const usage = msg.usage;
      const meta = usage
        ? `${usage.input_tokens ?? 0} in / ${usage.output_tokens ?? 0} out`
        : undefined;
      appendMessage({ id: newId(), role: "assistant", blocks, meta });
      return;
    }
    case "user": {
      // Only surface tool_result echoes; the user's own message was already optimistic-rendered.
      const msg = (ev as { message?: { content?: unknown } }).message;
      const content = msg?.content;
      if (Array.isArray(content)) {
        for (const block of content) {
          if (
            block &&
            typeof block === "object" &&
            (block as { type?: string }).type === "tool_result"
          ) {
            const text = String((block as { content?: unknown }).content ?? "");
            if (text) {
              appendMessage({
                id: newId(),
                role: "system",
                blocks: [{ kind: "text", text: `tool result: ${text.slice(0, 400)}` }],
              });
            }
          }
        }
      }
      return;
    }
    case "result": {
      // Always release the input on result, regardless of metadata content.
      sending.value = false;
      const r = ev as { duration_ms?: number; total_cost_usd?: number; subtype?: string };
      // Surface non-success result subtypes (error_during_execution, error_max_turns)
      // so the user sees what happened.
      if (r.subtype && r.subtype !== "success") {
        appendMessage({
          id: newId(),
          role: "stderr",
          blocks: [{ kind: "text", text: `result: ${r.subtype}` }],
        });
      }
      const parts: string[] = [];
      if (typeof r.duration_ms === "number") parts.push(`${(r.duration_ms / 1000).toFixed(1)}s`);
      if (typeof r.total_cost_usd === "number") parts.push(`$${r.total_cost_usd.toFixed(4)}`);
      if (parts.length === 0) return;
      // Attach to last assistant message rather than spawning a new bubble.
      for (let i = messages.value.length - 1; i >= 0; i--) {
        if (messages.value[i].role === "assistant") {
          const existing = messages.value[i].meta;
          messages.value[i].meta = existing
            ? `${existing} · ${parts.join(" · ")}`
            : parts.join(" · ");
          break;
        }
      }
      return;
    }
    case "stderr": {
      const text = (ev as { text: string }).text;
      appendMessage({
        id: newId(),
        role: "stderr",
        blocks: [{ kind: "text", text }],
      });
      return;
    }
    case "raw":
    case "pty": {
      // Diagnostic-only output: `raw` is unparseable stdout/JSONL lines (often
      // multi-line ANTHROPIC_LOG=debug entries arriving one line at a time);
      // `pty` is the verbatim output stream of the parallel claude (the TUI).
      // Both coalesce into a single bubble per "kind" so the Debug view stays
      // readable, and both are hidden by default.
      const kind = ev.type as "raw" | "pty";
      const text = (ev as { text: string }).text;
      const prefix = `${kind}:\n`;
      const lastRaw = messages.value[messages.value.length - 1];
      const isContinuation =
        lastRaw &&
        lastRaw.role === "system" &&
        lastRaw.blocks.length === 1 &&
        lastRaw.blocks[0].kind === "text" &&
        lastRaw.blocks[0].text.startsWith(prefix);
      if (isContinuation && lastRaw.blocks[0].kind === "text") {
        lastRaw.blocks[0].text += text;
        const prevRaw = lastRaw.raw;
        lastRaw.raw = typeof prevRaw === "string" ? prevRaw + text : text;
        return;
      }
      appendMessage({
        id: newId(),
        role: "system",
        blocks: [{ kind: "text", text: `${prefix}${text}` }],
        raw: text,
        debug: true,
      });
      return;
    }
    default: {
      // Render any other unrecognized event type so we don't lose information.
      // Some non-actionable telemetry-style events (rate_limit_event, etc.) get
      // marked debug so they only surface when the Debug toggle is on.
      const debugTypes = new Set(["rate_limit_event"]);
      appendMessage({
        id: newId(),
        role: "system",
        blocks: [
          { kind: "text", text: `${ev.type}: ${JSON.stringify(ev).slice(0, 300)}` },
        ],
        raw: ev,
        debug: debugTypes.has(ev.type),
      });
      return;
    }
  }
}

function onScroll() {
  const el = scrollerRef.value;
  if (!el) return;
  const distanceFromBottom = el.scrollHeight - el.scrollTop - el.clientHeight;
  stickToBottom = distanceFromBottom < 40;
}

watch(
  () => messages.value.length,
  async () => {
    if (!stickToBottom) return;
    await nextTick();
    const el = scrollerRef.value;
    if (el) el.scrollTop = el.scrollHeight;
  },
);

async function send() {
  const text = input.value.trim();
  if (!text || sending.value) return;
  appendMessage({
    id: newId(),
    role: "user",
    blocks: [{ kind: "text", text }],
  });
  input.value = "";
  sending.value = true;
  try {
    await sendClaudeMessage(props.sessionId, text);
  } catch (e) {
    sending.value = false;
    appendMessage({
      id: newId(),
      role: "stderr",
      blocks: [{ kind: "text", text: `send failed: ${String(e)}` }],
    });
  }
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    send();
  }
}

function toggleTool(msg: ChatMessage, idx: number) {
  const block = msg.blocks[idx];
  if (block.kind === "tool") {
    block.expanded = !block.expanded;
  }
}

function formatToolInput(input: unknown): string {
  try {
    return JSON.stringify(input, null, 2);
  } catch {
    return String(input);
  }
}

function messageToText(msg: ChatMessage): string {
  // If we stashed the raw event payload (e.g. truncated diagnostic dumps),
  // prefer the full JSON so the user can see everything.
  if (msg.raw !== undefined) {
    try {
      return JSON.stringify(msg.raw, null, 2);
    } catch {
      /* fall through to rendered text */
    }
  }
  return msg.blocks
    .map((b) => {
      if (b.kind === "text") return b.text;
      return `[${b.name}] ${formatToolInput(b.input)}`;
    })
    .join("\n");
}

const copiedId = ref<string | null>(null);

async function copyMessage(msg: ChatMessage) {
  const text = messageToText(msg);
  try {
    await navigator.clipboard.writeText(text);
    copiedId.value = msg.id;
    setTimeout(() => {
      if (copiedId.value === msg.id) copiedId.value = null;
    }, 1200);
  } catch {
    // Clipboard may be unavailable (denied permission, no secure context).
    // Fall back to a hidden textarea + execCommand which Tauri WebView always supports.
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    try {
      document.execCommand("copy");
      copiedId.value = msg.id;
      setTimeout(() => {
        if (copiedId.value === msg.id) copiedId.value = null;
      }, 1200);
    } catch {
      /* give up silently */
    }
    document.body.removeChild(ta);
  }
}

onMounted(async () => {
  unlistenEvent = await onClaudeEvent(handleEvent);
  unlistenClosed = await onClaudeSessionClosed((ev) => {
    if (ev.sessionId !== props.sessionId) return;
    sending.value = false;
    appendMessage({
      id: newId(),
      role: "system",
      blocks: [{ kind: "text", text: `Session closed (exit ${ev.exitCode})` }],
    });
    emit("closed");
  });

  try {
    await startClaudeSession(props.sessionId, props.cwd);
  } catch (e) {
    emit("error", String(e));
    return;
  }

  await nextTick();
  inputRef.value?.focus();
});

onUnmounted(() => {
  unlistenEvent?.();
  unlistenClosed?.();
  // Best-effort: don't await — caller already invoked close.
  stopClaudeSession(props.sessionId).catch(() => {});
});
</script>

<template>
  <div class="chat-pane">
    <div ref="scrollerRef" class="chat-scroll" @scroll="onScroll">
      <div v-if="visibleMessages.length === 0" class="chat-empty">
        {{ messages.length === 0 ? "Connecting to session…" : "(diagnostic events hidden — toggle Debug to show)" }}
      </div>
      <div
        v-for="msg in visibleMessages"
        :key="msg.id"
        class="chat-msg"
        :class="`chat-msg-${msg.role}`"
      >
        <div class="chat-meta">
          <span class="chat-role">
            {{
              msg.role === "user"
                ? "You"
                : msg.role === "assistant"
                ? "Claude"
                : msg.role === "stderr"
                ? "stderr"
                : "system"
            }}
          </span>
          <span v-if="msg.meta" class="chat-meta-extra">{{ msg.meta }}</span>
          <button
            class="chat-copy-btn"
            :title="copiedId === msg.id ? 'Copied' : 'Copy'"
            @click="copyMessage(msg)"
          >
            {{ copiedId === msg.id ? "✓" : "⧉" }}
          </button>
        </div>
        <template v-for="(block, i) in msg.blocks" :key="i">
          <div v-if="block.kind === 'text'" class="chat-text">{{ block.text }}</div>
          <div v-else class="chat-tool">
            <button class="chat-tool-header" @click="toggleTool(msg, i)">
              <span class="chat-tool-chevron">{{ block.expanded ? "▾" : "▸" }}</span>
              <span class="chat-tool-name">{{ block.name }}</span>
            </button>
            <pre v-if="block.expanded" class="chat-tool-input">{{ formatToolInput(block.input) }}</pre>
          </div>
        </template>
      </div>
    </div>
    <div class="chat-input-bar">
      <textarea
        ref="inputRef"
        v-model="input"
        class="chat-input"
        rows="2"
        :disabled="sending"
        :placeholder="sending ? 'Waiting for reply…' : 'Type a message — Enter to send, Shift+Enter for newline'"
        @keydown="onKeydown"
      />
      <div class="chat-input-actions">
        <button
          class="chat-debug-toggle"
          :class="{ active: showDebug }"
          :title="showDebug ? 'Hide diagnostic events' : 'Show diagnostic events (heartbeats, raw stdout, lifecycle)'"
          @click="showDebug = !showDebug"
        >
          Debug
        </button>
        <button class="chat-send" :disabled="sending || input.trim().length === 0" @click="send">
          Send
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.chat-pane {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.chat-scroll {
  flex: 1;
  overflow-y: auto;
  overflow-x: hidden;
  min-height: 0;
  padding: 8px 14px;
  scrollbar-width: thin;
  scrollbar-color: rgba(255, 255, 255, 0.1) transparent;
}

.chat-empty {
  padding: 24px 0;
  text-align: center;
  font-size: 11px;
  color: var(--text-secondary);
}

.chat-msg {
  margin-bottom: 12px;
  border-left: 3px solid transparent;
  padding: 4px 0 4px 8px;
}

.chat-msg-user {
  border-left-color: var(--accent-blue);
}

.chat-msg-assistant {
  border-left-color: var(--accent-green);
}

.chat-msg-system {
  border-left-color: rgba(255, 255, 255, 0.15);
}

.chat-msg-stderr {
  border-left-color: var(--accent-red);
}

.chat-meta {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 4px;
}

.chat-role {
  font-size: 11px;
  font-weight: 600;
  color: var(--text-primary);
}

.chat-meta-extra {
  font-size: 10px;
  color: var(--text-placeholder);
}

.chat-copy-btn {
  margin-left: auto;
  flex-shrink: 0;
  font-size: 11px;
  line-height: 1;
  padding: 2px 6px;
  border-radius: var(--radius-sm);
  border: 1px solid rgba(255, 255, 255, 0.08);
  background: transparent;
  color: var(--text-placeholder);
  cursor: pointer;
  opacity: 0;
  transition: opacity 0.1s ease, color 0.1s ease, background 0.1s ease;
}

.chat-msg:hover .chat-copy-btn {
  opacity: 1;
}

.chat-copy-btn:hover {
  color: var(--accent-blue);
  background: rgba(96, 165, 250, 0.1);
  border-color: rgba(96, 165, 250, 0.3);
}

.chat-text {
  font-size: 12px;
  color: var(--text-secondary);
  line-height: 1.45;
  white-space: pre-wrap;
  word-break: break-word;
  user-select: text;
  -webkit-user-select: text;
  cursor: text;
}

.chat-tool {
  margin: 4px 0;
  border-radius: var(--radius-sm);
  background: rgba(52, 211, 153, 0.06);
  border: 1px solid rgba(52, 211, 153, 0.2);
  overflow: hidden;
}

.chat-tool-header {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  padding: 4px 8px;
  background: transparent;
  border: 0;
  color: var(--accent-green);
  font-family: var(--font-mono);
  font-size: 11px;
  cursor: pointer;
  text-align: left;
}

.chat-tool-header:hover {
  background: rgba(52, 211, 153, 0.1);
}

.chat-tool-chevron {
  font-size: 10px;
  color: var(--accent-green);
}

.chat-tool-name {
  font-weight: 600;
}

.chat-tool-input {
  margin: 0;
  padding: 6px 8px;
  border-top: 1px solid rgba(52, 211, 153, 0.2);
  font-family: var(--font-mono);
  font-size: 10px;
  color: var(--text-secondary);
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 240px;
  overflow: auto;
}

.chat-input-bar {
  display: flex;
  gap: 8px;
  padding: 8px 14px;
  border-top: 1px solid var(--border-subtle);
  flex-shrink: 0;
}

.chat-input {
  flex: 1;
  resize: none;
  font-family: inherit;
  font-size: 12px;
  padding: 6px 8px;
  border-radius: var(--radius-sm);
  border: 1px solid rgba(255, 255, 255, 0.12);
  background: rgba(255, 255, 255, 0.02);
  color: var(--text-primary);
  line-height: 1.4;
}

.chat-input:focus {
  outline: none;
  border-color: rgba(96, 165, 250, 0.4);
}

.chat-input:disabled {
  opacity: 0.6;
}

.chat-input-actions {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 4px;
  flex-shrink: 0;
}

.chat-send {
  padding: 6px 14px;
  font-size: 11px;
  font-weight: 600;
  border-radius: var(--radius-sm);
  border: 1px solid rgba(96, 165, 250, 0.3);
  background: rgba(96, 165, 250, 0.12);
  color: var(--accent-blue);
  cursor: pointer;
}

.chat-send:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.chat-send:not(:disabled):hover {
  background: rgba(96, 165, 250, 0.2);
}

.chat-debug-toggle {
  padding: 3px 10px;
  font-size: 10px;
  font-weight: 500;
  border-radius: var(--radius-sm);
  border: 1px solid rgba(255, 255, 255, 0.1);
  background: transparent;
  color: var(--text-placeholder);
  cursor: pointer;
  transition: all 0.1s ease;
}

.chat-debug-toggle:hover {
  color: var(--text-secondary);
  border-color: rgba(255, 255, 255, 0.18);
}

.chat-debug-toggle.active {
  color: var(--accent-yellow, #fbbf24);
  border-color: rgba(251, 191, 36, 0.35);
  background: rgba(251, 191, 36, 0.08);
}
</style>
