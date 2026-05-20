import { ref, onMounted, onUnmounted, nextTick, type Ref } from "vue";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import {
  startClaudeSession,
  disconnectSession,
  writeSessionPty,
  resizeSessionPty,
  interruptClaudeSession,
  openPathInEditor,
  onClaudeEvent,
  onClaudeSessionClosed,
  type ClaudeEventEnvelope,
} from "@/lib/tauri";
import { notify, isAnyMcWindowFocused } from "@/composables/useNotifications";

/**
 * Regex matching file paths claude commonly emits. Conservative — requires
 * either an absolute prefix (`/`, `~/`, `./`, `../`, `<drive>:\`, `<drive>:/`)
 * or starts inside `/mnt/<drive>/`, so prose words with slashes (HTTP-less)
 * don't match. Optional `:LINE` or `:LINE:COL` suffix for editor jumps.
 */
const PATH_REGEX =
  /(?:\/mnt\/[a-zA-Z]\/[\w./\-]+|\/(?:home|usr|opt|var|tmp|root|etc)\/[\w./\-]+|~\/[\w./\-]+|\.{1,2}\/[\w./\-]+|[A-Za-z]:[\\/][\w.\\/\-]+)(?::\d+(?::\d+)?)?/g;

/**
 * Lifecycle wrapper around an xterm.js terminal hooked up to a tmux-backed
 * claude session. Owns:
 *   - xterm + FitAddon construction / disposal
 *   - ResizeObserver → PTY resize sync
 *   - claude-event subscription (writes PTY bytes to the terminal; surfaces
 *     trust-warning / permission-prompt as banners / toasts)
 *   - startup state machine + 5 s watchdog
 *   - PTY write per keystroke
 *   - Disconnect on unmount (keeps tmux alive — explicit Kill is separate)
 *
 * The caller (ChatPane) supplies the container ref + onClosed/onError emit
 * hooks. Returns refs for the banner state and the imperative `interrupt` /
 * `retryStart` actions.
 */

export type TerminalStartupState = "connecting" | "ready" | "stalled" | "error";

export interface UseTerminalOptions {
  sessionId: string;
  cwd: string;
  container: Ref<HTMLElement | null>;
  onClosed?: () => void;
  onError?: (msg: string) => void;
}

const TERMINAL_FONT_FAMILY =
  'Cascadia Code, Cascadia Mono, Consolas, "Courier New", monospace';
const STARTUP_WATCHDOG_MS = 5000;

