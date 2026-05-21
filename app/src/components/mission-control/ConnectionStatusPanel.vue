<script setup lang="ts">
import { computed } from "vue";
import type { ConnectionStatus } from "@/lib/tauri";
import ConnectionStatusRow from "./ConnectionStatusRow.vue";

const props = defineProps<{
  statuses: ConnectionStatus[];
  loading: boolean;
  collapsed: boolean;
  hideErrors: boolean;
}>();

const emit = defineEmits<{
  toggle: [];
  refresh: [];
  toggleHideErrors: [];
  select: [status: ConnectionStatus];
}>();

const visibleStatuses = computed(() =>
  props.hideErrors ? props.statuses.filter((s) => !s.error) : props.statuses,
);

const erroredCount = computed(
  () => props.statuses.filter((s) => !!s.error).length,
);
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
        v-if="erroredCount > 0"
        class="conn-toggle-errors"
        :class="{ active: !hideErrors }"
        :title="
          hideErrors
            ? `Show ${erroredCount} errored connection${erroredCount === 1 ? '' : 's'}`
            : 'Hide errored connections'
        "
        @click.stop="emit('toggleHideErrors')"
      >
        <svg
          v-if="hideErrors"
          viewBox="0 0 16 16"
          fill="currentColor"
          width="11"
          height="11"
        >
          <path d="M13.359 11.238C15.06 9.72 16 8 16 8s-3-5.5-8-5.5a7.028 7.028 0 0 0-2.79.588l.77.771A5.944 5.944 0 0 1 8 3.5c2.12 0 3.879 1.168 5.168 2.457A13.134 13.134 0 0 1 14.828 8c-.058.087-.122.183-.195.288-.335.48-.83 1.12-1.465 1.755-.165.165-.337.328-.517.486l.708.709z"/>
          <path d="M11.297 9.176a3.5 3.5 0 0 0-4.474-4.474l.823.823a2.5 2.5 0 0 1 2.829 2.829l.822.822zm-2.943 1.299.822.822a3.5 3.5 0 0 1-4.474-4.474l.823.823a2.5 2.5 0 0 0 2.829 2.829z"/>
          <path d="M3.35 5.47c-.18.16-.353.322-.518.487A13.134 13.134 0 0 0 1.172 8l.195.288c.335.48.83 1.12 1.465 1.755C4.121 11.332 5.881 12.5 8 12.5c.716 0 1.39-.133 2.02-.36l.77.772A7.029 7.029 0 0 1 8 13.5C3 13.5 0 8 0 8s.939-1.721 2.641-3.238l.708.709zm10.296 8.884-12-12 .708-.708 12 12-.708.708z"/>
        </svg>
        <svg
          v-else
          viewBox="0 0 16 16"
          fill="currentColor"
          width="11"
          height="11"
        >
          <path d="M16 8s-3-5.5-8-5.5S0 8 0 8s3 5.5 8 5.5S16 8 16 8zM1.173 8a13.133 13.133 0 0 1 1.66-2.043C4.12 4.668 5.88 3.5 8 3.5c2.12 0 3.879 1.168 5.168 2.457A13.133 13.133 0 0 1 14.828 8c-.058.087-.122.183-.195.288-.335.48-.83 1.12-1.465 1.755C11.879 11.332 10.119 12.5 8 12.5c-2.12 0-3.879-1.168-5.168-2.457A13.134 13.134 0 0 1 1.172 8z"/>
          <path d="M8 5.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 0 0 0-5zM4.5 8a3.5 3.5 0 1 1 7 0 3.5 3.5 0 0 1-7 0z"/>
        </svg>
      </button>
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
      <div
        v-else-if="visibleStatuses.length === 0"
        class="conn-empty"
      >
        All {{ erroredCount }} connection{{ erroredCount === 1 ? "" : "s" }} errored — click the eye icon to show
      </div>
      <ConnectionStatusRow
        v-for="s in visibleStatuses"
        :key="s.server"
        :status="s"
        @dblclick="!s.error && emit('select', s)"
      />
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

.conn-refresh,
.conn-toggle-errors {
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

/* When the toggle button is present, it takes the auto margin and refresh
 * sits flush next to it. */
.conn-toggle-errors + .conn-refresh {
  margin-left: 0;
}

.conn-refresh:hover,
.conn-toggle-errors:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.conn-toggle-errors.active {
  color: var(--accent-red);
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
