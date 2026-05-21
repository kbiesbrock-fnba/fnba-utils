<script setup lang="ts">
import { computed } from "vue";
import type { StandupRunResult } from "@/lib/tauri";

const props = defineProps<{ result: StandupRunResult }>();

const totalPoints = computed(() =>
  props.result.report.groups.reduce((acc, g) => acc + g.totalPoints, 0),
);

function pts(n: number | null): string {
  if (n === null || n === undefined) return "—";
  return Number.isInteger(n) ? `${n}` : n.toFixed(1);
}

function pointsTotal(n: number): string {
  return Number.isInteger(n) ? `${n}` : n.toFixed(1);
}
</script>

<template>
  <div class="report">
    <div class="report-header">
      <div class="report-stats">
        <span class="stat">
          <span class="stat-num">{{ result.report.issueCount }}</span>
          <span class="stat-label">issues</span>
        </span>
        <span class="stat-sep">·</span>
        <span class="stat">
          <span class="stat-num">{{ pointsTotal(totalPoints) }}</span>
          <span class="stat-label">points</span>
        </span>
      </div>
      <div class="report-badges">
        <span v-if="result.postedToTeams" class="badge ok">✓ Posted to Teams</span>
        <span v-if="result.copiedToClipboard" class="badge ok">✓ Copied</span>
      </div>
    </div>

    <div
      v-for="warning in result.warnings"
      :key="warning"
      class="warning"
    >⚠ {{ warning }}</div>

    <div class="report-body">
      <div v-if="result.report.groups.length === 0" class="empty">
        No assigned tasks found.
      </div>
      <div v-for="group in result.report.groups" :key="group.group" class="group">
        <div class="group-header" :data-group="group.group">
          <span class="group-emoji">{{ group.emoji }}</span>
          <span class="group-label">{{ group.label }}</span>
          <span class="group-count">({{ group.issues.length }})</span>
          <span class="group-pts">{{ pointsTotal(group.totalPoints) }} pt</span>
        </div>
        <a
          v-for="issue in group.issues"
          :key="issue.key"
          :href="issue.url"
          target="_blank"
          rel="noopener noreferrer"
          class="issue-row"
        >
          <span class="issue-key">{{ issue.key }}</span>
          <span class="issue-summary">{{ issue.summary }}</span>
          <span class="issue-status">{{ issue.status }}</span>
          <span class="issue-pts">{{ pts(issue.storyPoints) }}</span>
        </a>
      </div>
    </div>
  </div>
</template>

<style scoped>
.report {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px 16px 16px;
  max-height: 60vh;
  overflow-y: auto;
}

.report-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding-bottom: 6px;
  border-bottom: 1px solid var(--border-subtle);
}

.report-stats {
  display: flex;
  align-items: baseline;
  gap: 6px;
}

.stat {
  display: inline-flex;
  align-items: baseline;
  gap: 4px;
}

.stat-num {
  font-size: 16px;
  font-weight: 600;
  color: var(--text-primary);
  font-family: var(--font-mono);
}

.stat-label {
  font-size: 11px;
  color: var(--text-secondary);
}

.stat-sep {
  color: var(--text-placeholder);
}

.report-badges {
  display: flex;
  gap: 6px;
}

.badge {
  font-size: 10px;
  padding: 2px 8px;
  border-radius: 999px;
  border: 1px solid var(--border-subtle);
  color: var(--text-secondary);
  font-family: var(--font-mono);
  letter-spacing: 0.02em;
}

.badge.ok {
  border-color: rgba(74, 222, 128, 0.35);
  color: #4ade80;
}

.warning {
  font-size: 11px;
  color: #fbbf24;
  font-family: var(--font-mono);
  background: rgba(251, 191, 36, 0.08);
  border: 1px solid rgba(251, 191, 36, 0.25);
  padding: 6px 10px;
  border-radius: var(--radius-sm);
}

.report-body {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.empty {
  text-align: center;
  color: var(--text-secondary);
  font-size: 13px;
  padding: 24px 0;
}

.group {
  display: flex;
  flex-direction: column;
}

.group-header {
  display: flex;
  align-items: baseline;
  gap: 6px;
  padding: 4px 0 4px;
  font-size: 12px;
  font-weight: 600;
  color: var(--text-primary);
  border-bottom: 1px solid var(--border-subtle);
}

.group-header[data-group="in_progress"] { color: #60a5fa; }
.group-header[data-group="review"]      { color: #fbbf24; }
.group-header[data-group="todo"]        { color: var(--text-secondary); }
.group-header[data-group="attention"]   { color: #f87171; }
.group-header[data-group="done"]        { color: #4ade80; }

.group-count {
  color: var(--text-placeholder);
  font-weight: 400;
}

.group-pts {
  margin-left: auto;
  color: var(--text-placeholder);
  font-family: var(--font-mono);
  font-weight: 400;
  font-size: 11px;
}

.issue-row {
  display: grid;
  grid-template-columns: auto 1fr auto auto;
  gap: 10px;
  align-items: baseline;
  padding: 5px 4px;
  font-size: 12px;
  color: var(--text-primary);
  text-decoration: none;
  border-bottom: 1px dashed var(--border-subtle);
}

.issue-row:hover {
  background: var(--bg-hover);
}

.issue-key {
  font-family: var(--font-mono);
  font-weight: 600;
  color: #93c5fd;
  white-space: nowrap;
}

.issue-summary {
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.issue-status {
  color: var(--text-secondary);
  font-size: 11px;
  white-space: nowrap;
}

.issue-pts {
  color: var(--text-primary);
  font-family: var(--font-mono);
  font-weight: 600;
  text-align: right;
  min-width: 24px;
}
</style>
