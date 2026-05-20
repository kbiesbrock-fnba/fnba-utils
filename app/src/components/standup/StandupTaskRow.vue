<script setup lang="ts">
import type { JiraIssue } from "@/lib/tauri";

import { computed } from "vue";

const props = defineProps<{
  issue: JiraIssue;
  index: number;
  section: "bugs" | "tasks";
  completed: boolean;
  expanded: boolean;
  checklistExpanded: boolean;
  dropTarget: boolean;
}>();

const emit = defineEmits<{
  toggleExpand: [key: string];
  openDetail: [key: string];
  toggleCompleted: [key: string, completed: boolean];
  toggleChecklist: [key: string];
  dragStart: [event: DragEvent, section: "bugs" | "tasks", index: number, key: string];
  dragOver: [event: DragEvent, section: "bugs" | "tasks", index: number];
  dragLeave: [];
  drop: [event: DragEvent, section: "bugs" | "tasks", index: number];
  dragEnd: [];
}>();

const checklistCount = computed(() => {
  const items = props.issue.checklist ?? [];
  const total = items.filter((i) => !i.isHeader).length;
  const done = items.filter((i) => !i.isHeader && i.checked).length;
  return { total, done };
});

function keyWithPoints(key: string, pts: number | null): string {
  if (pts === null || pts === undefined) return key;
  const p = Number.isInteger(pts) ? `${pts}` : pts.toFixed(1);
  return `${key} (${p})`;
}

function shortType(t: string): string {
  if (!t) return "";
  const lower = t.toLowerCase();
  if (lower === "sub-task" || lower === "subtask") return "Sub";
  if (lower === "story") return "Story";
  if (lower === "bug") return "Bug";
  if (lower === "epic") return "Epic";
  if (lower === "incident") return "Incident";
  return t.length > 6 ? `${t.slice(0, 5)}…` : t;
}

function showTypePill(t: string): boolean {
  return !!t && t.toLowerCase() !== "task";
}

function priorityClass(rank: number): string {
  if (rank <= 2) return "p-high";
  if (rank <= 3) return "p-med";
  return "p-low";
}

function formatDueShort(iso: string | null): string {
  if (!iso) return "";
  const [y, m, d] = iso.split("-").map((n) => parseInt(n, 10));
  if (!y || !m || !d) return iso;
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const due = new Date(y, m - 1, d);
  const diffDays = Math.round((due.getTime() - today.getTime()) / 86_400_000);
  if (diffDays === 0) return "today";
  if (diffDays === 1) return "tomorrow";
  if (diffDays === -1) return "yesterday";
  if (diffDays < 0) return `${-diffDays}d late`;
  if (diffDays < 7) return `in ${diffDays}d`;
  return `${m}/${d}`;
}

function dueClass(iso: string | null): string {
  if (!iso) return "";
  const [y, m, d] = iso.split("-").map((n) => parseInt(n, 10));
  if (!y || !m || !d) return "";
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const due = new Date(y, m - 1, d);
  const diff = Math.round((due.getTime() - today.getTime()) / 86_400_000);
  if (diff < 0) return "overdue";
  if (diff <= 1) return "due-soon";
  return "";
}
</script>

