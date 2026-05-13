<script setup lang="ts">
import { computed } from "vue";
import type { ClaudeSession } from "@/lib/tauri";
import { displayNameForSession, formatRelative } from "@/lib/format";

type SessionStatusKey = "dead" | "busy" | "idle";
const STATUS_INFO: Record<SessionStatusKey, { dotClass: string; label: string }> = {
  dead: { dotClass: "dot-dead", label: "dead" },
  busy: { dotClass: "dot-busy", label: "busy" },
  idle: { dotClass: "dot-idle", label: "idle" },
};

const props = defineProps<{
  session: ClaudeSession;
  selected?: boolean;
  expanded?: boolean;
}>();

const emit = defineEmits<{
  "toggle-expand": [session: ClaudeSession];
  open: [session: ClaudeSession];
}>();

const displayName = computed(() =>
  displayNameForSession(props.session.name, props.session.cwd),
);

const relativeTime = computed(() =>
  formatRelative(props.session.lastMessageAt ?? props.session.startedAt),
);

const status = computed(() => {
  const key = (props.session.status === "dead" || props.session.status === "busy"
    ? props.session.status
    : "idle") as SessionStatusKey;
  return STATUS_INFO[key];
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

function handleKeydown(e: KeyboardEvent) {
  if (e.key === "Enter" || e.key === "ArrowRight") {
    e.preventDefault();
    emit("open", props.session);
  } else if (e.key === " ") {
    e.preventDefault();
    emit("toggle-expand", props.session);
  }
}
</script>

<template>
  <div
    class="session-card"
    :class="{ selected, expanded }"
    tabindex="0"
    role="button"
    :aria-expanded="expanded"
    :aria-label="`${displayName} — ${status.label}`"
    @click="emit('toggle-expand', session)"
    @keydown="handleKeydown"
  >
    <div class="session-row-compact">
      <svg
        class="session-chevron"
        :class="{ expanded }"
        viewBox="0 0 16 16"
        fill="currentColor"
        width="8"
        height="8"
      >
        <path d="M4.646 1.646a.5.5 0 0 1 .708 0l6 6a.5.5 0 0 1 0 .708l-6 6a.5.5 0 0 1-.708-.708L10.293 8 4.646 2.354a.5.5 0 0 1 0-.708z" />
      </svg>
      <span class="session-dot" :class="status.dotClass" />
      <span class="session-name" :title="session.cwd">{{ displayName }}</span>
      <button class="session-open-btn compact-open" title="Open session detail" @click.stop="emit('open', session)">Open</button>
      <span class="session-time">{{ relativeTime }}</span>
    </div>
    <div class="session-expand-wrapper" :class="{ expanded }">
      <div class="session-expanded">
        <div class="session-detail-row">
          <span class="session-badge status-badge" :class="status.dotClass">{{ status.label }}</span>
          <span v-if="session.kind" class="session-badge muted">{{ session.kind }}</span>
          <span class="session-badge muted">PID {{ session.pid }}</span>
          <button class="session-open-btn expanded-open" @click.stop="emit('open', session)">Open</button>
        </div>
        <div v-if="agentTypeSummary" class="session-detail-row">
          <span class="agent-count">{{ session.subagentCount }} agent{{ session.subagentCount !== 1 ? "s" : "" }}</span>
          <span class="agent-types">{{ agentTypeSummary }}</span>
        </div>
        <div class="session-cwd" :title="session.cwd">{{ session.cwd }}</div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.session-card {
  border-bottom: 1px solid var(--border-subtle);
  transition: background 0.1s ease;
  cursor: pointer;
  min-width: 0;
  overflow: hidden;
}

.session-card:last-child {
  border-bottom: none;
}

.session-card:hover,
.session-card:focus-visible {
  background: var(--bg-hover);
  outline: none;
}

.session-card:focus-visible {
  box-shadow: inset 0 0 0 1px var(--accent-blue);
}

.session-card.selected {
  background: var(--bg-selected);
  border-left: 2px solid var(--accent-blue);
}

.session-card.selected .session-row-compact {
  padding-left: 12px;
}

.session-card.expanded:not(.selected) {
  background: var(--bg-hover);
}

.session-row-compact {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px 14px;
  font-size: 12px;
}

.session-chevron {
  flex-shrink: 0;
  color: var(--text-placeholder);
  transition: transform 0.15s ease;
  transform: rotate(0deg);
}

.session-chevron.expanded {
  transform: rotate(90deg);
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

/* Open button in compact row — visible on card hover */
.compact-open {
  opacity: 0;
  transition: opacity 0.1s ease;
  flex-shrink: 0;
}

.session-card:hover .compact-open,
.session-card:focus-visible .compact-open {
  opacity: 1;
}

/* Hide compact open when expanded (the expanded row has its own) */
.session-card.expanded .compact-open {
  display: none;
}

.session-expand-wrapper {
  display: grid;
  grid-template-rows: 0fr;
  transition: grid-template-rows 0.15s ease;
  overflow: hidden;
  min-width: 0;
}

.session-expand-wrapper.expanded {
  grid-template-rows: 1fr;
}

.session-expanded {
  min-height: 0;
  min-width: 0;
  overflow: hidden;
  padding: 0 14px 0 34px;
  transition: padding 0.15s ease;
}

.session-expand-wrapper.expanded .session-expanded {
  padding: 2px 14px 8px 34px;
}

.session-open-btn {
  font-size: 9px;
  padding: 0 6px;
  line-height: 16px;
  border-radius: 3px;
  border: 1px solid rgba(96, 165, 250, 0.25);
  background: rgba(96, 165, 250, 0.08);
  color: var(--accent-blue);
  cursor: pointer;
  font-weight: 500;
  text-transform: uppercase;
  letter-spacing: 0.3px;
  transition: all 0.1s ease;
}

.session-open-btn:hover {
  background: rgba(96, 165, 250, 0.18);
  border-color: rgba(96, 165, 250, 0.4);
}

.expanded-open {
  margin-left: auto;
}

.session-detail-row {
  display: flex;
  align-items: center;
  gap: 5px;
  margin-bottom: 3px;
  min-width: 0;
  overflow: hidden;
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
