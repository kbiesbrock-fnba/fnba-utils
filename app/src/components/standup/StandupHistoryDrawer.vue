<script setup lang="ts">
import type { StandupRunSummary } from "@/lib/tauri";

defineProps<{ history: StandupRunSummary[] }>();

function fmt(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const weekday = d.toLocaleDateString(undefined, { weekday: "short" });
  const time = d.toLocaleTimeString(undefined, {
    hour: "numeric",
    minute: "2-digit",
  });
  const month = d.toLocaleDateString(undefined, { month: "short" });
  return `${weekday} ${month} ${d.getDate()} · ${time}`;
}
</script>

<template>
  <div class="history">
    <div class="history-title">Recent runs</div>
    <div v-if="history.length === 0" class="history-empty">No runs recorded yet.</div>
    <div
      v-for="run in history"
      :key="run.id"
      class="run-row"
      :class="{ failed: !!run.error }"
    >
      <span class="run-time">{{ fmt(run.runAt) }}</span>
      <span class="run-count">{{ run.issueCount }} task{{ run.issueCount === 1 ? '' : 's' }}</span>
      <span class="run-flags">
        <span v-if="run.error" class="flag err" :title="run.error ?? ''">error</span>
        <span v-else-if="run.postedToTeams" class="flag ok">posted</span>
        <span v-else class="flag muted">fetch</span>
      </span>
    </div>
  </div>
</template>

<style scoped>
.history {
  padding: 8px 12px 10px;
  display: flex;
  flex-direction: column;
  gap: 4px;
  max-height: 30vh;
  overflow-y: auto;
}

.history-title {
  font-size: 10px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--text-placeholder);
  padding-bottom: 2px;
}

.history-empty {
  font-size: 11px;
  color: var(--text-placeholder);
  text-align: center;
  padding: 8px 0;
}

.run-row {
  display: grid;
  grid-template-columns: 1fr auto auto;
  gap: 8px;
  align-items: baseline;
  font-size: 11px;
  padding: 3px 4px;
  border-radius: var(--radius-sm);
}

.run-row:hover {
  background: var(--bg-hover);
}

.run-row.failed .run-time,
.run-row.failed .run-count {
  color: #f87171;
}

.run-time {
  color: var(--text-primary);
  font-family: var(--font-mono);
}

.run-count {
  color: var(--text-secondary);
  font-family: var(--font-mono);
}

.flag {
  font-size: 9px;
  padding: 1px 6px;
  border-radius: 999px;
  border: 1px solid var(--border-subtle);
  font-family: var(--font-mono);
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.flag.ok {
  color: #4ade80;
  border-color: rgba(74, 222, 128, 0.35);
}

.flag.err {
  color: #f87171;
  border-color: rgba(248, 113, 113, 0.35);
}

.flag.muted {
  color: var(--text-placeholder);
}
</style>
