import { ref, computed, onUnmounted } from "vue";
import {
  getDockerStatus,
  onDockerStatus,
  dockerStart,
  dockerStop,
  dockerRestart,
  dockerLogs,
  listPinnedContainers,
  pinContainer,
  unpinContainer,
  saveDockerWidgetPosition,
  getDockerWidgetAnchorBottom,
  onDisplayChanged,
  copyText,
  runInTerminal,
  type DockerStatusPayload,
  type DockerContainer,
} from "@/lib/tauri";
import { openExternal } from "@/lib/external";

// Module-level shared state (single instance per window).
const status = ref<DockerStatusPayload | null>(null);
const pinnedNames = ref<string[]>([]);
const expanded = ref(false);
const pending = ref<Set<string>>(new Set());

// Logs popover state
const logsFor = ref<string | null>(null);
const logsText = ref("");

// --- Derived ---
const containers = computed(() => status.value?.containers ?? []);
const pinned = computed(() => containers.value.filter((c) => c.pinned));
const engineUp = computed(() => status.value?.engineUp ?? false);
const overall = computed(() => status.value?.overall ?? "red");
const runningCount = computed(() => status.value?.runningCount ?? 0);
const totalCount = computed(() => status.value?.totalCount ?? 0);
const error = computed(() => status.value?.error ?? null);

// --- Helpers ---
function clonePending(): Set<string> {
  return new Set(pending.value);
}

// --- Actions ---
async function start(id: string): Promise<void> {
  const next = clonePending();
  next.add(id);
  pending.value = next;
  try {
    await dockerStart(id);
    await getDockerStatus().then((p) => { status.value = p; });
  } finally {
    const after = clonePending();
    after.delete(id);
    pending.value = after;
  }
}

async function stop(id: string): Promise<void> {
  const next = clonePending();
  next.add(id);
  pending.value = next;
  try {
    await dockerStop(id);
    await getDockerStatus().then((p) => { status.value = p; });
  } finally {
    const after = clonePending();
    after.delete(id);
    pending.value = after;
  }
}

async function restart(id: string): Promise<void> {
  const next = clonePending();
  next.add(id);
  pending.value = next;
  try {
    await dockerRestart(id);
    await getDockerStatus().then((p) => { status.value = p; });
  } finally {
    const after = clonePending();
    after.delete(id);
    pending.value = after;
  }
}

async function togglePin(name: string): Promise<void> {
  const isPinned = pinnedNames.value.includes(name);
  if (isPinned) {
    await unpinContainer(name);
  } else {
    await pinContainer(name);
  }
  pinnedNames.value = await listPinnedContainers();
  status.value = await getDockerStatus();
}

async function openLogs(id: string): Promise<void> {
  logsFor.value = id;
  logsText.value = "";
  try {
    logsText.value = await dockerLogs(id, 200);
  } catch (e) {
    logsText.value = `Error fetching logs: ${e}`;
  }
}

function closeLogs(): void {
  logsFor.value = null;
  logsText.value = "";
}

async function copyLogs(): Promise<void> {
  await copyText(logsText.value);
}

async function execShell(id: string): Promise<void> {
  await runInTerminal(
    `docker exec -it ${id} sh -c 'command -v bash >/dev/null && exec bash || exec sh'`,
  );
}

/**
 * Open a published port for a container in the browser.
 * Returns the list of ports if there are multiple so the UI can show a menu.
 */
function openPort(c: DockerContainer): Array<{ hostPort: number }> | null {
  const published = c.ports.filter((p) => p.hostPort != null) as Array<{
    hostIp: string | null;
    hostPort: number;
    containerPort: number;
    protocol: string;
  }>;
  if (published.length === 0) return null;
  if (published.length === 1) {
    void openExternal(`http://localhost:${published[0].hostPort}`);
    return null;
  }
  return published.map((p) => ({ hostPort: p.hostPort }));
}

// -----------------------------------------------------------------------
// Size-to-content with bottom anchor
//
// The widget sits above the taskbar, so it should grow UPWARD. We keep track
// of the bottom-left anchor in logical coords (updated on user drag) and
// reposition after every programmatic resize to hold the bottom edge fixed.
//
// Physical coords (integers, from Tauri events) ↔ logical coords (divided by
// scaleFactor). We always work in logical for setSize/setPosition.
// -----------------------------------------------------------------------

// Width is fixed at 280 logical px (matches tauri.conf.json).
const LOGICAL_WIDTH = 280;

// Timestamp of the last programmatic move so onMoved can distinguish
// our own moves from user drag events.
let lastProgrammaticMoveAt = -Infinity;

