<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useMissionControl } from "@/composables/useMissionControl";
import { useSessionHistory } from "@/composables/useSessionHistory";
import PinButton from "@/components/common/PinButton.vue";
import ResizeHandles from "@/components/common/ResizeHandles.vue";
import RefreshButton from "@/components/common/RefreshButton.vue";
import ConnectionStatusPanel from "./ConnectionStatusPanel.vue";
import SessionCard from "./SessionCard.vue";

const {
  pinned,
  sessions,
  visibleSessions,
  sourceFilter,
  setSourceFilter,
  selectedPid,
  expandedPid,
  sessionsCollapsed,
  refreshSessions,
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
} = useMissionControl();

type FilterChip = { key: "all" | "mc" | "claude" | "tmux"; label: string };
const FILTER_CHIPS: FilterChip[] = [
  { key: "all", label: "All" },
  { key: "mc", label: "MC" },
  { key: "claude", label: "claude" },
  { key: "tmux", label: "tmux" },
];

function sessionCountForChip(key: FilterChip["key"]): number {
  if (key === "all") return sessions.value.length;
  if (key === "mc") return sessions.value.filter((s) => s.source === "mc").length;
  if (key === "claude")
    return sessions.value.filter((s) => s.source === "mc" || s.source === "claude-external").length;
  return sessions.value.filter((s) => s.source === "tmux").length;
}

const { history, refresh: refreshHistory, forget, resume } = useSessionHistory();
const historyCollapsed = ref(true);

onMounted(() => {
  refreshHistory();
});

function toggleHistoryCollapsed() {
  historyCollapsed.value = !historyCollapsed.value;
  if (!historyCollapsed.value) refreshHistory();
}

function endedAgo(endedAt: number | null): string {
  if (!endedAt) return "";
  const sec = Math.floor((Date.now() - endedAt) / 1000);
  if (sec < 60) return `${sec}s ago`;
  if (sec < 3600) return `${Math.floor(sec / 60)}m ago`;
  if (sec < 86400) return `${Math.floor(sec / 3600)}h ago`;
  return `${Math.floor(sec / 86400)}d ago`;
}

async function onResume(sid: string) {
  const info = await resume(sid);
  if (info) {
    await openSessionDetail({
      pid: info.pid,
      sessionId: info.sessionId,
      cwd: info.cwd,
      // Other ClaudeSession fields are filled in by MC's next poll.
      startedAt: info.startedAt,
      kind: "interactive",
      name: null,
      entrypoint: "mc",
      isAlive: true,
      subagentCount: 0,
      subagents: [],
      status: "idle",
      lastMessageAt: null,
      label: null,
      worktreePath: info.worktreePath,
      source: "mc",
      tmuxSessionName: `claude-${info.sessionId}`,
      runningCommand: null,
      currentPath: info.cwd,
      attached: false,
      windowCount: 1,
    });
  }
}
</script>

<template>
  <div class="mc-app">
    <ResizeHandles />
    <div class="mc-header">
      <div class="mc-title-group">
        <span class="mc-title">Mission Control</span>
        <span class="mc-experimental" title="Work in progress; behavior may change between builds">(experimental)</span>
      </div>
      <PinButton
        :pinned="pinned"
        :size="24"
        :pin-title="'Pin (keep open when focus leaves)'"
        :unpin-title="'Unpin (auto-hide on focus loss)'"
        @toggle="togglePin"
      />
    </div>
    <div class="mc-divider" />
    <ConnectionStatusPanel
      :statuses="connectionStatuses"
      :loading="connectionsLoading"
      :collapsed="connectionsCollapsed"
      :hide-errors="connectionsHideErrors"
      :on-refresh="refreshConnections"
      @toggle="toggleConnectionsCollapsed"
      @toggle-hide-errors="toggleConnectionsHideErrors"
      @select="selectConnection"
    />
    <div class="mc-divider" />
    <div class="mc-section">
      <div class="mc-section-header" @click="toggleSessionsCollapsed">
        <svg
          class="mc-chevron"
          :class="{ collapsed: sessionsCollapsed }"
          viewBox="0 0 16 16"
          fill="currentColor"
          width="10"
          height="10"
        >
          <path d="M4.646 1.646a.5.5 0 0 1 .708 0l6 6a.5.5 0 0 1 0 .708l-6 6a.5.5 0 0 1-.708-.708L10.293 8 4.646 2.354a.5.5 0 0 1 0-.708z" />
        </svg>
        <span class="mc-section-title">Tmux Sessions</span>
        <span class="mc-section-count">{{ sessions.length }}</span>
        <RefreshButton
          class="mc-refresh"
          :on-refresh="refreshSessions"
          title="Refresh tmux sessions"
        />
      </div>
      <div v-if="!sessionsCollapsed" class="mc-filter-chips">
        <button
          v-for="chip in FILTER_CHIPS"
          :key="chip.key"
          class="mc-chip"
          :class="{ active: sourceFilter === chip.key }"
          @click="setSourceFilter(chip.key)"
        >
          {{ chip.label }}
          <span class="mc-chip-count">{{ sessionCountForChip(chip.key) }}</span>
        </button>
      </div>
      <div v-if="!sessionsCollapsed" class="mc-list">
        <div v-if="visibleSessions.length === 0" class="mc-empty">
          {{ sessions.length === 0 ? "No active tmux sessions" : "No sessions match this filter" }}
        </div>
        <SessionCard
          v-for="s in visibleSessions"
          :key="s.sessionId || s.tmuxSessionName"
          :session="s"
          :selected="selectedPid === s.pid"
          :expanded="expandedPid === s.pid"
          @toggle-expand="toggleSessionExpand"
          @open="openSessionDetail"
        />
      </div>
    </div>
    <div class="mc-divider" />
    <div class="mc-section mc-section-history">
      <div class="mc-section-header" @click="toggleHistoryCollapsed">
        <svg
          class="mc-chevron"
          :class="{ collapsed: historyCollapsed }"
          viewBox="0 0 16 16"
          fill="currentColor"
          width="10"
          height="10"
        >
          <path d="M4.646 1.646a.5.5 0 0 1 .708 0l6 6a.5.5 0 0 1 0 .708l-6 6a.5.5 0 0 1-.708-.708L10.293 8 4.646 2.354a.5.5 0 0 1 0-.708z" />
        </svg>
        <span class="mc-section-title">History</span>
        <span class="mc-section-count">{{ history.length }}</span>
      </div>
      <div v-if="!historyCollapsed" class="mc-list mc-history-list">
        <div v-if="history.length === 0" class="mc-empty">No history yet</div>
        <div v-for="h in history" :key="h.sessionId" class="mc-history-row">
          <div class="mc-history-meta">
            <span class="mc-history-name" :title="h.cwd">
              {{ h.label ?? h.sessionId.slice(0, 8) }}
            </span>
            <span class="mc-history-ago">{{ endedAgo(h.endedAt) }}</span>
          </div>
          <div class="mc-history-cwd" :title="h.cwd">{{ h.cwd }}</div>
          <div class="mc-history-actions">
            <button class="mc-history-btn resume" @click="onResume(h.sessionId)">Resume</button>
            <button class="mc-history-btn forget" @click="forget(h.sessionId)">Forget</button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.mc-app {
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
}

