<script setup lang="ts">
import { ref, onMounted, onUnmounted } from "vue";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { pickupSession, writePty, resizePty, onPtyData, onPtyClosed } from "@/lib/tauri";
import type { PtyDataEvent, PtyClosedEvent } from "@/lib/tauri";

const props = defineProps<{
  sessionId: string;
  cwd: string;
  pid: number;
  name: string | null;
}>();
const emit = defineEmits<{ closed: []; error: [msg: string] }>();

const containerRef = ref<HTMLElement | null>(null);

let terminal: Terminal | null = null;
let fitAddon: FitAddon | null = null;
let unlistenData: (() => void) | null = null;
let unlistenClosed: (() => void) | null = null;
let resizeObserver: ResizeObserver | null = null;

onMounted(async () => {
  if (!containerRef.value) return;

  terminal = new Terminal({
    fontSize: 13,
    fontFamily: "'Cascadia Code', 'JetBrains Mono', 'Fira Code', monospace",
    theme: {
      background: "#1a1a2e",
      foreground: "#e2e8f0",
      cursor: "#60a5fa",
      selectionBackground: "#233554",
      black: "#1a1a2e",
      red: "#f87171",
      green: "#34d399",
      yellow: "#fbbf24",
      blue: "#60a5fa",
      magenta: "#c084fc",
      cyan: "#22d3ee",
      white: "#e2e8f0",
      brightBlack: "#4a5568",
      brightRed: "#fca5a5",
      brightGreen: "#6ee7b7",
      brightYellow: "#fde68a",
      brightBlue: "#93c5fd",
      brightMagenta: "#d8b4fe",
      brightCyan: "#67e8f9",
      brightWhite: "#f8fafc",
    },
    cursorBlink: true,
    scrollback: 5000,
    allowProposedApi: true,
  });

  fitAddon = new FitAddon();
  terminal.loadAddon(fitAddon);
  terminal.loadAddon(new WebLinksAddon());

  terminal.open(containerRef.value);
  terminal.focus();

  // Set up event listeners BEFORE calling pickup so we don't miss early output
  unlistenData = await onPtyData((event: PtyDataEvent) => {
    if (event.sessionId !== props.sessionId || !terminal) return;
    const bytes = Uint8Array.from(atob(event.data), (c) => c.charCodeAt(0));
    terminal.write(bytes);
  });

  unlistenClosed = await onPtyClosed((event: PtyClosedEvent) => {
    if (event.sessionId !== props.sessionId) return;
    terminal?.write("\r\n\x1b[2m[Session ended]\x1b[0m\r\n");
    emit("closed");
  });

  // Send user keystrokes to PTY
  terminal.onData((data) => {
    writePty(props.sessionId, data).catch(() => {});
  });

  // Defer first fit one frame so the container has its real layout — otherwise
  // cols/rows fall back to 80×24 and the PTY gets sized wrong.
  await new Promise(requestAnimationFrame);
  fitAddon.fit();

  terminal.write(
    `\x1b[2m[Taking over PID ${props.pid} — killing original session...]\x1b[0m\r\n`,
  );

  // NOW call pickup with the actual terminal dimensions
  try {
    await pickupSession(
      props.sessionId,
      props.cwd,
      props.pid,
      terminal.cols,
      terminal.rows,
      props.name,
    );
  } catch (e) {
    emit("error", String(e));
    return;
  }

  // Resize on container size change
  resizeObserver = new ResizeObserver(() => {
    if (!fitAddon || !terminal) return;
    fitAddon.fit();
    resizePty(props.sessionId, terminal.cols, terminal.rows).catch(() => {});
  });
  resizeObserver.observe(containerRef.value);
});

onUnmounted(() => {
  unlistenData?.();
  unlistenClosed?.();
  resizeObserver?.disconnect();
  terminal?.dispose();
});
</script>

<template>
  <div class="terminal-pane" ref="containerRef" />
</template>

<style>
@import "@xterm/xterm/css/xterm.css";
</style>

<style scoped>
.terminal-pane {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  padding: 4px;
}

.terminal-pane :deep(.xterm) {
  height: 100%;
}

.terminal-pane :deep(.xterm-viewport) {
  scrollbar-width: thin;
  scrollbar-color: rgba(255, 255, 255, 0.1) transparent;
}
</style>
