<script setup lang="ts">
import type { JiraIssue, IssueDetail } from "@/lib/tauri";

defineProps<{
  issue: JiraIssue;
  detail: IssueDetail | undefined;
  loading: boolean;
  error: string | undefined;
}>();

const emit = defineEmits<{
  openJira: [url: string, event: Event];
  openDetail: [key: string];
}>();

function statusGroupClass(g: string): string {
  return `sg-${g}`;
}

function formatPoints(pts: number | null): string {
  if (pts === null || pts === undefined) return "";
  return Number.isInteger(pts) ? `${pts}` : pts.toFixed(1);
}

function formatDueAbsolute(iso: string | null): string {
  if (!iso) return "—";
  const [y, m, d] = iso.split("-").map((n) => parseInt(n, 10));
  if (!y || !m || !d) return iso;
  const date = new Date(y, m - 1, d);
  return date.toLocaleDateString(undefined, {
    weekday: "short",
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

function isOverdue(iso: string | null): boolean {
  if (!iso) return false;
  const [y, m, d] = iso.split("-").map((n) => parseInt(n, 10));
  if (!y || !m || !d) return false;
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const due = new Date(y, m - 1, d);
  return due.getTime() < today.getTime();
}
</script>

<template>
  <div class="task-detail">
    <div class="detail-row">
      <span class="detail-label">Status</span>
      <span class="status-dot" :class="statusGroupClass(issue.statusGroup)" />
      <span>{{ issue.status }}</span>
    </div>
    <div class="detail-row">
      <span class="detail-label">Priority</span>
      <span>{{ issue.priority ?? "—" }}</span>
    </div>
    <div class="detail-row">
      <span class="detail-label">Type</span>
      <span>{{ issue.issueType }}</span>
    </div>
    <div class="detail-row">
      <span class="detail-label">Due</span>
      <span :class="{ overdue: isOverdue(issue.dueDate) }">
        {{ formatDueAbsolute(issue.dueDate) }}
      </span>
    </div>
    <div class="detail-row">
      <span class="detail-label">Points</span>
      <span>{{ formatPoints(issue.storyPoints) || "—" }}</span>
    </div>

    <div v-if="loading" class="detail-block-loading">Loading description…</div>
    <div v-else-if="error" class="detail-block-error">⚠ {{ error }}</div>
    <div v-else-if="detail" class="detail-block">
      <div class="detail-block-label">Description</div>
      <pre
        v-if="detail.description.trim()"
        class="detail-block-body"
      >{{ detail.description }}</pre>
      <div v-else class="detail-block-empty">No description.</div>
    </div>

    <div class="detail-actions">
      <button class="detail-link" @click.stop="emit('openJira', issue.url, $event)">
        Open in Jira ↗
      </button>
      <button class="detail-link" @click.stop="emit('openDetail', issue.key)">
        View full task →
      </button>
    </div>
  </div>
</template>

<style scoped>
.task-detail {
  grid-column: 1 / -1;
  padding: 8px 12px 10px 40px;
  background: rgba(96, 165, 250, 0.04);
  border-left: 2px solid var(--accent-blue);
  border-bottom: 1px solid var(--border-subtle);
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 11px;
  color: var(--text-secondary);
}

.detail-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.detail-label {
  font-family: var(--font-mono);
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-placeholder);
  min-width: 60px;
}

.detail-row .overdue {
  color: #f87171;
  font-weight: 600;
}

.status-dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 999px;
  background: var(--text-placeholder);
}

.sg-in_progress { background: #60a5fa; }
.sg-review      { background: #fbbf24; }
.sg-todo        { background: var(--text-secondary); }
.sg-attention   { background: #f87171; }
.sg-done        { background: #4ade80; }

.detail-block {
  margin-top: 6px;
}

.detail-block-label {
  font-family: var(--font-mono);
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-placeholder);
  margin-bottom: 4px;
}

.detail-block-body {
  font-family: var(--font-sans);
  font-size: 12px;
  line-height: 1.45;
  color: var(--text-primary);
  white-space: pre-wrap;
  word-wrap: break-word;
  background: var(--bg-hover);
  border: 1px solid var(--border-subtle);
  padding: 8px 10px;
  border-radius: var(--radius-sm);
  margin: 0;
  max-height: 180px;
  overflow-y: auto;
}

.detail-block-empty {
  font-style: italic;
  color: var(--text-placeholder);
  font-size: 11px;
}

.detail-block-loading {
  font-style: italic;
  color: var(--text-placeholder);
  font-size: 11px;
  padding: 4px 0;
}

.detail-block-error {
  color: #f87171;
  font-size: 11px;
  font-family: var(--font-mono);
  padding: 4px 0;
}

.detail-actions {
  display: flex;
  gap: 12px;
  margin-top: 6px;
}

.detail-link {
  color: var(--accent-blue);
  text-decoration: none;
  font-size: 11px;
  background: transparent;
  border: none;
  padding: 0;
  cursor: pointer;
  font-family: inherit;
}

.detail-link:hover {
  text-decoration: underline;
}
</style>
