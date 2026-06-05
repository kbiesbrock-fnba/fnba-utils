<script setup lang="ts">
import { computed } from "vue";
import type { StandupRunResult, JiraIssue } from "@/lib/tauri";
import { setStandupIssuePostToTeams } from "@/lib/tauri";

const props = defineProps<{
  result: StandupRunResult;
  /** Optional override for the "✓ Copied" badge. Falls back to the result's
   *  own `copiedToClipboard` flag when not provided, so older callers still
   *  work. The Standup command passes a local ref driven by the Copy button. */
  copied?: boolean;
}>();

const totalPoints = computed(() =>
  props.result.report.groups.reduce((acc, g) => acc + g.totalPoints, 0),
);

const showCopied = computed(() =>
  props.copied !== undefined ? props.copied : props.result.copiedToClipboard,
);

function pts(n: number | null): string {
  if (n === null || n === undefined) return "—";
  return Number.isInteger(n) ? `${n}` : n.toFixed(1);
}

function pointsTotal(n: number): string {
  return Number.isInteger(n) ? `${n}` : n.toFixed(1);
}

function todoPostedCount(issues: JiraIssue[]): number {
  return issues.filter((i) => i.postToTeams).length;
}

function todoPostedPoints(issues: JiraIssue[]): number {
  return issues
    .filter((i) => i.postToTeams)
    .reduce((acc, i) => acc + (i.storyPoints ?? 0), 0);
}

/** Render the To Do group with starred items on top so the "next up" picks
 *  are immediately visible. Stable within each partition — preserves the
 *  upstream order otherwise. */
function todoIssuesSorted(issues: JiraIssue[]): JiraIssue[] {
  const starred: JiraIssue[] = [];
  const rest: JiraIssue[] = [];
  for (const issue of issues) {
    (issue.postToTeams ? starred : rest).push(issue);
  }
  return [...starred, ...rest];
}

// Optimistic toggle — flip the local flag immediately, then persist. If the
// backend call fails we revert and surface a console error (no toast UI here).
async function togglePost(issue: JiraIssue) {
  const next = !issue.postToTeams;
  issue.postToTeams = next;
  try {
    await setStandupIssuePostToTeams(issue.key, next);
  } catch (e) {
    issue.postToTeams = !next;
    console.error("setStandupIssuePostToTeams failed:", e);
  }
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
        <span v-if="showCopied" class="badge ok">✓ Copied</span>
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
          <span
            v-if="group.group === 'todo'"
            class="group-post-count"
            :title="'Items checked will be included in the Teams post. Unchecked items stay in the preview only.'"
          >posting {{ todoPostedCount(group.issues) }} of {{ group.issues.length }} to Teams</span>
          <span v-if="group.group === 'todo'" class="group-pts">
            {{ pointsTotal(todoPostedPoints(group.issues)) }} of {{ pointsTotal(group.totalPoints) }} pt
          </span>
          <span v-else class="group-pts">{{ pointsTotal(group.totalPoints) }} pt</span>
        </div>

        <template v-if="group.group === 'todo'">
          <div
            v-for="issue in todoIssuesSorted(group.issues)"
            :key="issue.key"
            class="issue-row issue-row-toggle"
            :class="{ 'issue-skipped': !issue.postToTeams }"
          >
            <button
              type="button"
              class="post-toggle"
              :class="{ on: issue.postToTeams }"
              :title="issue.postToTeams ? 'Posted to Teams — click to skip' : 'Preview only — click to include in Teams post'"
              :aria-pressed="issue.postToTeams"
              :aria-label="`Toggle ${issue.key} in Teams post`"
              @click="togglePost(issue)"
            >{{ issue.postToTeams ? '★' : '☆' }}</button>
            <a
              :href="issue.url"
              target="_blank"
              rel="noopener noreferrer"
              class="issue-link"
            >
              <span class="issue-key">{{ issue.key }}</span>
              <span class="issue-summary">{{ issue.summary }}</span>
              <span class="issue-status">{{ issue.status }}</span>
              <span class="issue-pts">{{ pts(issue.storyPoints) }}</span>
            </a>
          </div>
        </template>

        <a
          v-else
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

.group-post-count {
  margin-left: 8px;
  font-size: 10.5px;
  font-weight: 400;
  font-family: var(--font-mono);
  color: var(--text-placeholder);
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

/* Rows in the To Do group carry a leading star button; the rest of the row is
 * a separate <a>. Override the grid so the row holds [button, link] only. */
.issue-row-toggle {
  grid-template-columns: auto 1fr;
  gap: 8px;
  align-items: center;
}

.issue-link {
  display: grid;
  grid-template-columns: auto 1fr auto auto;
  gap: 10px;
  align-items: baseline;
  text-decoration: none;
  color: inherit;
}

.issue-skipped .issue-link {
  opacity: 0.4;
}

.issue-skipped .issue-summary {
  text-decoration: line-through;
  text-decoration-color: var(--text-placeholder);
}

.post-toggle {
  background: transparent;
  border: none;
  cursor: pointer;
  padding: 0 2px;
  font-size: 14px;
  line-height: 1;
  color: var(--text-placeholder);
  font-family: var(--font-mono);
  transition: color 0.1s ease, transform 0.1s ease;
  width: 18px;
  text-align: center;
}

.post-toggle:hover {
  transform: scale(1.15);
}

.post-toggle.on {
  color: #fbbf24;
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
