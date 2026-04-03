<script setup lang="ts">
import { computed } from "vue";
import type { ClaudeSession } from "@/lib/tauri";

const props = defineProps<{
  session: ClaudeSession;
  selected?: boolean;
  expanded?: boolean;
}>();

const emit = defineEmits<{
  "toggle-expand": [session: ClaudeSession];
  open: [session: ClaudeSession];
}>();

let clickTimer: ReturnType<typeof setTimeout> | null = null;

function handleClick() {
  if (clickTimer) clearTimeout(clickTimer);
  clickTimer = setTimeout(() => {
    emit("toggle-expand", props.session);
    clickTimer = null;
  }, 200);
}

function handleDblClick() {
  if (clickTimer) {
    clearTimeout(clickTimer);
    clickTimer = null;
  }
  emit("open", props.session);
}

const shortCwd = computed(() => {
  const parts = props.session.cwd.split("/");
  return parts[parts.length - 1] || props.session.cwd;
});

const displayName = computed(() => props.session.name || shortCwd.value);

const relativeTime = computed(() => {
  const ts = props.session.lastMessageAt
    ? new Date(props.session.lastMessageAt).getTime()
    : props.session.startedAt;
  const diff = Date.now() - ts;
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}h ago`;
  return `${Math.floor(hrs / 24)}d ago`;
});

const statusDotClass = computed(() => {
  if (props.session.status === "dead") return "dot-dead";
  if (props.session.status === "busy") return "dot-busy";
  return "dot-idle";
});

const statusLabel = computed(() => {
  if (props.session.status === "dead") return "dead";
  if (props.session.status === "busy") return "active";
  return "idle";
});

const agentTypeSummary = computed(() => {
  if (props.session.subagents.length === 0) return null;
  const counts = new Map<string, number>();
  for (const a of props.session.subagents) {
    counts.set(a.agentType, (counts.get(a.agentType) || 0) + 1);
  }
  return Array.from(counts.entries())
    .map(([type, count]) => (count > 1 ? `${count} ${type}` : type))
    .join(", ");
});
</script>

<template>
  <div
    class="session-card"
    :class="{ selected, expanded }"
    @click="handleClick"
    @dblclick="handleDblClick"
  >
    <div class="session-row-compact">
      <span class="session-dot" :class="statusDotClass" />
      <span class="session-name" :title="session.cwd">{{ displayName }}</span>
      <span class="session-time">{{ relativeTime }}</span>
    </div>
    <div v-if="expanded" class="session-expanded">
      <div class="session-detail-row">
        <span class="session-badge status-badge" :class="statusDotClass">{{ statusLabel }}</span>
        <span v-if="session.kind" class="session-badge muted">{{ session.kind }}</span>
        <span class="session-badge muted">PID {{ session.pid }}</span>
      </div>
      <div v-if="agentTypeSummary" class="session-detail-row">
        <span class="agent-count">{{ session.subagentCount }} agent{{ session.subagentCount !== 1 ? "s" : "" }}</span>
        <span class="agent-types">{{ agentTypeSummary }}</span>
      </div>
      <div class="session-cwd" :title="session.cwd">{{ session.cwd }}</div>
    </div>
  </div>
</template>

<style scoped>
.session-card {
  border-bottom: 1px solid var(--border-subtle);
  transition: background 0.1s ease;
  cursor: pointer;
}

.session-card:last-child {
  border-bottom: none;
}

.session-card:hover {
  background: var(--bg-hover);
}

.session-card.selected {
  background: var(--bg-selected);
  border-left: 2px solid var(--accent-blue);
}

.session-card.selected .session-row-compact {
  padding-left: 12px;
}

.session-card.expanded {
  background: var(--bg-hover);
}

.session-row-compact {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 14px;
  font-size: 12px;
}

.session-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
}

.dot-idle {
  background: var(--accent-green);
  box-shadow: 0 0 4px rgba(74, 222, 128, 0.4);
}

.dot-busy {
  background: var(--accent-yellow);
  box-shadow: 0 0 4px rgba(250, 204, 21, 0.4);
}

.dot-dead {
  background: var(--accent-red);
  box-shadow: 0 0 4px rgba(248, 113, 113, 0.4);
}

.session-name {
  font-weight: 500;
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  min-width: 0;
}

.session-time {
  font-size: 10px;
  color: var(--text-placeholder);
  margin-left: auto;
  white-space: nowrap;
  flex-shrink: 0;
}

.session-expanded {
  padding: 2px 14px 8px 29px;
}

.session-detail-row {
  display: flex;
  align-items: center;
  gap: 5px;
  margin-bottom: 3px;
}

.session-badge {
  font-size: 9px;
  padding: 0 4px;
  border-radius: 3px;
  font-weight: 500;
  text-transform: uppercase;
  letter-spacing: 0.3px;
  line-height: 16px;
}

.session-badge.muted {
  background: rgba(255, 255, 255, 0.04);
  color: var(--text-placeholder);
}

.session-badge.status-badge.dot-idle {
  background: rgba(74, 222, 128, 0.12);
  color: var(--accent-green);
}

.session-badge.status-badge.dot-busy {
  background: rgba(250, 204, 21, 0.12);
  color: var(--accent-yellow);
}

.session-badge.status-badge.dot-dead {
  background: rgba(248, 113, 113, 0.12);
  color: var(--accent-red);
}

.agent-count {
  font-size: 11px;
  color: var(--accent-green);
  font-weight: 500;
}

.agent-types {
  font-size: 11px;
  color: var(--text-secondary);
}

.session-cwd {
  margin-top: 2px;
  font-size: 11px;
  font-family: var(--font-mono);
  color: var(--text-placeholder);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
