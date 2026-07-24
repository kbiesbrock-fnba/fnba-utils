<script setup lang="ts">
import { ref } from "vue";
import { useStandupPanel } from "@/composables/useStandupPanel";
import PinButton from "@/components/common/PinButton.vue";
import StandupHistoryDrawer from "./StandupHistoryDrawer.vue";
import IssueRowDetail from "./IssueRowDetail.vue";
import StandupTaskRow from "./StandupTaskRow.vue";
import { getIssueDetail, type IssueDetail } from "@/lib/tauri";
import { openExternal } from "@/lib/external";

const {
  pinned,
  showCompleted,
  historyOpen,
  loading,
  refreshing,
  error,
  panelState,
  bugs,
  tasks,
  completedCount,
  history,
  lastRun,
  refresh,
  toggleCompleted,
  isCompleted,
  unhideAll,
  resetOrder,
  reorderSection,
  togglePin,
  toggleShowCompleted,
  toggleHistory,
} = useStandupPanel();

// --- Formatting helpers ---

function humanAgo(iso: string): string {
  const t = new Date(iso).getTime();
  if (Number.isNaN(t)) return iso;
  const m = Math.floor((Date.now() - t) / 60_000);
  if (m < 1) return "just now";
  if (m < 60) return `${m}m ago`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h ago`;
  return `${Math.floor(h / 24)}d ago`;
}

// Row-formatting helpers (keyWithPoints, shortType, due/priority) now live in
// StandupTaskRow.vue.

// --- Row expansion (single click) ---

const expandedKey = ref<string | null>(null);
const detailCache = ref<Record<string, IssueDetail>>({});
const detailLoading = ref<Set<string>>(new Set());
const detailError = ref<Record<string, string>>({});

/** Issues whose checklist sub-rows are currently expanded. Default: empty. */
const expandedChecklists = ref<Set<string>>(new Set());

function toggleChecklist(key: string) {
  const next = new Set(expandedChecklists.value);
  if (next.has(key)) next.delete(key);
  else next.add(key);
  expandedChecklists.value = next;
}

async function toggleExpand(key: string) {
  if (expandedKey.value === key) {
    expandedKey.value = null;
    return;
  }
  expandedKey.value = key;
  // Lazy fetch detail for description + spec; cache so re-expanding is instant.
  if (!detailCache.value[key] && !detailLoading.value.has(key)) {
    detailLoading.value.add(key);
    try {
      const d = await getIssueDetail(key);
      detailCache.value = { ...detailCache.value, [key]: d };
      delete detailError.value[key];
    } catch (e) {
      detailError.value = {
        ...detailError.value,
        [key]: e instanceof Error ? e.message : String(e),
      };
    } finally {
      detailLoading.value.delete(key);
    }
  }
}

// --- Open detail panel (double click) ---

async function openDetail(key: string) {
  // localStorage handoff guarantees the detail window picks up the key even if
  // it loaded before the event listener was registered (first-open race).
  try {
    localStorage.setItem("fnba-utils:issue-detail-pending", key);
  } catch {
    // ignore
  }
  const { emit } = await import("@tauri-apps/api/event");
  const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
  const w = await WebviewWindow.getByLabel("issue-detail");
  if (w) {
    // Show first so the webview is alive and listening; then emit.
    await w.show();
    await w.setFocus();
    await emit("issue-detail-open", { key });
  }
}

async function onOpenJira(url: string, e: Event) {
  e.stopPropagation();
  await openExternal(url);
}

// --- Window resize (decorations:false strips OS edge hit zones) ---

type ResizeDir =
  | "North"
  | "South"
  | "East"
  | "West"
  | "NorthWest"
  | "NorthEast"
  | "SouthWest"
  | "SouthEast";

async function startResize(dir: ResizeDir, e: MouseEvent) {
  e.preventDefault();
  e.stopPropagation();
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().startResizeDragging(dir);
}

// --- Drag and drop ---

interface DragState {
  section: "bugs" | "tasks";
  fromIndex: number;
  key: string;
}

const dragState = ref<DragState | null>(null);
const dragOverIndex = ref<number | null>(null);
const dragOverSection = ref<"bugs" | "tasks" | null>(null);

function onDragStart(
  e: DragEvent,
  section: "bugs" | "tasks",
  index: number,
  key: string,
) {
  dragState.value = { section, fromIndex: index, key };
  if (e.dataTransfer) {
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData("text/plain", key);
  }
}

function onDragOver(
  e: DragEvent,
  section: "bugs" | "tasks",
  index: number,
) {
  if (!dragState.value || dragState.value.section !== section) return;
  e.preventDefault();
  if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
  dragOverIndex.value = index;
  dragOverSection.value = section;
}

function onDragLeave() {
  // intentionally empty
}

async function onDrop(
  e: DragEvent,
  section: "bugs" | "tasks",
  toIndex: number,
) {
  e.preventDefault();
  const state = dragState.value;
  dragState.value = null;
  dragOverIndex.value = null;
  dragOverSection.value = null;
  if (!state || state.section !== section) return;
  if (state.fromIndex === toIndex) return;

  const list = section === "bugs" ? bugs.value : tasks.value;
  const newOrder = list.map((i) => i.key);
  const [moved] = newOrder.splice(state.fromIndex, 1);
  newOrder.splice(toIndex, 0, moved);
  await reorderSection(newOrder);
}

function onDragEnd() {
  dragState.value = null;
  dragOverIndex.value = null;
  dragOverSection.value = null;
}
</script>

<template>
  <div class="panel">
    <!-- Resize hit zones (decorations:false hides native OS handles) -->
    <div class="resize-edge resize-n" @mousedown="startResize('North', $event)" />
    <div class="resize-edge resize-s" @mousedown="startResize('South', $event)" />
    <div class="resize-edge resize-e" @mousedown="startResize('East', $event)" />
    <div class="resize-edge resize-w" @mousedown="startResize('West', $event)" />
    <div class="resize-corner resize-nw" @mousedown="startResize('NorthWest', $event)" />
    <div class="resize-corner resize-ne" @mousedown="startResize('NorthEast', $event)" />
    <div class="resize-corner resize-sw" @mousedown="startResize('SouthWest', $event)" />
    <div class="resize-corner resize-se" @mousedown="startResize('SouthEast', $event)" />

    <div class="panel-header">
      <div class="title-group">
        <span class="title">Standup</span>
        <span v-if="lastRun" class="subtitle">{{ humanAgo(lastRun.runAt) }}</span>
      </div>
      <div class="header-actions">
        <button
          class="icon-btn"
          :class="{ active: showCompleted }"
          :title="showCompleted ? `Hide ${completedCount} completed` : `Show ${completedCount} completed`"
          @click="toggleShowCompleted"
        >
          <svg viewBox="0 0 16 16" width="14" height="14" fill="currentColor">
            <path d="M13.854 3.646a.5.5 0 010 .708l-7 7a.5.5 0 01-.708 0l-3.5-3.5a.5.5 0 11.708-.708L6.5 10.293l6.646-6.647a.5.5 0 01.708 0z" />
          </svg>
          <span v-if="completedCount > 0" class="badge">{{ completedCount }}</span>
        </button>
        <button
          class="icon-btn"
          :class="{ active: historyOpen }"
          title="History"
          @click="toggleHistory"
        >
          <svg viewBox="0 0 16 16" width="14" height="14" fill="currentColor">
            <path d="M8 3.5a4.5 4.5 0 100 9 4.5 4.5 0 000-9zM2 8a6 6 0 1112 0A6 6 0 012 8zm6-3a.5.5 0 01.5.5v2.293l1.354 1.353a.5.5 0 11-.708.708l-1.5-1.5A.5.5 0 017.5 8V5.5A.5.5 0 018 5z" />
          </svg>
        </button>
        <button
          class="icon-btn"
          title="Reset manual order"
          @click="resetOrder"
        >
          <svg viewBox="0 0 16 16" width="14" height="14" fill="currentColor">
            <path d="M8 3a5 5 0 105 5h-1.5A3.5 3.5 0 118 4.5V6L4.5 3.25 8 .5V3z" />
          </svg>
        </button>
        <button
          class="icon-btn"
          :disabled="refreshing"
          title="Refresh (pull from Jira)"
          @click="refresh"
        >
          <svg
            viewBox="0 0 16 16"
            width="14"
            height="14"
            fill="currentColor"
            :class="{ spin: refreshing }"
          >
            <path d="M8 3a5 5 0 014.546 2.916l-1.302.434A3.5 3.5 0 108 11.5v1.5a5 5 0 010-10zm5 0v3.5a.5.5 0 01-.5.5H9V5.5h2.379A3.5 3.5 0 008 4.5a3.5 3.5 0 00-3.5 3.5h-1.5A5 5 0 0113 3z" />
          </svg>
        </button>
        <PinButton
          :pinned="pinned"
          :size="24"
          pin-title="Pin (keep open when focus leaves)"
          unpin-title="Unpin (auto-hide on focus loss)"
          @toggle="togglePin"
        />
      </div>
    </div>

    <div v-if="error" class="error-banner">⚠ {{ error }}</div>

    <div v-if="historyOpen" class="history-section">
      <StandupHistoryDrawer :history="history" />
    </div>

    <div v-if="loading && !panelState" class="panel-empty">Loading...</div>
    <div v-else-if="!panelState?.report" class="panel-empty">
      No standup yet. Run from the palette (Win+Shift+F → Standup) to populate.
    </div>
    <div v-else class="panel-body">
      <div v-if="bugs.length === 0 && tasks.length === 0" class="panel-empty">
        <template v-if="completedCount > 0">
          All tasks completed.
          <button class="link-btn" @click="unhideAll">
            Show {{ completedCount }} completed
          </button>
        </template>
        <template v-else>No tasks.</template>
      </div>

      <template v-if="bugs.length > 0">
        <div class="section-header bugs-header">
          <span class="section-emoji">🐞</span>
          <span class="section-label">Bugs</span>
          <span class="section-count">{{ bugs.length }}</span>
        </div>
        <template v-for="(issue, idx) in bugs" :key="issue.key">
          <StandupTaskRow
            :issue="issue"
            :index="idx"
            section="bugs"
            :completed="isCompleted(issue.key)"
            :expanded="expandedKey === issue.key"
            :checklist-expanded="expandedChecklists.has(issue.key)"
            :drop-target="dragOverSection === 'bugs' && dragOverIndex === idx"
            @toggle-expand="toggleExpand"
            @open-detail="openDetail"
            @toggle-completed="toggleCompleted"
            @toggle-checklist="toggleChecklist"
            @drag-start="onDragStart"
            @drag-over="onDragOver"
            @drag-leave="onDragLeave"
            @drop="onDrop"
            @drag-end="onDragEnd"
          />
          <IssueRowDetail
            v-if="expandedKey === issue.key"
            :issue="issue"
            :detail="detailCache[issue.key]"
            :loading="detailLoading.has(issue.key)"
            :error="detailError[issue.key]"
            @open-jira="onOpenJira"
            @open-detail="openDetail"
          />
        </template>
      </template>

      <template v-for="(issue, idx) in tasks" :key="issue.key">
        <StandupTaskRow
          :issue="issue"
          :index="idx"
          section="tasks"
          :completed="isCompleted(issue.key)"
          :expanded="expandedKey === issue.key"
          :checklist-expanded="expandedChecklists.has(issue.key)"
          :drop-target="dragOverSection === 'tasks' && dragOverIndex === idx"
          @toggle-expand="toggleExpand"
          @open-detail="openDetail"
          @toggle-completed="toggleCompleted"
          @toggle-checklist="toggleChecklist"
          @drag-start="onDragStart"
          @drag-over="onDragOver"
          @drag-leave="onDragLeave"
          @drop="onDrop"
          @drag-end="onDragEnd"
        />
        <IssueRowDetail
          v-if="expandedKey === issue.key"
          :issue="issue"
          :detail="detailCache[issue.key]"
          :loading="detailLoading.has(issue.key)"
          :error="detailError[issue.key]"
          @open-jira="onOpenJira"
          @open-detail="openDetail"
        />
      </template>
    </div>

    <div class="panel-footer">
      <span class="hint" title="Toggle panel">⌨ Win+Shift+D</span>
      <span v-if="lastRun" class="footer-meta">
        {{ lastRun.issueCount }} task{{ lastRun.issueCount === 1 ? '' : 's' }} ·
        {{ lastRun.postedToTeams ? 'posted' : 'not posted' }}
      </span>
    </div>
  </div>
</template>

<style scoped>
.panel {
  width: 100%;
  height: 100vh;
  background: var(--bg-primary);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: var(--radius-lg);
  box-shadow:
    0 0 0 1px rgba(255, 255, 255, 0.06),
    0 0 20px rgba(96, 165, 250, 0.08),
    0 25px 50px -12px rgba(0, 0, 0, 0.6);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  position: relative;
}

/* Resize hit zones — invisible but clickable. Edges 6px, corners 12x12. */
.resize-edge,
.resize-corner {
  position: absolute;
  z-index: 1000;
}
.resize-n { top: 0;    left: 8px;  right: 8px; height: 6px; cursor: ns-resize; }
.resize-s { bottom: 0; left: 8px;  right: 8px; height: 6px; cursor: ns-resize; }
.resize-w { left: 0;   top: 8px;   bottom: 8px; width: 6px; cursor: ew-resize; }
.resize-e { right: 0;  top: 8px;   bottom: 8px; width: 6px; cursor: ew-resize; }
.resize-nw { top: 0;    left: 0;   width: 12px; height: 12px; cursor: nwse-resize; }
.resize-ne { top: 0;    right: 0;  width: 12px; height: 12px; cursor: nesw-resize; }
.resize-sw { bottom: 0; left: 0;   width: 12px; height: 12px; cursor: nesw-resize; }
.resize-se { bottom: 0; right: 0;  width: 12px; height: 12px; cursor: nwse-resize; }

.panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 12px;
  border-bottom: 1px solid var(--border-subtle);
}

.title-group {
  display: flex;
  align-items: baseline;
  gap: 8px;
}

.title {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary);
  letter-spacing: 0.02em;
}

.subtitle {
  font-size: 11px;
  color: var(--text-secondary);
  font-family: var(--font-mono);
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 4px;
}

.icon-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  position: relative;
  width: 24px;
  height: 24px;
  border: none;
  background: transparent;
  color: var(--text-secondary);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: background 0.1s ease, color 0.1s ease;
}

.icon-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.icon-btn.active {
  background: var(--bg-selected);
  color: var(--accent-blue);
}

.icon-btn[disabled] {
  opacity: 0.5;
  cursor: not-allowed;
}

.badge {
  position: absolute;
  top: -2px;
  right: -2px;
  background: var(--accent-blue);
  color: white;
  font-size: 9px;
  font-weight: 600;
  padding: 0 4px;
  border-radius: 999px;
  min-width: 14px;
  height: 14px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.spin {
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.error-banner {
  background: rgba(248, 113, 113, 0.08);
  border-bottom: 1px solid rgba(248, 113, 113, 0.2);
  color: #f87171;
  padding: 6px 12px;
  font-size: 11px;
  font-family: var(--font-mono);
}

.history-section {
  border-bottom: 1px solid var(--border-subtle);
}

/* --- Grid layout (subgrid keeps every row's columns aligned) --- */

.panel-body {
  flex: 1;
  overflow-y: auto;
  display: grid;
  /* Columns:
       1. checkbox
       2. key + (pts)
       3. type pill
       4. checklist icon (reserved even if absent)
       5. summary (stretch)
       6. meta cluster (priority + due)
       7. drag handle — always rightmost so it doesn't drift with meta contents */
  grid-template-columns:
    24px
    auto
    auto
    16px
    minmax(0, 1fr)
    auto
    20px;
  align-items: center;
  column-gap: 10px;
  padding: 6px 0;
}

.section-header {
  grid-column: 1 / -1;
  display: flex;
  align-items: baseline;
  gap: 6px;
  padding: 8px 12px 4px;
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  border-bottom: 1px solid var(--border-subtle);
  margin-bottom: 2px;
}

.bugs-header {
  color: #f87171;
  background: rgba(248, 113, 113, 0.05);
}

.section-emoji {
  font-size: 12px;
}

.section-count {
  margin-left: auto;
  color: var(--text-placeholder);
  font-weight: 400;
}

.panel-empty {
  grid-column: 1 / -1;
  display: flex;
  align-items: center;
  justify-content: center;
  text-align: center;
  font-size: 12px;
  color: var(--text-secondary);
  padding: 24px 16px;
  line-height: 1.5;
  flex: 1;
}

.link-btn {
  background: transparent;
  border: none;
  color: var(--accent-blue);
  font-size: 12px;
  cursor: pointer;
  padding: 0;
  margin-left: 4px;
  text-decoration: underline;
}

/* Row-level styles (task-row, checkbox, cells, pills, drag handle) live in
   StandupTaskRow.vue. Inline expansion lives in IssueRowDetail.vue. */

/* --- Footer --- */

.panel-footer {
  border-top: 1px solid var(--border-subtle);
  padding: 6px 12px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 10px;
  color: var(--text-placeholder);
  font-family: var(--font-mono);
}

.hint {
  letter-spacing: 0.02em;
}

.footer-meta {
  color: var(--text-secondary);
}
</style>
