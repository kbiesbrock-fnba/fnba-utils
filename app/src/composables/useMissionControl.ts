import { computed, ref } from "vue";
import {
  getClaudeSessions,
  getConnectionStatuses,
  hideWindow,
  isTauri,
  type ClaudeSession,
  type ConnectionStatus,
  type SessionSource,
} from "@/lib/tauri";
import {
  readBool,
  writeBool,
  readPinnedPanels,
  readLastFocused,
  rememberWindowFocus,
  type DetailPanelPayload,
  type PanelKind,
  type PinnedPanel,
  type SqlPanelPayload,
} from "@/lib/panelStorage";
import {
  PANEL_DEFAULTS,
  panelKeyFor,
  panelLabelFor,
  panelUrlFor,
  payloadOf,
} from "@/lib/panels";
import { notify, isAnyMcWindowFocused } from "@/composables/useNotifications";

const PINNED_KEY = "fnba-utils:mission-control-pinned";
const CONNECTIONS_COLLAPSED_KEY = "fnba-utils:mc-connections-collapsed";
const CONNECTIONS_HIDE_ERRORS_KEY = "fnba-utils:mc-connections-hide-errors";
const SESSIONS_COLLAPSED_KEY = "fnba-utils:mc-sessions-collapsed";
const SOURCE_FILTER_KEY = "fnba-utils:mc-source-filter";
const POLL_INTERVAL = 3000;
const CONNECTIONS_POLL_INTERVAL = 30000;
const BLUR_SUPPRESS_MS = 300;

/** Chip values for the source filter bar above the session list. */
export type SourceFilter = "all" | "mc" | "claude" | "tmux";

function readFilter(): SourceFilter {
  try {
    const raw = localStorage.getItem(SOURCE_FILTER_KEY);
    if (raw === "mc" || raw === "claude" || raw === "tmux" || raw === "all") {
      return raw;
    }
  } catch {
    /* ignore */
  }
  return "all";
}

function writeFilter(v: SourceFilter) {
  try {
    localStorage.setItem(SOURCE_FILTER_KEY, v);
  } catch {
    /* ignore */
  }
}

function matchesFilter(source: SessionSource, filter: SourceFilter): boolean {
  switch (filter) {
    case "all":
      return true;
    case "mc":
      return source === "mc";
    // The "claude" chip covers both MC-spawned and external claude rows so
    // the user can see "every session where I'm using claude" in one click.
    case "claude":
      return source === "mc" || source === "claude-external";
    case "tmux":
      return source === "tmux";
  }
}

const pinned = ref(readBool(PINNED_KEY));
const sessions = ref<ClaudeSession[]>([]);
const sourceFilter = ref<SourceFilter>(readFilter());
const visibleSessions = computed(() =>
  sessions.value.filter((s) => matchesFilter(s.source, sourceFilter.value)),
);
const loading = ref(true);
const error = ref<string | null>(null);
const selectedPid = ref<number | null>(null);
const expandedPid = ref<number | null>(null);

const connectionStatuses = ref<ConnectionStatus[]>([]);
const connectionsLoading = ref(true);
const connectionsCollapsed = ref(readBool(CONNECTIONS_COLLAPSED_KEY));
// Hide errored connections by default. readBool returns false for a missing
// key, so we invert the storage: persist "show errors" and default to false.
const connectionsHideErrors = ref(!readBool(CONNECTIONS_HIDE_ERRORS_KEY));
const sessionsCollapsed = ref(readBool(SESSIONS_COLLAPSED_KEY));
const sessionsRefreshing = ref(false);

let pollTimer: ReturnType<typeof setInterval> | null = null;
let connectionsPollTimer: ReturnType<typeof setInterval> | null = null;
let initialized = false;
let suppressBlur = false;

/**
 * Last observed status per session, used to detect Busy → Idle transitions
 * for ambient notifications. Lives outside `sessions.value` so mutating it
 * doesn't trigger Vue re-renders.
 */
const lastStatus = new Map<string, string>();