.mc-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 14px;
  -webkit-app-region: drag;
}

.mc-title-group {
  display: flex;
  align-items: baseline;
  gap: 8px;
  min-width: 0;
}

.mc-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary);
  letter-spacing: 0.2px;
}

.mc-experimental {
  font-size: 10px;
  font-family: var(--font-mono);
  color: var(--text-secondary);
  font-style: italic;
  letter-spacing: 0.2px;
  opacity: 0.85;
  -webkit-app-region: no-drag;
  cursor: help;
}

.mc-divider {
  height: 1px;
  background: var(--border-subtle);
}

.mc-section {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.mc-section-header {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 7px 14px;
  cursor: pointer;
  transition: background 0.1s ease;
  user-select: none;
  flex-shrink: 0;
}

.mc-section-header:hover {
  background: var(--bg-hover);
}

.mc-chevron {
  color: var(--text-secondary);
  transition: transform 0.15s ease;
  transform: rotate(90deg);
}

.mc-chevron.collapsed {
  transform: rotate(0deg);
}

.mc-section-title {
  font-size: 11px;
  font-weight: 600;
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.mc-section-count {
  font-size: 10px;
  color: var(--text-placeholder);
  background: rgba(255, 255, 255, 0.06);
  padding: 0 5px;
  border-radius: 3px;
  line-height: 16px;
}

.mc-refresh {
  margin-left: auto;
}

.mc-list {
  flex: 1;
  overflow-y: auto;
  min-height: 0;
}

.mc-filter-chips {
  display: flex;
  gap: 4px;
  padding: 4px 12px 6px;
  border-bottom: 1px solid var(--border-subtle);
  flex-shrink: 0;
}

.mc-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 8px;
  font-size: 10px;
  font-weight: 500;
  letter-spacing: 0.3px;
  text-transform: lowercase;
  background: transparent;
  border: 1px solid var(--border-subtle);
  border-radius: 999px;
  color: var(--text-secondary);
  cursor: pointer;
  transition: background 0.1s ease, color 0.1s ease, border-color 0.1s ease;
}

.mc-chip:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.mc-chip.active {
  background: rgba(96, 165, 250, 0.15);
  color: var(--accent-blue);
  border-color: rgba(96, 165, 250, 0.4);
}

.mc-chip-count {
  font-size: 9px;
  color: var(--text-placeholder);
  font-variant-numeric: tabular-nums;
}

.mc-chip.active .mc-chip-count {
  color: var(--accent-blue);
}

.mc-empty {
  padding: 24px 16px;
  text-align: center;
  font-size: 11px;
  color: var(--text-secondary);
}

.mc-section-history {
  flex: 0 0 auto;
  max-height: 50%;
}

.mc-history-list {
  flex: 1;
  overflow-y: auto;
  max-height: 240px;
}

.mc-history-row {
  padding: 6px 14px;
  border-bottom: 1px solid var(--border-subtle);
}

.mc-history-row:last-child {
  border-bottom: 0;
}

.mc-history-meta {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 8px;
}

.mc-history-name {
  font-size: 12px;
  font-weight: 500;
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
}

.mc-history-ago {
  font-size: 10px;
  color: var(--text-placeholder);
  flex-shrink: 0;
}

.mc-history-cwd {
  font-size: 10px;
  font-family: var(--font-mono);
  color: var(--text-placeholder);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  margin-top: 2px;
}

.mc-history-actions {
  display: flex;
  gap: 4px;
  margin-top: 4px;
}

.mc-history-btn {
  padding: 2px 8px;
  font-size: 10px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border-subtle);
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
}

.mc-history-btn.resume {
  border-color: rgba(52, 211, 153, 0.3);
  color: var(--accent-green);
}

.mc-history-btn.resume:hover {
  background: rgba(52, 211, 153, 0.1);
}

.mc-history-btn.forget:hover {
  color: var(--accent-red);
  border-color: rgba(248, 113, 113, 0.4);
}
</style>
