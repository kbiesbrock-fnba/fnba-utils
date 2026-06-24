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
let anchorBottomPhysical: number | null = null;

// Re-entrancy guard: ResizeObserver + watch can both fire for one layout change.
let syncing = false;

// Debounce timer for position persistence.
let positionDebounce: ReturnType<typeof setTimeout> | null = null;

// Unlisten handles.
let unlistenDockerStatus: (() => void) | null = null;
let unlistenMove: (() => void) | null = null;

/**
 * Resize the Tauri window to fit the measured content height, keeping the
 * BOTTOM edge pinned to the taskbar top so the widget grows upward.
 *
 * `contentH` is the measured offsetHeight of the padded wrapper (.widget-pad).
 */
async function syncSizeToContent(contentH: number): Promise<void> {
  if (syncing) return;
  syncing = true;
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
    if (Math.abs(desiredH - curSize.height) < 1) return;

    const newY = Math.round(bottom - desiredH);
    lastProgrammaticMoveAt = performance.now();
    await win.setSize(new LogicalSize(LOGICAL_WIDTH, desiredH));
    await win.setPosition(new LogicalPosition(Math.round(curPos.x), newY));
  } catch {
    // Window API unavailable (e.g., non-Tauri dev env). Ignore — UI renders fine.
  } finally {
    syncing = false;
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
    try {
      anchorBottomPhysical = await getDockerWidgetAnchorBottom();
    } catch {
      anchorBottomPhysical = null; // dev/non-Windows — fall back to current bottom
    }

    // Subscribe to push events (~3s interval from backend).
    try {
      unlistenDockerStatus = await onDockerStatus((p) => {
        status.value = p;
      });
    } catch (e) {
      console.warn("useDockerWidget: onDockerStatus subscription failed", e);
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
    if (positionDebounce) {
      clearTimeout(positionDebounce);
      positionDebounce = null;
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
