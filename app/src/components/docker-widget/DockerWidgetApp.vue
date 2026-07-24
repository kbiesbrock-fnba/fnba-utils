<script setup lang="ts">
import { onMounted, computed, ref, onUnmounted, watch, nextTick } from "vue";
import { useDockerWidget } from "@/composables/useDockerWidget";
import DockerContainerRow from "./DockerContainerRow.vue";
import DockerExpandedView from "./DockerExpandedView.vue";
import DockerLogsPopover from "./DockerLogsPopover.vue";
import type { DockerContainer } from "@/lib/tauri";

const {
  engineUp,
  overall,
  runningCount,
  totalCount,
  containers,
  pinnedNames,
  expanded,
  pending,
  logsFor,
  logsText,
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
} = useDockerWidget();

// Measured element is the padded wrapper (.widget-pad), so the height we send
// to the window includes the transparent shadow padding — the full DOM height.
const padEl = ref<HTMLElement | null>(null);

// Window handle for the header's click-vs-drag gesture (grabbed on mount).
let dragWin: { startDragging: () => Promise<void> } | null = null;

// Unlisten handle for the foreground-change ("defocus") event from the backend.
let unlistenDefocus: (() => void) | null = null;

// Three display states:
//   heading-only — not hovering, not expanded (idle)
//   hover        — pointer over the widget → show the pinned set
//   expanded     — heading clicked → show ALL containers grouped
// `hovering` is transient UI; `expanded` is a clicked mode. Both collapse back
// to heading-only when the pointer leaves the widget.
const hovering = ref(false);

function resync(): void {
  if (padEl.value) {
    syncSizeToContent(padEl.value.offsetHeight);
  }
}

// Pointer leaving the widget ends the HOVER (pinned) state only. The expanded
// "show all" view persists — it collapses on focus loss (docker-widget-defocus)
// or a re-click, not on mouse-off. A short grace avoids collapsing on edge skim.
let collapseTimer: ReturnType<typeof setTimeout> | null = null;

function onWidgetLeave(): void {
  if (collapseTimer) clearTimeout(collapseTimer);
  collapseTimer = setTimeout(() => {
    collapseTimer = null;
    hovering.value = false;
  }, 300);
}

function onWidgetEnter(): void {
  if (collapseTimer) {
    clearTimeout(collapseTimer);
    collapseTimer = null;
  }
  hovering.value = true;
}

onUnmounted(() => {
  if (collapseTimer) {
    clearTimeout(collapseTimer);
    collapseTimer = null;
  }
  if (unlistenDefocus) {
    unlistenDefocus();
    unlistenDefocus = null;
  }
});

// The whole title bar is both the drag handle AND the expand/collapse toggle.
// The heading is the window DRAG handle only — expansion is triggered solely by
// the "All containers" bar. A real move past a small threshold starts a window
// drag; a click does nothing (no toggle).
function onHeaderPointerDown(e: MouseEvent): void {
  if (e.button !== 0) return;
  const startX = e.screenX;
  const startY = e.screenY;
  const onMove = (ev: MouseEvent) => {
    if (Math.abs(ev.screenX - startX) > 4 || Math.abs(ev.screenY - startY) > 4) {
      teardown();
      void dragWin?.startDragging().catch(() => {});
    }
  };
  function teardown() {
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", teardown);
  }
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", teardown);
}

onMounted(async () => {
  await init();

  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    dragWin = getCurrentWindow();
  } catch {
    /* non-Tauri env — header click still toggles, just no window drag */
  }

  // Collapse to heading-only when the user switches to another window. The
  // widget is non-focusable, so this comes from a backend foreground watch
  // rather than a DOM blur event (which never fires for a non-focusable window).
  try {
    const { listen } = await import("@tauri-apps/api/event");
    unlistenDefocus = await listen("docker-widget-defocus", () => {
      hovering.value = false;
      expanded.value = false;
    });
  } catch {
    /* non-Tauri env */
  }

  // Wire ResizeObserver to drive size-to-content after every layout change.
  if (padEl.value) {
    const ro = new ResizeObserver(() => {
      requestAnimationFrame(resync);
    });
    ro.observe(padEl.value);
    onUnmounted(() => ro.disconnect());
  }

  // Belt-and-suspenders: re-sync the window size right after a state change
  // (heading ↔ hover ↔ show-all) alters the DOM, in case the observer coalesces.
  watch([expanded, hovering], async () => {
    await nextTick();
    resync();
  });

  // Size + pin the idle heading on first paint (the observer's initial tick can
  // land before the anchor is fetched).
  await nextTick();
  resync();
});

