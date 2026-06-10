<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { getIssueDetail, hideWindow, type IssueDetail } from "@/lib/tauri";
import { openExternal } from "@/lib/external";

const detail = ref<IssueDetail | null>(null);
const loading = ref(true);
const error = ref<string | null>(null);
const descriptionExpanded = ref(false);
const specExpanded = ref(false);

function toggleDescription() {
  descriptionExpanded.value = !descriptionExpanded.value;
}

function toggleSpec() {
  specExpanded.value = !specExpanded.value;
}

async function load(key: string) {
  loading.value = true;
  error.value = null;
  try {
    detail.value = await getIssueDetail(key);
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    loading.value = false;
  }
}

function close() {
  void hideWindow();
}

function formatDate(iso: string | null): string {
  if (!iso) return "—";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString(undefined, {
    weekday: "short",
    month: "short",
    day: "numeric",
    year: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
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

function points(n: number | null): string {
  if (n === null || n === undefined) return "—";
  return Number.isInteger(n) ? `${n}` : n.toFixed(1);
}

onMounted(async () => {
  const { listen } = await import("@tauri-apps/api/event");
  await listen<{ key: string }>("issue-detail-open", (e) => {
    // Clear the localStorage handoff once we've consumed the event.
    try { localStorage.removeItem("fnba-utils:issue-detail-pending"); } catch { /* ignore */ }
    void load(e.payload.key);
  });

  // First-open fallback: if the opener wrote the key to localStorage before
  // we subscribed, pick it up now so we don't sit on a blank panel.
  try {
    const pending = localStorage.getItem("fnba-utils:issue-detail-pending");
    if (pending) {
      localStorage.removeItem("fnba-utils:issue-detail-pending");
      void load(pending);
    }
  } catch {
    // ignore
  }
});

async function openInJira() {
  if (!detail.value) return;
  await openExternal(detail.value.url);
}

const checklistTotal = computed(() =>
  (detail.value?.checklist ?? []).filter((i) => !i.isHeader).length,
);
const checkedCount = computed(
  () => (detail.value?.checklist ?? []).filter((i) => !i.isHeader && i.checked).length,
);

function onKeyDown(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.preventDefault();
    close();
  }
}
</script>

<template>
  <div class="detail-panel" tabindex="-1" @keydown="onKeyDown">
    <div class="detail-header">
      <div class="detail-title-row">
        <span v-if="detail" class="detail-key">{{ detail.key }}</span>
        <span v-if="detail" class="pill type" :class="`t-${detail.issueType.toLowerCase()}`">
          {{ detail.issueType }}
        </span>
        <span
          v-if="detail?.priority"
          class="pill priority"
          :class="detail.priority.toLowerCase()"
        >{{ detail.priority }}</span>
        <span class="detail-spacer" />
        <button
          v-if="detail"
          class="open-in-jira"
          @click="openInJira"
        >Open in Jira ↗</button>
        <button class="close-btn" title="Close" @click="close">✕</button>
      </div>
      <div v-if="detail" class="detail-summary">{{ detail.summary }}</div>
    </div>

    <div v-if="loading" class="state-msg">Loading...</div>
    <div v-else-if="error" class="state-msg error">⚠ {{ error }}</div>
    <div v-else-if="detail" class="detail-body">
      <div class="meta-grid">
        <div class="meta-label">Status</div>
        <div class="meta-value">
          <span class="status-dot" :class="`sg-${detail.statusGroup}`" />
          {{ detail.status }}
        </div>

        <div class="meta-label">Assignee</div>
        <div class="meta-value">{{ detail.assignee ?? "—" }}</div>

        <div class="meta-label">Reporter</div>
        <div class="meta-value">{{ detail.reporter ?? "—" }}</div>

        <div class="meta-label">Due</div>
        <div class="meta-value">{{ formatDueAbsolute(detail.dueDate) }}</div>

        <div class="meta-label">Points</div>
        <div class="meta-value">{{ points(detail.storyPoints) }}</div>

        <div class="meta-label">Labels</div>
        <div class="meta-value">
          <span v-if="detail.labels.length === 0">—</span>
          <span v-for="l in detail.labels" :key="l" class="label-chip">{{ l }}</span>
        </div>

        <div class="meta-label">Created</div>
        <div class="meta-value mono">{{ formatDate(detail.created) }}</div>

        <div class="meta-label">Updated</div>
        <div class="meta-value mono">{{ formatDate(detail.updated) }}</div>
      </div>

      <div class="description-section">
        <button
          type="button"
          class="section-title section-toggle"
          :aria-expanded="descriptionExpanded"
          @click="toggleDescription"
        >
          <svg
            class="toggle-chevron"
            :class="{ open: descriptionExpanded }"
            viewBox="0 0 16 16"
            width="10"
            height="10"
            fill="currentColor"
          >
            <path d="M4.646 1.646a.5.5 0 0 1 .708 0l6 6a.5.5 0 0 1 0 .708l-6 6a.5.5 0 0 1-.708-.708L10.293 8 4.646 2.354a.5.5 0 0 1 0-.708z" />
          </svg>
          Description
        </button>
        <template v-if="descriptionExpanded">
          <pre v-if="detail.description.trim()" class="description">{{ detail.description }}</pre>
          <div v-else class="description empty">No description.</div>
        </template>
      </div>

      <div v-if="detail.spec" class="description-section">
        <button
          type="button"
          class="section-title section-toggle"
          :aria-expanded="specExpanded"
          @click="toggleSpec"
        >
          <svg
            class="toggle-chevron"
            :class="{ open: specExpanded }"
            viewBox="0 0 16 16"
            width="10"
            height="10"
            fill="currentColor"
          >
            <path d="M4.646 1.646a.5.5 0 0 1 .708 0l6 6a.5.5 0 0 1 0 .708l-6 6a.5.5 0 0 1-.708-.708L10.293 8 4.646 2.354a.5.5 0 0 1 0-.708z" />
          </svg>
          Specification
        </button>
        <pre v-if="specExpanded" class="description">{{ detail.spec }}</pre>
      </div>

      <div v-if="detail.checklist.length > 0" class="description-section">
        <div class="section-title">
          Checklist
          <span class="checklist-progress">
            {{ checkedCount }} / {{ checklistTotal }}
          </span>
        </div>
        <div class="checklist">
          <template v-for="(item, idx) in detail.checklist" :key="idx">
            <div v-if="item.isHeader" class="checklist-header">{{ item.text }}</div>
            <label v-else class="checklist-item" :class="{ checked: item.checked }">
              <input
                type="checkbox"
                :checked="item.checked"
                disabled
              />
              <span>{{ item.text }}</span>
            </label>
          </template>
        </div>
      </div>

      <!--
        Diagnostic fallback: Smart Checklist field returned something but our
        parser produced no items. Surface the raw value so we can adjust the
        parser without round-tripping through stderr.
      -->
      <div
        v-else-if="detail.checklistRaw && detail.checklistRaw.trim().length > 0"
        class="description-section"
      >
        <div class="section-title">
          Checklist (raw)
          <span class="checklist-progress">parser produced 0 items</span>
        </div>
        <pre class="description">{{ detail.checklistRaw }}</pre>
      </div>
    </div>
  </div>
</template>

<style scoped>
.detail-panel {
  width: 100%;
  height: 100vh;
  background: var(--bg-primary);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: var(--radius-lg);
  display: flex;
  flex-direction: column;
  outline: none;
  overflow: hidden;
}

.detail-header {
  padding: 14px 18px 12px;
  border-bottom: 1px solid var(--border-subtle);
}

.detail-title-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.detail-key {
  font-family: var(--font-mono);
  font-weight: 700;
  color: #93c5fd;
  font-size: 14px;
}

.detail-spacer {
  flex: 1;
}

.open-in-jira {
  font-size: 11px;
  color: var(--accent-blue);
  background: transparent;
  border: none;
  padding: 2px 4px;
  cursor: pointer;
  font-family: inherit;
}

.open-in-jira:hover {
  text-decoration: underline;
}

.close-btn {
  background: transparent;
  border: none;
  color: var(--text-secondary);
  cursor: pointer;
  font-size: 14px;
  padding: 2px 6px;
  border-radius: var(--radius-sm);
}

.close-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.detail-summary {
  margin-top: 8px;
  font-size: 16px;
  font-weight: 500;
  color: var(--text-primary);
  line-height: 1.4;
}

.state-msg {
  display: flex;
  align-items: center;
  justify-content: center;
  flex: 1;
  color: var(--text-secondary);
  font-size: 13px;
}

.state-msg.error {
  color: #f87171;
  font-family: var(--font-mono);
}

.detail-body {
  flex: 1;
  overflow-y: auto;
  padding: 14px 18px 24px;
}

.meta-grid {
  display: grid;
  grid-template-columns: auto 1fr;
  column-gap: 16px;
  row-gap: 6px;
  align-items: baseline;
  font-size: 12px;
  padding-bottom: 16px;
  border-bottom: 1px solid var(--border-subtle);
}

.meta-label {
  font-family: var(--font-mono);
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-placeholder);
}

.meta-value {
  color: var(--text-primary);
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}

.meta-value.mono {
  font-family: var(--font-mono);
  color: var(--text-secondary);
  font-size: 11px;
}

.label-chip {
  font-size: 10px;
  padding: 1px 6px;
  border-radius: 999px;
  background: var(--bg-hover);
  border: 1px solid var(--border-subtle);
  color: var(--text-secondary);
  font-family: var(--font-mono);
}

.description-section {
  margin-top: 16px;
}

.section-title {
  font-family: var(--font-mono);
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-placeholder);
  margin-bottom: 8px;
}

