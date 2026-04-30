<script setup lang="ts">
import { computed } from "vue";
import type { SessionDetail } from "@/lib/tauri";

const props = defineProps<{ detail: SessionDetail; pinned: boolean }>();
const emit = defineEmits<{ togglePin: [] }>();

const displayName = computed(() => {
  if (props.detail.name) return props.detail.name;
  const parts = props.detail.cwd.split("/");
  return parts[parts.length - 1] || props.detail.cwd;
});

const elapsed = computed(() => {
  const diff = Date.now() - props.detail.startedAt;
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return "< 1m";
  if (mins < 60) return `${mins}m`;
  const hrs = Math.floor(mins / 60);
  const rem = mins % 60;
  if (hrs < 24) return rem > 0 ? `${hrs}h ${rem}m` : `${hrs}h`;
  const days = Math.floor(hrs / 24);
  return `${days}d ${hrs % 24}h`;
});

const statusColor = computed(() => {
  if (props.detail.status === "busy") return "var(--accent-yellow)";
  if (props.detail.status === "dead") return "var(--accent-red)";
  return "var(--accent-green)";
});
</script>

<template>
  <div class="sd-header" data-tauri-drag-region>
    <div class="sd-title-row">
      <span class="sd-name" :title="detail.cwd">{{ displayName }}</span>
      <button
        class="sd-pin"
        :class="{ active: pinned }"
        :title="pinned ? 'Unpin' : 'Pin'"
        @click="emit('togglePin')"
      >
        <svg viewBox="0 0 16 16" fill="currentColor" width="11" height="11">
          <path d="M9.828.722a.5.5 0 0 1 .354.146l4.95 4.95a.5.5 0 0 1 0 .707c-.48.48-1.072.588-1.503.588-.177 0-.335-.018-.46-.039l-3.134 3.134a6 6 0 0 1 .16 1.013c.046.702-.032 1.687-.72 2.375a.5.5 0 0 1-.707 0l-2.829-2.828-3.182 3.182a.5.5 0 0 1-.707-.708l3.182-3.182L2.398 8.23a.5.5 0 0 1 0-.707c.688-.688 1.673-.767 2.375-.72a6 6 0 0 1 1.013.16l3.134-3.133a3 3 0 0 1-.04-.461c0-.43.109-1.022.589-1.503a.5.5 0 0 1 .353-.146z" />
        </svg>
      </button>
      <div class="sd-status">
        <span class="sd-status-dot" :style="{ background: statusColor }" />
        <span class="sd-status-text">{{ detail.status }}</span>
      </div>
    </div>
    <div class="sd-badges">
      <span class="sd-badge pid">PID {{ detail.pid }}</span>
      <span v-if="detail.gitBranch" class="sd-badge branch">{{ detail.gitBranch }}</span>
      <span class="sd-badge elapsed">{{ elapsed }}</span>
    </div>
    <div class="sd-cwd" :title="detail.cwd">{{ detail.cwd }}</div>
  </div>
</template>

<style scoped>
.sd-header {
  padding: 12px 14px 10px;
  -webkit-app-region: drag;
}

.sd-title-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.sd-name {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
}

.sd-pin {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border: none;
  background: transparent;
  color: var(--text-secondary);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: background 0.1s ease, color 0.1s ease;
  -webkit-app-region: no-drag;
}

.sd-pin:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.sd-pin.active {
  color: var(--accent-blue);
}

.sd-pin.active:hover {
  background: rgba(96, 165, 250, 0.12);
}

.sd-status {
  display: flex;
  align-items: center;
  gap: 5px;
  flex-shrink: 0;
}

.sd-status-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
}

.sd-status-text {
  font-size: 11px;
  color: var(--text-secondary);
  text-transform: capitalize;
}

.sd-badges {
  display: flex;
  gap: 6px;
  margin-top: 6px;
  flex-wrap: wrap;
}

.sd-badge {
  font-size: 10px;
  padding: 1px 6px;
  border-radius: 3px;
  font-weight: 500;
  letter-spacing: 0.3px;
}

.sd-badge.pid {
  background: rgba(255, 255, 255, 0.06);
  color: var(--text-secondary);
  text-transform: uppercase;
}

.sd-badge.branch {
  background: rgba(96, 165, 250, 0.15);
  color: var(--accent-blue);
  font-family: var(--font-mono);
}

.sd-badge.elapsed {
  background: rgba(255, 255, 255, 0.06);
  color: var(--text-secondary);
}

.sd-cwd {
  margin-top: 6px;
  font-size: 11px;
  font-family: var(--font-mono);
  color: var(--text-placeholder);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