const pinnedCount = computed(() => pinnedNames.value.length);

// --- Pinned-set health (left heading dot) ---
//   red   — a pinned container has an actual error (unhealthy / restart loop / dead)
//   amber — error-free but at least one pinned isn't running (stopped / removed)
//   green — all pinned containers running & healthy
//   grey  — engine down, or nothing pinned
const pinnedHealth = computed<"green" | "amber" | "red" | "grey">(() => {
  if (!engineUp.value || pinnedCount.value === 0) return "grey";
  const pins = containers.value.filter((c) => c.pinned);
  if (pins.some((c) => c.health === "unhealthy" || c.restartLoop || c.state === "dead")) {
    return "red";
  }
  const anyStopped = pins.some((c) => c.state !== "running");
  const anyAbsent = pinnedNames.value.some(
    (n) => !containers.value.some((c) => c.name === n),
  );
  return anyStopped || anyAbsent ? "amber" : "green";
});

const pinnedDotClass = computed(() => `status-dot dot-${pinnedHealth.value}`);

const pinnedDotTitle = computed(() => {
  if (!engineUp.value) return "Docker not running";
  if (pinnedCount.value === 0) return "No pinned containers";
  const pins = containers.value.filter((c) => c.pinned);
  switch (pinnedHealth.value) {
    case "red":
      return "Pinned container unhealthy";
    case "amber": {
      const stopped =
        pins.filter((c) => c.state !== "running").length +
        pinnedNames.value.filter((n) => !containers.value.some((c) => c.name === n)).length;
      return `${stopped} of ${pinnedCount.value} pinned not running`;
    }
    default:
      return "All pinned running";
  }
});

// --- Overall health across ALL containers (right heading dot) ---
const overallDotClass = computed(
  () => `status-dot dot-${engineUp.value ? overall.value : "grey"}`,
);

const overallDotTitle = computed(() => {
  if (!engineUp.value) return "Docker not running";
  switch (overall.value) {
    case "red":
      return "A container is unhealthy";
    case "amber": {
      const stopped = totalCount.value - runningCount.value;
      return stopped > 0 ? `${stopped} not running` : "Starting";
    }
    default:
      return "All containers running";
  }
});

// Collapsed/minimal view: ONLY the curated pinned set, so it never grows with
// the running stack. Pinned-but-stopped containers stay visible (running first).
const collapsedContainers = computed<DockerContainer[]>(() => {
  const pinned = containers.value.filter((c) => c.pinned);
  const run    = pinned.filter((c) => c.state === "running");
  const stop   = pinned.filter((c) => c.state !== "running");
  return [...run, ...stop];
});

// Pinned names with no live container (pinned, then removed — e.g. compose down)
// surface as greyed "absent" rows so a watched service that's gone is still seen.
const absentPinnedNames = computed<string[]>(() =>
  pinnedNames.value.filter((n) => !containers.value.some((c) => c.name === n)),
);

// Logs popover title.
const logsTitle = computed(() => {
  if (!logsFor.value) return "";
  const c = containers.value.find((c) => c.id === logsFor.value);
  return c ? c.name : logsFor.value;
});

function handleOpenPort(c: DockerContainer): void {
  openPort(c);
}
</script>