.section-toggle {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  background: transparent;
  border: none;
  cursor: pointer;
  padding: 2px 6px;
  margin-left: -6px; /* keep visual left edge aligned with non-toggle headers */
  border-radius: var(--radius-sm);
  font-family: inherit;
  color: inherit;
  letter-spacing: inherit;
  text-transform: inherit;
  font-size: inherit;
}

.section-toggle:hover {
  color: var(--text-secondary);
  background: var(--bg-hover);
}

.section-toggle:focus-visible {
  outline: 2px solid var(--accent-blue);
  outline-offset: 1px;
}

.toggle-chevron {
  color: var(--text-placeholder);
  transition: transform 0.12s ease;
}

.toggle-chevron.open {
  transform: rotate(90deg);
}

.section-toggle:hover .toggle-chevron {
  color: var(--accent-blue);
}

.description {
  font-family: var(--font-sans);
  font-size: 13px;
  line-height: 1.5;
  color: var(--text-primary);
  white-space: pre-wrap;
  word-wrap: break-word;
  margin: 0;
  background: var(--bg-hover);
  padding: 12px 14px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border-subtle);
}

.description.empty {
  color: var(--text-placeholder);
  font-style: italic;
}

.checklist-progress {
  margin-left: 8px;
  font-family: var(--font-mono);
  font-size: 10px;
  color: var(--text-placeholder);
  font-weight: 400;
  text-transform: none;
  letter-spacing: 0;
}