<template>
  <div
    class="task-row"
    :class="{
      completed: completed,
      expanded: expanded,
      'drop-target': dropTarget,
    }"
    :data-status="issue.statusGroup"
    @click="emit('toggleExpand', issue.key)"
    @dblclick="emit('openDetail', issue.key)"
    draggable="true"
    @dragstart="emit('dragStart', $event, section, index, issue.key)"
    @dragover="emit('dragOver', $event, section, index)"
    @dragleave="emit('dragLeave')"
    @drop="emit('drop', $event, section, index)"
    @dragend="emit('dragEnd')"
  >
    <input
      type="checkbox"
      class="checkbox"
      :checked="completed"
      @change="emit('toggleCompleted', issue.key, completed)"
      @click.stop
    />
    <span class="cell-key">{{ keyWithPoints(issue.key, issue.storyPoints) }}</span>
    <span class="cell-type">
      <span
        v-if="showTypePill(issue.issueType)"
        class="pill type"
        :class="`t-${issue.issueType.toLowerCase()}`"
      >{{ shortType(issue.issueType) }}</span>
    </span>
    <span class="cell-checklist">
      <svg
        v-if="issue.hasChecklist"
        viewBox="0 0 16 16"
        width="13"
        height="13"
        fill="currentColor"
        class="checklist-icon"
        :title="'Has Smart Checklist'"
      >
        <path d="M2 2.5A1.5 1.5 0 013.5 1h9A1.5 1.5 0 0114 2.5v11a1.5 1.5 0 01-1.5 1.5h-9A1.5 1.5 0 012 13.5v-11zM3.5 2a.5.5 0 00-.5.5v11a.5.5 0 00.5.5h9a.5.5 0 00.5-.5v-11a.5.5 0 00-.5-.5h-9z" />
        <path d="M5.5 6.5a.5.5 0 00-.5.5v.5a.5.5 0 00.5.5h5a.5.5 0 00.5-.5V7a.5.5 0 00-.5-.5h-5zM5 9a.5.5 0 01.5-.5h5a.5.5 0 010 1h-5A.5.5 0 015 9zm.5 1.5a.5.5 0 000 1h3a.5.5 0 000-1h-3z" />
      </svg>
    </span>
    <span class="cell-summary" :title="issue.summary">{{ issue.summary }}</span>
    <span class="cell-meta">
      <span
        v-if="issue.priority"
        class="pill priority"
        :class="priorityClass(issue.priorityRank)"
        :title="`Priority: ${issue.priority}`"
      >{{ issue.priority }}</span>
      <span
        v-if="issue.dueDate"
        class="pill due"
        :class="dueClass(issue.dueDate)"
        :title="`Due ${issue.dueDate}`"
      >{{ formatDueShort(issue.dueDate) }}</span>
    </span>
    <span
      class="drag-handle"
      title="Drag to reorder"
      @click.stop
      @mousedown.stop
    >⋮⋮</span>
  </div>

  <!--
    Smart Checklist: collapsed by default behind a toggle row. Click to expand,
    then each item renders as its own sub-row.
  -->
  <template v-if="issue.hasChecklist && issue.checklist.length > 0">
    <button
      type="button"
      class="checklist-toggle"
      :class="{ 'parent-completed': completed, expanded: checklistExpanded }"
      :data-status="issue.statusGroup"
      :aria-expanded="checklistExpanded"
      :aria-label="`Toggle Smart Checklist for ${issue.key}`"
      @click.stop="emit('toggleChecklist', issue.key)"
    >
      <span class="cell-sub-spacer" />
      <span class="cell-sub-glyph">└</span>
      <span class="cell-sub-spacer" />
      <span class="cell-sub-spacer" />
      <span class="cell-toggle-content">
        <svg
          class="toggle-chevron"
          :class="{ open: checklistExpanded }"
          viewBox="0 0 16 16"
          width="10"
          height="10"
          fill="currentColor"
        >
          <path d="M4.646 1.646a.5.5 0 0 1 .708 0l6 6a.5.5 0 0 1 0 .708l-6 6a.5.5 0 0 1-.708-.708L10.293 8 4.646 2.354a.5.5 0 0 1 0-.708z" />
        </svg>
        <span class="toggle-label">Toggle Smart Checklist</span>
        <span class="toggle-count">
          {{ checklistCount.done }}/{{ checklistCount.total }}
        </span>
      </span>
      <span class="cell-sub-spacer" />
      <span class="cell-sub-spacer" />
    </button>

    <template v-if="checklistExpanded">
      <div
        v-for="(item, i) in issue.checklist"
        :key="`${issue.key}-cl-${i}`"
        class="checklist-subrow"
        :class="{
          'is-header': item.isHeader,
          'is-checked': item.checked,
          'parent-completed': completed,
        }"
        :data-status="issue.statusGroup"
      >
        <span class="cell-sub-spacer" />
        <span class="cell-sub-glyph">{{ item.isHeader ? '' : '·' }}</span>
        <span class="cell-sub-spacer" />
        <span class="cell-sub-spacer" />
        <span class="cell-sub-content">
          <input
            v-if="!item.isHeader"
            type="checkbox"
            class="sub-checkbox"
            :checked="item.checked"
            disabled
            @click.stop
          />
          <span :class="{ 'sub-header': item.isHeader, 'sub-text': !item.isHeader }">{{ item.text }}</span>
        </span>
        <span class="cell-sub-spacer" />
        <span class="cell-sub-spacer" />
      </div>
    </template>
  </template>