async function fetchSessions(force = false) {
  try {
    const next = await getClaudeSessions(force);
    notifyIdleTransitions(next);
    sessions.value = next;
    error.value = null;
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

function notifyIdleTransitions(next: ClaudeSession[]) {
  const liveIds = new Set<string>();
  for (const s of next) {
    liveIds.add(s.sessionId);
    const prev = lastStatus.get(s.sessionId);
    lastStatus.set(s.sessionId, s.status);
    // First observation: nothing to compare; just record.
    if (prev === undefined) continue;
    if (prev === "busy" && s.status === "idle") {
      const label = s.label ?? s.name ?? s.sessionId.slice(0, 8);
      isAnyMcWindowFocused().then((focused) => {
        if (!focused) {
          notify({
            title: `Claude finished: ${label}`,
            body: s.cwd,
          });
        }
      });
    }
  }
  // Drop dead-session entries so the map doesn't grow unbounded.
  for (const sid of [...lastStatus.keys()]) {
    if (!liveIds.has(sid)) lastStatus.delete(sid);
  }
}

function startPolling() {
  if (pollTimer) return;
  fetchSessions();
  pollTimer = setInterval(fetchSessions, POLL_INTERVAL);
}

async function fetchConnectionStatuses() {
  try {
    connectionStatuses.value = await getConnectionStatuses();
  } catch (e) {
    console.error("Connection status fetch failed:", e);
  } finally {
    connectionsLoading.value = false;
  }
}

function startConnectionsPolling() {
  if (connectionsPollTimer) return;
  fetchConnectionStatuses();
  connectionsPollTimer = setInterval(fetchConnectionStatuses, CONNECTIONS_POLL_INTERVAL);
}

// --- Dynamic side-panel windows (one per opener) ---

async function listSidePanels() {
  const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
  const all = await WebviewWindow.getAll();
  return all.filter(
    (w) =>
      w.label.startsWith("sql-query:") ||
      w.label.startsWith("session-detail:"),
  );
}

async function computePanelPosition(
  win: import("@tauri-apps/api/webviewWindow").WebviewWindow,
) {
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const { PhysicalPosition } = await import("@tauri-apps/api/dpi");

  const mcWin = getCurrentWindow();
  const [mcPos, mcSize, panelSize] = await Promise.all([
    mcWin.outerPosition(),
    mcWin.outerSize(),
    win.outerSize(),
  ]);

  // Bottom-align with MC so taller panels grow upward, not behind taskbar.
  const baseX = mcPos.x + mcSize.width + 8;
  const baseY = mcPos.y + mcSize.height - panelSize.height;

  // Cascade horizontally past any other visible panel.
  const others = (await listSidePanels()).filter((w) => w.label !== win.label);
  const probes = await Promise.all(
    others.map(async (w) => {
      if (!(await w.isVisible())) return null;
      const [wpos, wsize] = await Promise.all([w.outerPosition(), w.outerSize()]);
      return { x: wpos.x, w: wsize.width };
    }),
  );
  const occupied = probes.filter((p): p is { x: number; w: number } => p !== null);

  let x = baseX;
  const step = panelSize.width + 8;
  while (occupied.some((o) => x < o.x + o.w && x + panelSize.width > o.x)) {
    x += step;
  }

  return new PhysicalPosition(x, baseY);
}

async function openOrFocusPanel(
  kind: PanelKind,
  payload: SqlPanelPayload | DetailPanelPayload,
  options: { focus?: boolean } = {},
) {
  if (!isTauri) return;
  const focus = options.focus ?? true;

  const key = panelKeyFor(kind, payload);
  const label = panelLabelFor(kind, key);

  const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
  let win = await WebviewWindow.getByLabel(label);

  suppressBlur = true;
  try {
    if (!win) {
      const url = panelUrlFor(kind, payload);
      win = new WebviewWindow(label, { ...PANEL_DEFAULTS[kind], url });
      // Tauri's `once` auto-unlistens on first fire, so the firing listener
      // is fine — but the OTHER one never fires and leaks for the lifetime
      // of the window. Capture both unlisten handles and clean up the loser.
      // A 5s timeout protects against the case where neither event fires
      // (hung webview init); both listeners get cleaned up regardless.
      let resolved: (() => void) | null = null;
      let rejected: ((e: unknown) => void) | null = null;
      const ready = new Promise<void>((res, rej) => {
        resolved = res;
        rejected = rej;
      });
      const [unCreated, unError] = await Promise.all([
        win.once("tauri://created", () => resolved!()),
        win.once<string>("tauri://error", (e) => rejected!(e.payload)),
      ]);
      let timeoutId: ReturnType<typeof setTimeout> | null = null;
      const timed = new Promise<void>((_, rej) => {
        timeoutId = setTimeout(() => rej(new Error("window create timeout")), 5000);
      });
      try {
        await Promise.race([ready, timed]);
        unError(); // created fired; error listener is still attached
      } catch (e) {
        unCreated();
        unError();
        throw e;
      } finally {
        if (timeoutId) clearTimeout(timeoutId);
      }
    }
    const pos = await computePanelPosition(win);
    await win.setPosition(pos);
    await win.show();
    if (focus) await win.setFocus();
  } catch (e) {
    console.error(`[mc] openOrFocusPanel failed for ${label}`, e);
  }
  setTimeout(() => {
    suppressBlur = false;
  }, BLUR_SUPPRESS_MS);
}

async function hideAllSidePanels() {
  if (!isTauri) return;
  // Honor each panel's own pin state — pinned session-detail / sql-query
  // panels survive MC's blur-hide. The explicit Win+Shift+C dismiss path
  // (in lib.rs) bypasses this and hides everything regardless of pin.
  const pinnedLabels = new Set(
    readPinnedPanels().map((p) =>
      panelLabelFor(p.kind, p.kind === "sql-query" ? p.server : p.sessionId),
    ),
  );
  const panels = await listSidePanels();
  await Promise.all(
    panels.filter((w) => !pinnedLabels.has(w.label)).map((w) => w.hide()),
  );
}

async function restorePinnedSidePanels() {
  if (!isTauri) return;
  const list = readPinnedPanels();
  if (list.length === 0) return;

  suppressBlur = true;
  try {
    for (const panel of list) {
      await openOrFocusPanel(panel.kind, payloadOf(panel), { focus: false });
    }

    const lastFocused = readLastFocused();

    const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
    const { getCurrentWindow } = await import("@tauri-apps/api/window");

    if (lastFocused && lastFocused !== "mission-control") {
      const target = await WebviewWindow.getByLabel(lastFocused);
      if (target && (await target.isVisible())) {
        await target.setFocus();
      } else {
        await getCurrentWindow().setFocus();
      }
    } else {
      await getCurrentWindow().setFocus();
    }
  } catch (e) {
    console.error("[mc] restorePinnedSidePanels failed", e);
  }
  setTimeout(() => {
    suppressBlur = false;
  }, BLUR_SUPPRESS_MS);
}

function toggleSessionExpand(session: ClaudeSession) {
  expandedPid.value = expandedPid.value === session.pid ? null : session.pid;
}

async function openSessionDetail(session: ClaudeSession) {
  selectedPid.value = session.pid;
  await openOrFocusPanel("session-detail", {
    sessionId: session.sessionId,
    cwd: session.cwd,
    pid: session.pid,
  });
}

export function useMissionControl() {
  if (!initialized) {
    initialized = true;
    startPolling();
    startConnectionsPolling();

    window.addEventListener("blur", () => {
      if (pinned.value || suppressBlur) return;
      // A click into an open sub-panel also blurs MC. Defer one tick so
      // OS focus is observable, then bail if any sub-panel now has focus
      // — that blur was internal to the MC group.
      setTimeout(async () => {
        if (isTauri) {
          try {
            const panels = await listSidePanels();
            const focused = await Promise.all(panels.map((w) => w.isFocused()));
            if (focused.some(Boolean)) return;
          } catch {
            // fall through to dismiss
          }
        }
        dismiss();
      }, 50);
    });

    rememberWindowFocus("mission-control");

    if (isTauri) {
      import("@tauri-apps/api/event").then(({ listen }) => {
        listen<{ pid: number }>("session-killed", () => {
          selectedPid.value = null;
          fetchSessions();
        });
        listen("mc-shown", () => {
          restorePinnedSidePanels();
        });
        // MRU hotkey (Super+Shift+N) → backend looks up most-recent project
        // and emits this. We do the actual spawn + window opening here so
        // existing launcher logic (start_new_claude_session +
        // openOrFocusPanel) is reused untouched.
        listen<{ cwd: string; displayName: string }>("mc-mru-launch", async (e) => {
          await launchMru(e.payload.cwd);
        });
      });
    }
  }

  async function launchMru(cwd: string) {
    try {
      const { startNewClaudeSession } = await import("@/lib/tauri");
      const info = await startNewClaudeSession(cwd, null, false);
      await openOrFocusPanel("session-detail", {
        sessionId: info.sessionId,
        cwd: info.cwd,
        pid: info.pid,
      });
      fetchSessions();
    } catch (e) {
      console.error("[mc] MRU launch failed", e);
    }
  }

  function dismiss() {
    hideAllSidePanels();
    hideWindow();
  }

  async function selectConnection(status: ConnectionStatus) {
    await openOrFocusPanel("sql-query", {
      server: status.server,
      label: status.label,
    });
  }

  function togglePin() {
    pinned.value = !pinned.value;
    writeBool(PINNED_KEY, pinned.value);
  }

  function toggleConnectionsCollapsed() {
    connectionsCollapsed.value = !connectionsCollapsed.value;
    writeBool(CONNECTIONS_COLLAPSED_KEY, connectionsCollapsed.value);
  }

  function toggleConnectionsHideErrors() {
    connectionsHideErrors.value = !connectionsHideErrors.value;
    // Persisted value tracks "show errors" so a missing key defaults to hide.
    writeBool(CONNECTIONS_HIDE_ERRORS_KEY, !connectionsHideErrors.value);
  }

  function toggleSessionsCollapsed() {
    sessionsCollapsed.value = !sessionsCollapsed.value;
    writeBool(SESSIONS_COLLAPSED_KEY, sessionsCollapsed.value);
  }

  function refreshConnections() {
    fetchConnectionStatuses();
  }

  async function refreshSessions() {
    if (sessionsRefreshing.value) return;
    sessionsRefreshing.value = true;
    try {
      await fetchSessions(true);
    } finally {
      sessionsRefreshing.value = false;
    }
  }

  function setSourceFilter(v: SourceFilter) {
    sourceFilter.value = v;
    writeFilter(v);
  }

  return {
    pinned,
    sessions,
    visibleSessions,
    sourceFilter,
    setSourceFilter,
    loading,
    error,
    selectedPid,
    expandedPid,
    sessionsCollapsed,
    sessionsRefreshing,
    refreshSessions,
    dismiss,
    togglePin,
    toggleSessionExpand,
    openSessionDetail,
    toggleSessionsCollapsed,
    connectionStatuses,
    connectionsLoading,
    connectionsCollapsed,
    connectionsHideErrors,
    toggleConnectionsCollapsed,
    toggleConnectionsHideErrors,
    refreshConnections,
    selectConnection,
  };
}
