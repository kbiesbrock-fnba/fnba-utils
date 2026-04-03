<script setup lang="ts">
import { computed } from "vue";
import type { ClaudeSession } from "@/lib/tauri";

const props = defineProps<{ session: ClaudeSession; selected?: boolean }>();
const emit = defineEmits<{ select: [session: ClaudeSession] }>();

const shortCwd = computed(() => {
  const parts = props.session.cwd.split("/");
  return parts[parts.length - 1] || props.session.cwd;
});

const displayName = computed(() => props.session.name || shortCwd.value);

const relativeTime = computed(() => {
  const diff = Date.now() - props.session.startedAt;
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}h ago`;
  return `${Math.floor(hrs / 24)}d ago`;
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
  <div class="session-card" :class="{ selected }" @click="emit('select', session)">
    <div class="session-row">
      <span class="session-name" :title="session.cwd">{{ displayName }}</span>
      <span class="session-time">{{ relativeTime }}</span>
    </div>
    <div class="session-meta">
      <span v-if="session.kind" class="session-badge kind">{{ session.kind }}</span>
      <span class="session-badge pid">PID {{ session.pid }}</span>
    </div>
    <div v-if="agentTypeSummary" class="session-agents">
      <span class="agent-count">{{ session.subagentCount }} agent{{ session.subagentCount !== 1 ? "s" : "" }}</span>
      <span class="agent-types">{{ agentTypeSummary }}</span>
    </div>
    <div v-if="session.cwd" class="session-cwd" :title="session.cwd">{{ session.cwd }}</div>
  </div>
</template>

<style scoped>
.session-card {
  padding: 10px 14px;
  border-bottom: 1px solid var(--border-subtle);
  transition: background 0.1s ease;
  cursor: pointer;
}

.session-card.selected {
  background: var(--bg-selected);
  border-left: 2px solid var(--accent-blue);
  padding-left: 12px;
}

.session-card:last-child {
  border-bottom: none;
}

.session-card:hover {
  background: var(--bg-hover);
}

.session-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.session-name {
  font-size: 13px;
  font-weight: 500;
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
}

.session-time {
  font-size: 11px;
  color: var(--text-secondary);
  white-space: nowrap;
  flex-shrink: 0;
}

.session-meta {
  display: flex;
  gap: 6px;
  margin-top: 4px;
}

.session-badge {
  font-size: 10px;
  padding: 1px 6px;
  border-radius: 3px;
  font-weight: 500;
  text-transform: uppercase;
  letter-spacing: 0.3px;
}

.session-badge.kind {
  background: rgba(96, 165, 250, 0.15);
  color: var(--accent-blue);
}

.session-badge.pid {
  background: rgba(255, 255, 255, 0.06);
  color: var(--text-secondary);
}

.session-agents {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 6px;
  font-size: 11px;
}

.agent-count {
  color: var(--accent-green);
  font-weight: 500;
}

.agent-types {
  color: var(--text-secondary);
}

.session-cwd {
  margin-top: 4px;
  font-size: 11px;
  font-family: var(--font-mono);
  color: var(--text-placeholder);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