<template>
  <!--
    .widget-pad: transparent padding wrapper so the CSS drop-shadow isn't
    clipped by the window edge. ResizeObserver measures .widget-root inside.
  -->
  <!--
    .widget-viewport fills the window and bottom-aligns the card, so the widget
    stays flush above the taskbar even if the OS won't let the window shrink all
    the way to the content height. The measured element is still .widget-pad
    (content height), so size-to-content is unaffected.
  -->
  <div class="widget-viewport">
    <div
      ref="padEl"
      class="widget-pad"
      @mouseenter="onWidgetEnter"
      @mouseleave="onWidgetLeave"
    >
      <div class="widget-root">
      <!-- Header (always shown): pinned summary | running/total summary.
           Hover grows to the pinned set; click grows to all (disambiguated
           from drag in onHeaderPointerDown). -->
      <div
        class="title-row"
        :class="{ clickable: engineUp }"
        @mousedown="onHeaderPointerDown"
      >
        <span class="whale-glyph">&#x1F433;</span>

        <!-- Pinned group: health dot + pin count -->
        <span class="stat-group" :title="pinnedDotTitle">
          <span class="pin-glyph">&#x1F4CC;</span>
          <span :class="pinnedDotClass"></span>
          <span class="stat-num">{{ pinnedCount }}</span>
        </span>

        <span class="divider">|</span>

        <!-- Running group: overall dot + running / total -->
        <span class="stat-group" :title="overallDotTitle">
          <span :class="overallDotClass"></span>
          <span class="count-display"><span class="count-run">{{ runningCount }}</span><span class="count-sep">/</span><span class="count-total">{{ totalCount }}</span></span>
        </span>

        <div class="title-spacer"></div>
      </div>

      <!-- Engine-down (only once the user looks: on hover or expand) -->
      <div v-if="!engineUp && (hovering || expanded)" class="engine-down">
        <span class="engine-down-text">Docker not running</span>
      </div>

      <!-- Hover: the curated pinned set (running first). -->
      <div v-else-if="engineUp && !expanded && hovering" class="collapsed-section">
        <DockerContainerRow
          v-for="c in collapsedContainers"
          :key="c.id"
          :container="c"
          :pending="pending.has(c.id)"
          @start="start"
          @stop="stop"
          @restart="restart"
          @logs="openLogs"
          @exec="execShell"
          @open-port="handleOpenPort"
          @toggle-pin="togglePin"
        />

        <!-- Pinned but no live container (removed) — kept visible, greyed out. -->
        <div
          v-for="name in absentPinnedNames"
          :key="'absent-' + name"
          class="absent-row"
          :title="name + ' — pinned, no running container'"
        >
          <span class="absent-dot"></span>
          <span class="absent-name">{{ name }}</span>
          <button class="absent-unpin" title="Unpin" @click="togglePin(name)">&#x2715;</button>
        </div>

        <div
          v-if="collapsedContainers.length === 0 && absentPinnedNames.length === 0"
          class="empty-hint"
        >
          No pinned containers
        </div>

        <!-- Click bar → phase 3 (show all containers). -->
        <button class="expand-bar" title="Show all containers" @click="expanded = true">
          <span class="expand-bar-label">All containers</span>
          <span class="expand-bar-count">{{ totalCount }}</span>
          <span class="expand-bar-chevron">&#x25B4;</span>
        </button>
      </div>

      <!-- Phase 3 (clicked): all containers grouped, with a collapse bar. -->
      <template v-else-if="engineUp && expanded">
        <DockerExpandedView
          :containers="containers"
          :pending="pending"
          @start="start"
          @stop="stop"
          @restart="restart"
          @logs="openLogs"
          @exec="execShell"
          @open-port="handleOpenPort"
          @toggle-pin="togglePin"
        />
        <!-- Collapse bar → back to the pinned view (or heading once the
             pointer leaves). Mirrors the expand bar; chevron points down. -->
        <button class="expand-bar" title="Collapse" @click="expanded = false">
          <span class="expand-bar-label">Collapse</span>
          <span class="expand-bar-chevron">&#x25BE;</span>
        </button>
      </template>

      <!-- Logs popover overlay -->
      <DockerLogsPopover
        v-if="logsFor !== null"
        :text="logsText"
        :title="logsTitle"
        @copy="copyLogs"
        @close="closeLogs"
      />
    </div>
    </div>
  </div>
</template>

<style scoped>
/* Fills the window and bottom-aligns the card so the widget stays flush above
   the taskbar even if the window can't shrink fully to the content height. */
.widget-viewport {
  height: 100vh;
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
}

/* Transparent padding so drop-shadow isn't clipped. Width is always 280px. */
.widget-pad {
  width: 280px;
  padding: 6px;
  box-sizing: border-box;
}

