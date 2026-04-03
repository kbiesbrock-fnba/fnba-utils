<script setup lang="ts">
import type { ConnectionStatus } from "@/lib/tauri";
import ConnectionStatusRow from "./ConnectionStatusRow.vue";

defineProps<{
  statuses: ConnectionStatus[];
  loading: boolean;
  collapsed: boolean;
}>();

const emit = defineEmits<{
  toggle: [];
  refresh: [];
}>();
</script>

<template>
  <div class="conn-panel">
    <div class="conn-header" @click="emit('toggle')">
      <svg
        class="conn-chevron"
        :class="{ collapsed }"
        viewBox="0 0 16 16"
        fill="currentColor"
        width="10"
        height="10"
      >
        <path d="M4.646 1.646a.5.5 0 0 1 .708 0l6 6a.5.5 0 0 1 0 .708l-6 6a.5.5 0 0 1-.708-.708L10.293 8 4.646 2.354a.5.5 0 0 1 0-.708z" />
      </svg>
      <span class="conn-title">Connections</span>
      <span class="conn-count">{{ statuses.length }}</span>
      <button
        class="conn-refresh"
        :class="{ spinning: loading }"
        title="Refresh"
        @click.stop="emit('refresh')"
      >
        <svg viewBox="0 0 16 16" fill="currentColor" width="11" height="11">
          <path d="M11.534 7h3.932a.25.25 0 0 1 .192.41l-1.966 2.36a.25.25 0 0 1-.384 0l-1.966-2.36a.25.25 0 0 1 .192-.41zm-11 2h3.932a.25.25 0 0 0 .192-.41L2.692 6.23a.25.25 0 0 0-.384 0L.342 8.59A.25.25 0 0 0 .534 9z" />
          <path d="M8 3c-1.552 0-2.94.707-3.857 1.818a.5.5 0 1 1-.771-.636A6.002 6.002 0 0 1 13.917 7H12.9A5.002 5.002 0 0 0 8 3zM3.1 9a5.002 5.002 0 0 0 8.757 2.182.5.5 0 1 1 .771.636A6.002 6.002 0 0 1 2.083 9H3.1z" />
        </svg>
      </button>
    </div>
    <div v-if="!collapsed" class="conn-list">
      <div v-if="statuses.length === 0 && loading" class="conn-empty">Loading...</div>
      <div v-else-if="statuses.length === 0" class="conn-empty">No connections configured</div>
      <ConnectionStatusRow v-for="s in statuses" :key="s.server" :status="s" />
    </div>
  </div>
</template>

<style scoped>
.conn-panel {
  flex-shrink: 0;
}

.conn-header {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 7px 14px;
  cursor: pointer;
  transition: background 0.1s ease;
  user-select: none;
}

.conn-header:hover {
  background: var(--bg-hover);
}

.conn-chevron {
  color: var(--text-secondary);
  transition: transform 0.15s ease;
  transform: rotate(90deg);
}

.conn-chevron.collapsed {
  transform: rotate(0deg);
}

.conn-title {
  font-size: 11px;
  font-weight: 600;
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.conn-count {
  font-size: 10px;
  color: var(--text-placeholder);
  background: rgba(255, 255, 255, 0.06);
  padding: 0 5px;
  border-radius: 3px;
  line-height: 16px;
}

.conn-refresh {
  margin-left: auto;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  border: none;
  background: transparent;
  color: var(--text-secondary);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: background 0.1s ease, color 0.1s ease;
}

.conn-refresh:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.conn-refresh.spinning svg {
  animation: spin 0.8s linear infinite;
}

.conn-list {
  max-height: 200px;
  overflow-y: auto;
}

.conn-empty {
  padding: 12px 14px;
  font-size: 11px;
  color: var(--text-secondary);
  text-align: center;
}
</style>