// Pinned bottom edge in PHYSICAL px (primary work area = taskbar top). The
// widget's bottom is anchored here on every resize so it stays flush above the
// taskbar regardless of DPI/taskbar height. Null until fetched / on non-Windows.
// Refreshed on `display-changed` (dock/undock) — a stale anchor pins the widget
// to coordinates on a detached display.
let anchorBottomPhysical: number | null = null;

// Last content height passed to syncSizeToContent, so a display-change resync
// can re-pin the widget without needing a fresh DOM measurement.
let lastContentH = 0;

// Re-entrancy guard: ResizeObserver + watch can both fire for one layout change.
let syncing = false;

// Debounce timer for position persistence.
let positionDebounce: ReturnType<typeof setTimeout> | null = null;

// One-shot "late settle" timer, reset on every `display-changed`. Windows can
// publish the final work area a beat after the last WM burst without emitting a
// further Rust event, so we re-fetch+resync once ~2s after the last event.
let lateResyncTimer: ReturnType<typeof setTimeout> | null = null;
const LATE_RESYNC_MS = 2000;

// Unlisten handles.
let unlistenDockerStatus: (() => void) | null = null;
let unlistenMove: (() => void) | null = null;
let unlistenDisplayChanged: (() => void) | null = null;

/**
 * Resize the Tauri window to fit the measured content height, keeping the
 * BOTTOM edge pinned to the taskbar top so the widget grows upward.
 *
 * `contentH` is the measured offsetHeight of the padded wrapper (.widget-pad).
 */
async function syncSizeToContent(contentH: number): Promise<void> {
  if (syncing) return;
  syncing = true;
  lastContentH = contentH;
  try {
    const { getCurrentWindow, primaryMonitor } = await import("@tauri-apps/api/window");
    const { LogicalSize, LogicalPosition } = await import("@tauri-apps/api/dpi");

    const win = getCurrentWindow();
    const sf = await win.scaleFactor();
    const curPos = (await win.outerPosition()).toLogical(sf);
    const curSize = (await win.outerSize()).toLogical(sf);

    // Bottom edge pinned to the taskbar top (work-area bottom); fall back to the
    // current bottom if the anchor isn't available (non-Windows / dev).
    const bottom =
      anchorBottomPhysical != null
        ? anchorBottomPhysical / sf
        : curPos.y + curSize.height;

    // Clamp max height to the primary monitor so a long list scrolls.
    let maxH = 4000;
    try {
      const mon = await primaryMonitor();
      if (mon) {
        const monTop = mon.position.y / sf;
        maxH = Math.max(80, bottom - monTop - 8); // 8px breathing room at the top
      }
    } catch {
      /* monitor query best-effort */
    }

    // Monitor-based scroll cap for the expanded list (NOT 100vh — that would
    // feed back into the measured height and stall the resize).
    try {
      document.documentElement.style.setProperty(
        "--docker-list-max",
        `${Math.max(120, Math.floor(maxH - 50))}px`,
      );
    } catch {
      /* no DOM (non-browser env) — ignore */
    }

    const desiredH = Math.max(60, Math.min(Math.ceil(contentH), Math.floor(maxH)));
    const newY = Math.round(bottom - desiredH);

    // Bail only when BOTH the height and the vertical anchor are already
    // correct. After a redock the height is usually unchanged but the taskbar
    // anchor (work-area bottom) has moved, so we must still reposition.
    const heightUnchanged = Math.abs(desiredH - curSize.height) < 1;
    const positionUnchanged = Math.abs(newY - Math.round(curPos.y)) < 1;
    if (heightUnchanged && positionUnchanged) return;

    lastProgrammaticMoveAt = performance.now();
    await win.setSize(new LogicalSize(LOGICAL_WIDTH, desiredH));
    await win.setPosition(new LogicalPosition(Math.round(curPos.x), newY));
  } catch {
    // Window API unavailable (e.g., non-Tauri dev env). Ignore — UI renders fine.
  } finally {
    syncing = false;
  }
}

/**
 * Fetch the taskbar-top anchor, but only accept it if it's plausible for the
 * current primary monitor. During a dock/undock Windows can briefly report a
 * work-area bottom from a just-detached display; adopting it would strand the
 * widget off-screen. Returns null (→ caller keeps the last-known-good anchor)
 * when the fetch fails, returns null/none, or lands outside the primary
 * monitor's physical vertical span.
 */
async function fetchPlausibleAnchor(): Promise<number | null> {
  let next: number | null;
  try {
    next = await getDockerWidgetAnchorBottom();
  } catch {
    return null; // fetch failed — keep last-known-good
  }
  if (next == null) return null;
  try {
    const { primaryMonitor } = await import("@tauri-apps/api/window");
    const mon = await primaryMonitor();
    if (mon) {
      // Monitor bounds are physical px, same units as the anchor.
      const monTop = mon.position.y;
      const monBottom = mon.position.y + mon.size.height;
      if (next <= monTop || next > monBottom) {
        return null; // implausible — keep last-known-good
      }
    }
  } catch {
    // Monitor query unavailable (dev/non-Tauri) — accept the fetched value.
  }
  return next;
}