.widget-root {
  position: relative;
  width: 100%;
  background: #1c1c1c;
  border-radius: 6px;
  /* Neutral border — no accent ring, no inset box-shadow. */
  border: 1px solid #2e2e2e;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  font-family: "Monaco", "Menlo", "Ubuntu Mono", monospace;
  /* Neutral drop shadow only — no accent glow. */
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.55);
}

/* === Title row === */
.title-row {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 8px 4px;
  cursor: default;
  user-select: none;
  border-radius: 6px 6px 0 0;
  transition: background 0.12s;
}

/* The heading is the drag handle (not a click target) — default cursor, with a
   subtle hover tint as interactivity feedback. Expansion is via the bar. */
.title-row.clickable {
  cursor: default;
}

.title-row.clickable:hover {
  background: rgba(255, 255, 255, 0.03);
}

.whale-glyph {
  font-size: 13px;
  flex-shrink: 0;
  line-height: 1;
}

/* Heading stat group: [icon/dot] + number. */
.stat-group {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
}

.pin-glyph {
  font-size: 9px;
  line-height: 1;
  opacity: 0.75;
}

.divider {
  color: #3a3a3a;
  font-weight: 400;
}

.stat-num {
  font-size: 13px;
  font-weight: 700;
  color: #ddd;
  font-variant-numeric: tabular-nums;
  letter-spacing: -0.5px;
}

/* Status dot: 8px circle health indicator */
.status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
  display: inline-block;
}

.dot-green { background: #3fb950; }
.dot-amber { background: #d29922; }
.dot-red   { background: #f85149; }
.dot-grey  { background: #6e7681; }

/* Count: neutral color, not accent */
.count-display {
  font-size: 13px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  letter-spacing: -0.5px;
}

.count-run   { color: #ddd; }
.count-sep   { color: #444; font-weight: 400; margin: 0 1px; }
.count-total { color: #666; }

.title-spacer {
  flex: 1;
}

/* Engine-down message */
.engine-down {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 10px 8px 12px;
}

.engine-down-text {
  font-size: 11px;
  color: #6e7681;
}

/* Collapsed section */
.collapsed-section {
  padding: 2px 4px 6px;
  display: flex;
  flex-direction: column;
}

.empty-hint {
  text-align: center;
  color: #484848;
  font-size: 10px;
  padding: 10px 0 8px;
}

/* Click bar at the bottom of the hover view → expands to show all containers. */
.expand-bar {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  margin-top: 4px;
  padding: 5px 8px;
  background: rgba(255, 255, 255, 0.025);
  border: none;
  border-top: 1px solid #2a2a2a;
  border-radius: 0 0 4px 4px;
  color: #8b949e;
  cursor: pointer;
  font-family: inherit;
  font-size: 10px;
  transition: background 0.12s, color 0.12s;
}

.expand-bar:hover {
  background: rgba(255, 255, 255, 0.07);
  color: #e0e0e0;
}

.expand-bar-label {
  flex: 1;
  text-align: left;
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.expand-bar-count {
  font-variant-numeric: tabular-nums;
  color: #6e7681;
}

.expand-bar-chevron {
  font-size: 9px;
  opacity: 0.8;
}

/* Pinned-but-removed rows: same footprint as a minimal container row. */
.absent-row {
  display: flex;
  align-items: center;
  gap: 5px;
  height: 26px;
  padding: 0 6px;
  box-sizing: border-box;
  border-radius: 3px;
}

.absent-row:hover {
  background: #242424;
}

.absent-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  border: 1.5px solid #4a4a4a;
  box-sizing: border-box;
  flex-shrink: 0;
}

.absent-name {
  flex: 1;
  min-width: 0;
  font-size: 12px;
  color: #5a5a5a;
  font-style: italic;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.absent-unpin {
  width: 16px;
  height: 16px;
  padding: 0;
  background: none;
  border: none;
  color: #555;
  cursor: pointer;
  font-size: 9px;
  border-radius: 3px;
  opacity: 0;
  transition: opacity 0.12s, color 0.1s;
  flex-shrink: 0;
}

.absent-row:hover .absent-unpin {
  opacity: 1;
}

.absent-unpin:hover {
  color: #f85149;
}
</style>