.checklist {
  display: flex;
  flex-direction: column;
  gap: 4px;
  background: var(--bg-hover);
  padding: 10px 12px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border-subtle);
}

.checklist-header {
  font-family: var(--font-mono);
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-placeholder);
  margin-top: 6px;
  padding-top: 4px;
  border-top: 1px solid var(--border-subtle);
}

.checklist-header:first-child {
  margin-top: 0;
  padding-top: 0;
  border-top: none;
}

.checklist-item {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: var(--text-primary);
  cursor: default;
}

.checklist-item input[type="checkbox"] {
  width: 13px;
  height: 13px;
  margin: 0;
  accent-color: var(--accent-blue);
  cursor: default;
}

.checklist-item.checked span {
  text-decoration: line-through;
  color: var(--text-placeholder);
}

.pill {
  font-size: 10px;
  padding: 1px 8px;
  border-radius: 999px;
  font-family: var(--font-mono);
  letter-spacing: 0.02em;
  white-space: nowrap;
  border: 1px solid transparent;
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

.priority.highest, .priority.high {
  color: #f87171;
  border-color: rgba(248, 113, 113, 0.35);
  background: rgba(248, 113, 113, 0.08);
}

.priority.medium {
  color: #fbbf24;
  border-color: rgba(251, 191, 36, 0.35);
}

.priority.low, .priority.lowest {
  color: var(--text-placeholder);
  border-color: var(--border-subtle);
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
</style>