/**
 * Re-fetch the anchor (tolerantly) and re-pin the widget using the last
 * measured content height. Adopts a new anchor only when it's plausible;
 * otherwise the previous anchor is retained.
 */
async function refetchAnchorAndResync(): Promise<void> {
  const next = await fetchPlausibleAnchor();
  if (next != null) anchorBottomPhysical = next; // else keep last-known-good
  if (lastContentH > 0) {
    await syncSizeToContent(lastContentH);
  }
}

export function useDockerWidget() {
  async function init(): Promise<void> {
    // Initial data fetch for first paint.
    try {
      status.value = await getDockerStatus();
    } catch (e) {
      console.warn("useDockerWidget: initial getDockerStatus failed", e);
    }

    // Load pinned names.
    try {
      pinnedNames.value = await listPinnedContainers();
    } catch (e) {
      console.warn("useDockerWidget: listPinnedContainers failed", e);
    }

    // Fetch the taskbar-top anchor so resizes pin the widget flush above it.
    // Tolerant: only adopts a plausible value (else leaves it null → resize
    // falls back to the current bottom).
    const initialAnchor = await fetchPlausibleAnchor();
    if (initialAnchor != null) anchorBottomPhysical = initialAnchor;

    // Subscribe to push events (~3s interval from backend).
    try {
      unlistenDockerStatus = await onDockerStatus((p) => {
        status.value = p;
      });
    } catch (e) {
      console.warn("useDockerWidget: onDockerStatus subscription failed", e);
    }

    // On a dock/undock the primary monitor and taskbar anchor can move; the
    // anchor we fetched above is now stale. Re-fetch it (tolerantly — keep the
    // last-known-good on an implausible/failed read) and re-pin using the last
    // measured content height (Rust also repositions the window, but this keeps
    // our size/anchor math from fighting it on the next natural resize).
    try {
      unlistenDisplayChanged = await onDisplayChanged(async () => {
        await refetchAnchorAndResync();
        // Belt-and-braces: catch a late work-area settle that emits no further
        // Rust event. Re-check once ~2s after the LAST display-changed (the
        // timer is reset per event so a burst collapses to a single late check).
        if (lateResyncTimer) clearTimeout(lateResyncTimer);
        lateResyncTimer = setTimeout(() => {
          lateResyncTimer = null;
          void refetchAnchorAndResync();
        }, LATE_RESYNC_MS);
      });
    } catch (e) {
      console.warn("useDockerWidget: onDisplayChanged subscription failed", e);
    }

    // Wire position persistence on window move. The bottom-anchor is read
    // fresh on every resize (syncSizeToContent), so we don't track it here.
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      const win = getCurrentWindow();
      unlistenMove = await win.onMoved(async ({ payload: pos }) => {
        const now = performance.now();

        // Skip moves we triggered ourselves (resize repositions the window),
        // so we only persist genuine user drags.
        if (now - lastProgrammaticMoveAt < 250) return;

        // Persist the physical position (Rust side stores/restores physical).
        if (positionDebounce) clearTimeout(positionDebounce);
        positionDebounce = setTimeout(async () => {
          positionDebounce = null;
          try {
            await saveDockerWidgetPosition(pos.x, pos.y);
          } catch {
            // best-effort
          }
        }, 500);
      });
    } catch {
      // window may not be available in non-Tauri environments
    }
  }

  onUnmounted(() => {
    if (unlistenDockerStatus) {
      unlistenDockerStatus();
      unlistenDockerStatus = null;
    }
    if (unlistenMove) {
      unlistenMove();
      unlistenMove = null;
    }
    if (unlistenDisplayChanged) {
      unlistenDisplayChanged();
      unlistenDisplayChanged = null;
    }
    if (positionDebounce) {
      clearTimeout(positionDebounce);
      positionDebounce = null;
    }
    if (lateResyncTimer) {
      clearTimeout(lateResyncTimer);
      lateResyncTimer = null;
    }
  });

  return {
    // State
    status,
    pinnedNames,
    expanded,
    pending,
    logsFor,
    logsText,
    // Derived
    containers,
    pinned,
    engineUp,
    overall,
    runningCount,
    totalCount,
    error,
    // Actions
    init,
    start,
    stop,
    restart,
    togglePin,
    openLogs,
    closeLogs,
    copyLogs,
    execShell,
    openPort,
    syncSizeToContent,
  };
}