</template>

<style scoped>
.task-row {
  grid-column: 1 / -1;
  display: grid;
  grid-template-columns: subgrid;
  align-items: center;
  padding: 0 12px;
  border-left: 2px solid transparent;
  cursor: pointer;
  user-select: none;
  font-size: 12px;
  height: 32px;
  min-height: 32px;
  max-height: 32px;
  overflow: hidden;
}

.task-row[data-status="in_progress"] { border-left-color: #60a5fa; }
.task-row[data-status="review"]      { border-left-color: #fbbf24; }
.task-row[data-status="todo"]        { border-left-color: var(--border-subtle); }
.task-row[data-status="attention"]   { border-left-color: #f87171; }
.task-row[data-status="done"]        { border-left-color: #4ade80; }

.task-row:hover { background: var(--bg-hover); }
.task-row.expanded { background: var(--bg-selected); }

.task-row.completed .cell-key,
.task-row.completed .cell-summary {
  text-decoration: line-through;
  color: var(--text-placeholder);
}
.task-row.completed .pill,
.task-row.completed .checklist-icon {
  opacity: 0.5;
}

.task-row.drop-target {
  box-shadow: inset 0 2px 0 0 var(--accent-blue);
}

.checkbox {
  width: 14px;
  height: 14px;
  margin: 0;
  cursor: pointer;
  accent-color: var(--accent-blue);
  justify-self: center;
}

.cell-key {
  font-family: var(--font-mono);
  font-weight: 600;
  color: #93c5fd;
  white-space: nowrap;
  font-size: 11px;
}

.cell-type {
  display: inline-flex;
  align-items: center;
  min-width: 0;
}

.cell-checklist {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  /* Reserved space whether icon is present or not. */
  width: 16px;
  height: 16px;
}

.checklist-icon {
  color: var(--accent-blue);
  opacity: 0.85;
}

.cell-summary {
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
}

.cell-meta {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  white-space: nowrap;
}

.drag-handle {
  color: var(--text-secondary);
  font-size: 12px;
  letter-spacing: -1px;
  cursor: grab;
  user-select: none;
  opacity: 0.55;
  transition: opacity 0.1s ease, color 0.1s ease;
  padding: 2px 4px;
  line-height: 1;
  border-radius: var(--radius-sm);
  justify-self: end;
}

.task-row:hover .drag-handle,
.task-row.expanded .drag-handle { opacity: 0.85; }

.drag-handle:hover {
  opacity: 1;
  color: var(--accent-blue);
  background: var(--bg-selected);
}

.drag-handle:active { cursor: grabbing; }

.pill {
  font-size: 10px;
  padding: 1px 6px;
  border-radius: 999px;
  font-family: var(--font-mono);
  letter-spacing: 0.02em;
  white-space: nowrap;
  border: 1px solid transparent;
  line-height: 1.4;
}

.type {
  color: var(--text-secondary);
  background: var(--bg-hover);
  border-color: var(--border-subtle);
}
.type.t-bug {
  color: #f87171;
  background: rgba(248, 113, 113, 0.1);
  border-color: rgba(248, 113, 113, 0.3);
}
.type.t-story {
  color: #4ade80;
  background: rgba(74, 222, 128, 0.08);
  border-color: rgba(74, 222, 128, 0.3);
}
.type.t-epic {
  color: #c084fc;
  background: rgba(192, 132, 252, 0.08);
  border-color: rgba(192, 132, 252, 0.3);
}
.type.t-task {
  color: #60a5fa;
  background: rgba(96, 165, 250, 0.08);
  border-color: rgba(96, 165, 250, 0.3);
}

.priority.p-high {
  color: #f87171;
  border-color: rgba(248, 113, 113, 0.35);
  background: rgba(248, 113, 113, 0.08);
}
.priority.p-med {
  color: #fbbf24;
  border-color: rgba(251, 191, 36, 0.35);
  background: rgba(251, 191, 36, 0.06);
}
.priority.p-low {
  color: var(--text-placeholder);
  border-color: var(--border-subtle);
}

.due {
  color: var(--text-secondary);
  border-color: var(--border-subtle);
}
.due.due-soon {
  color: #fbbf24;
  border-color: rgba(251, 191, 36, 0.35);
}
.due.overdue {
  color: #f87171;
  border-color: rgba(248, 113, 113, 0.45);
  background: rgba(248, 113, 113, 0.08);
  font-weight: 600;
}

/* --- Checklist toggle (the collapsible header) --- */

.checklist-toggle {
  grid-column: 1 / -1;
  display: grid;
  grid-template-columns: subgrid;
  align-items: center;
  height: 24px;
  min-height: 24px;
  max-height: 24px;
  padding: 0 12px;
  font-size: 11px;
  border: none;
  border-left: 2px solid transparent;
  cursor: pointer;
  user-select: none;
  color: var(--text-secondary);
  background: rgba(96, 165, 250, 0.03);
  font-family: inherit;
  text-align: left;
  width: 100%;
}

.checklist-toggle:focus-visible {
  outline: 2px solid var(--accent-blue);
  outline-offset: -2px;
  background: rgba(96, 165, 250, 0.1);
}

.checklist-toggle[data-status="in_progress"] { border-left-color: rgba(96, 165, 250, 0.4); }
.checklist-toggle[data-status="review"]      { border-left-color: rgba(251, 191, 36, 0.4); }
.checklist-toggle[data-status="todo"]        { border-left-color: var(--border-subtle); }
.checklist-toggle[data-status="attention"]   { border-left-color: rgba(248, 113, 113, 0.4); }
.checklist-toggle[data-status="done"]        { border-left-color: rgba(74, 222, 128, 0.4); }

.checklist-toggle:hover {
  background: rgba(96, 165, 250, 0.08);
  color: var(--text-primary);
}

.checklist-toggle.expanded {
  background: rgba(96, 165, 250, 0.06);
}

.cell-toggle-content {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.toggle-chevron {
  color: var(--text-placeholder);
  transition: transform 0.12s ease;
}

.toggle-chevron.open {
  transform: rotate(90deg);
}

.checklist-toggle:hover .toggle-chevron {
  color: var(--accent-blue);
}

.toggle-label {
  font-weight: 500;
  letter-spacing: 0.01em;
}

.toggle-count {
  font-family: var(--font-mono);
  font-size: 10px;
  color: var(--text-placeholder);
  padding: 1px 6px;
  border: 1px solid var(--border-subtle);
  border-radius: 999px;
}

.checklist-toggle.parent-completed {
  opacity: 0.5;
}

/* --- Checklist sub-rows --- */

.checklist-subrow {
  grid-column: 1 / -1;
  display: grid;
  grid-template-columns: subgrid;
  align-items: center;
  height: 22px;
  min-height: 22px;
  max-height: 22px;
  padding: 0 12px;
  font-size: 11px;
  border-left: 2px solid transparent;
  background: rgba(255, 255, 255, 0.015);
}

.checklist-subrow[data-status="in_progress"] { border-left-color: rgba(96, 165, 250, 0.4); }
.checklist-subrow[data-status="review"]      { border-left-color: rgba(251, 191, 36, 0.4); }
.checklist-subrow[data-status="todo"]        { border-left-color: var(--border-subtle); }
.checklist-subrow[data-status="attention"]   { border-left-color: rgba(248, 113, 113, 0.4); }
.checklist-subrow[data-status="done"]        { border-left-color: rgba(74, 222, 128, 0.4); }

.checklist-subrow:hover { background: var(--bg-hover); }

.cell-sub-spacer {
  /* Empty placeholder to keep subgrid columns aligned. */
}

.cell-sub-glyph {
  color: var(--text-placeholder);
  font-family: var(--font-mono);
  font-size: 10px;
  text-align: center;
  user-select: none;
}

.cell-sub-content {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  overflow: hidden;
}

.sub-checkbox {
  width: 12px;
  height: 12px;
  margin: 0;
  accent-color: var(--accent-blue);
  cursor: default;
  flex-shrink: 0;
}

.sub-text {
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.sub-header {
  color: var(--text-placeholder);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  font-size: 10px;
  font-weight: 600;
}

.checklist-subrow.is-checked .sub-text {
  text-decoration: line-through;
  color: var(--text-placeholder);
}

.checklist-subrow.parent-completed .sub-text,
.checklist-subrow.parent-completed .sub-header,
.checklist-subrow.parent-completed .cell-sub-glyph {
  opacity: 0.4;
}
</style>
