<script setup lang="ts">
import { useMissionControl } from "@/composables/useMissionControl";
import ConnectionStatusPanel from "./ConnectionStatusPanel.vue";
import SessionCard from "./SessionCard.vue";

const {
  pinned,
  sessions,
  selectedPid,
  expandedPid,
  sessionsCollapsed,
  togglePin,
  toggleSessionExpand,
  openSessionDetail,
  toggleSessionsCollapsed,
  connectionStatuses,
  connectionsLoading,
  connectionsCollapsed,
  toggleConnectionsCollapsed,
  refreshConnections,
  selectConnection,
} = useMissionControl();
</script>

<template>
  <div class="mc-app">
    <div class="mc-header">
      <span class="mc-title">Mission Control</span>
      <button
        class="mc-btn"
        :class="{ active: pinned }"
        :title="pinned ? 'Unpin (hide on startup)' : 'Pin (show on startup)'"
        @click="togglePin"
      >
        <svg viewBox="0 0 16 16" fill="currentColor" width="12" height="12">
          <path d="M9.828.722a.5.5 0 0 1 .354.146l4.95 4.95a.5.5 0 0 1 0 .707c-.48.48-1.072.588-1.503.588-.177 0-.335-.018-.46-.039l-3.134 3.134a6 6 0 0 1 .16 1.013c.046.702-.032 1.687-.72 2.375a.5.5 0 0 1-.707 0l-2.829-2.828-3.182 3.182a.5.5 0 0 1-.707-.708l3.182-3.182L2.398 8.23a.5.5 0 0 1 0-.707c.688-.688 1.673-.767 2.375-.72a6 6 0 0 1 1.013.16l3.134-3.133a3 3 0 0 1-.04-.461c0-.43.109-1.022.589-1.503a.5.5 0 0 1 .353-.146z" />
        </svg>
      </button>
    </div>
    <div class="mc-divider" />
    <ConnectionStatusPanel
      :statuses="connectionStatuses"
      :loading="connectionsLoading"
      :collapsed="connectionsCollapsed"
      @toggle="toggleConnectionsCollapsed"
      @refresh="refreshConnections"
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
        <span class="mc-section-title">Claude Sessions</span>
        <span class="mc-section-count">{{ sessions.length }}</span>
      </div>
      <div v-if="!sessionsCollapsed" class="mc-list">
        <div v-if="sessions.length === 0" class="mc-empty">
          No active sessions
        </div>
        <SessionCard
          v-for="s in sessions"
          :key="s.pid"
          :session="s"
          :selected="selectedPid === s.pid"
          :expanded="expandedPid === s.pid"
          @toggle-expand="toggleSessionExpand"
          @open="openSessionDetail"
        />
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

.mc-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary);
  letter-spacing: 0.2px;
}

.mc-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border: none;
  background: transparent;
  color: var(--text-secondary);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: background 0.1s ease, color 0.1s ease;
  -webkit-app-region: no-drag;
}

.mc-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.mc-btn.active {
  color: var(--accent-blue);
}

.mc-btn.active:hover {
  color: var(--accent-blue);
  background: rgba(96, 165, 250, 0.12);
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

.mc-list {
  flex: 1;
  overflow-y: auto;
  min-height: 0;
}

.mc-empty {
  padding: 24px 16px;
  text-align: center;
  font-size: 11px;
  color: var(--text-secondary);
}
</style>