export function useTerminal(opts: UseTerminalOptions) {
  const trustWarning = ref<string | null>(null);
  const startupState = ref<TerminalStartupState>("connecting");

  let term: Terminal | null = null;
  let fitAddon: FitAddon | null = null;
  let unlistenEvent: (() => void) | null = null;
  let unlistenClosed: (() => void) | null = null;
  let resizeObserver: ResizeObserver | null = null;
  let startupWatchdog: ReturnType<typeof setTimeout> | null = null;

  function arriveReady() {
    if (
      startupState.value === "connecting" ||
      startupState.value === "stalled"
    ) {
      startupState.value = "ready";
      if (startupWatchdog !== null) {
        clearTimeout(startupWatchdog);
        startupWatchdog = null;
      }
    }
  }

  function handleEvent(envelope: ClaudeEventEnvelope) {
    if (envelope.sessionId !== opts.sessionId) return;
    arriveReady();
    const ev = envelope.event;

    switch (ev.type) {
      case "pty": {
        term?.write((ev as { text: string }).text);
        return;
      }
      case "raw": {
        // Malformed JSONL — surface as dim terminal text so the user knows.
        term?.write(`\x1b[2m${(ev as { text: string }).text}\x1b[0m\r\n`);
        return;
      }
      case "system": {
        const sub = (ev as { subtype?: string }).subtype;
        if (sub === "trust-warning") {
          trustWarning.value = String(
            (ev as { text?: string }).text ??
              "Workspace trust not pre-accepted.",
          );
        } else if (sub === "permission-prompt") {
          // Ambient nudge for "claude is waiting and you're not looking."
          // The prompt itself is already in the terminal.
          isAnyMcWindowFocused().then((focused) => {
            if (!focused) {
              notify({
                title: "Claude is waiting",
                body: "A decision prompt needs your attention.",
              });
            }
          });
        }
        return;
      }
      // assistant / user / result / stderr come from the JSONL tail and are
      // already rendered into the PTY stream by claude itself. Stats panel
      // reads them separately via get_session_detail.
      default:
        return;
    }
  }

  async function interrupt() {
    try {
      await interruptClaudeSession(opts.sessionId);
    } catch (e) {
      console.warn("[terminal] interrupt failed", e);
    }
  }

  async function retryStart() {
    startupState.value = "connecting";
    if (startupWatchdog !== null) clearTimeout(startupWatchdog);
    try {
      await startClaudeSession(opts.sessionId, opts.cwd);
    } catch (e) {
      startupState.value = "error";
      opts.onError?.(String(e));
      return;
    }
    startupWatchdog = setTimeout(() => {
      if (startupState.value === "connecting") startupState.value = "stalled";
    }, STARTUP_WATCHDOG_MS);
  }

  async function pushSize() {
    if (!term || !fitAddon) return;
    fitAddon.fit();
    const { cols, rows } = term;
    try {
      await resizeSessionPty(opts.sessionId, cols, rows);
    } catch (e) {
      console.warn("[terminal] resize failed", e);
    }
  }

  onMounted(async () => {
    await nextTick();
    const el = opts.container.value;
    if (!el) return;

    term = new Terminal({
      fontFamily: TERMINAL_FONT_FAMILY,
      fontSize: 12,
      lineHeight: 1.15,
      cursorBlink: true,
      cursorStyle: "block",
      scrollback: 5000,
      allowProposedApi: true,
      theme: {
        // Match the panel's dark backdrop — xterm's default black looks like
        // a hole against the subtle window border.
        background: "#0b1116",
        foreground: "#e6edf3",
        cursor: "#60a5fa",
        cursorAccent: "#0b1116",
        selectionBackground: "rgba(96, 165, 250, 0.35)",
      },
    });
    fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.open(el);

    // File-path link provider: scan each line for path-like tokens; on click
    // hand the matched text to the backend, which translates WSL→Windows and
    // launches IntelliJ (or Explorer fallback).
    term.registerLinkProvider({
      provideLinks(bufferLineNumber, callback) {
        if (!term) return callback(undefined);
        const lineBuf = term.buffer.active.getLine(bufferLineNumber - 1);
        if (!lineBuf) return callback(undefined);
        const lineText = lineBuf.translateToString(true);
        if (!lineText) return callback(undefined);

        const links: Array<{
          range: { start: { x: number; y: number }; end: { x: number; y: number } };
          text: string;
          activate: (e: MouseEvent, text: string) => void;
        }> = [];
        PATH_REGEX.lastIndex = 0;
        let match: RegExpExecArray | null;
        while ((match = PATH_REGEX.exec(lineText)) !== null) {
          const start = match.index + 1;
          const end = start + match[0].length - 1;
          links.push({
            range: {
              start: { x: start, y: bufferLineNumber },
              end: { x: end, y: bufferLineNumber },
            },
            text: match[0],
            activate: (_e, text) => {
              openPathInEditor(text).catch((err) =>
                console.warn("[terminal] open path failed", err),
              );
            },
          });
        }
        callback(links);
      },
    });

    term.onData((data) => {
      writeSessionPty(opts.sessionId, data).catch((e) =>
        console.warn("[terminal] PTY write failed", e),
      );
    });

    resizeObserver = new ResizeObserver(() => {
      pushSize();
    });
    resizeObserver.observe(el);

    // Wire listeners BEFORE startClaudeSession so the replay-pty-buffer
    // event (emitted from the idempotent re-attach path) lands here.
    unlistenEvent = await onClaudeEvent(handleEvent);
    unlistenClosed = await onClaudeSessionClosed((ev) => {
      if (ev.sessionId !== opts.sessionId) return;
      term?.write(
        `\r\n\x1b[33m[session closed (exit ${ev.exitCode})]\x1b[0m\r\n`,
      );
      opts.onClosed?.();
    });

    try {
      await startClaudeSession(opts.sessionId, opts.cwd);
    } catch (e) {
      startupState.value = "error";
      opts.onError?.(String(e));
      return;
    }

    await pushSize();

    startupWatchdog = setTimeout(() => {
      if (startupState.value === "connecting") startupState.value = "stalled";
    }, STARTUP_WATCHDOG_MS);

    term.focus();
  });

  onUnmounted(() => {
    unlistenEvent?.();
    unlistenClosed?.();
    if (startupWatchdog !== null) clearTimeout(startupWatchdog);
    resizeObserver?.disconnect();
    resizeObserver = null;
    // Disconnect (not stop): tmux + claude keep running for resume.
    disconnectSession(opts.sessionId).catch(() => {});
    term?.dispose();
    term = null;
    fitAddon = null;
  });

  return {
    trustWarning,
    startupState,
    interrupt,
    retryStart,
  };
}
