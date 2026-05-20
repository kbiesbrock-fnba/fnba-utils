<script setup lang="ts">
import { ref, onMounted } from "vue";
import {
  getStandupLastRun,
  runStandup,
  type StandupLastRun,
  type StandupRunResult,
} from "@/lib/tauri";
import { refreshStandupCommand } from "@/commands";
import { useKeyLayer, KEY_PRIORITY } from "@/composables/useKeyLayer";
import StatusBar from "../StatusBar.vue";
import LoadingView from "../LoadingView.vue";
import ErrorView from "../ErrorView.vue";
import StandupReportView from "./StandupReportView.vue";

const emit = defineEmits<{
  back: [];
  dismiss: [];
}>();

type Step = "idle" | "running" | "result" | "error";

const step = ref<Step>("idle");
const lastRun = ref<StandupLastRun | null>(null);
const result = ref<StandupRunResult | null>(null);
const error = ref<string | null>(null);
const selectedAction = ref<0 | 1>(0); // 0 = post to teams, 1 = fetch only

onMounted(async () => {
  try {
    lastRun.value = await getStandupLastRun();
  } catch (e) {
    // Last-run is best-effort; failing to load it shouldn't block the command.
    console.warn("standup: get_standup_last_run failed", e);
  }
});

async function execute(postToTeams: boolean) {
  step.value = "running";
  error.value = null;
  try {
    result.value = await runStandup(postToTeams);
    step.value = "result";
    // Refresh palette subtitle so "Last run" is current next time.
    void refreshStandupCommand();
    // Update local last-run badge too.
    try {
      lastRun.value = await getStandupLastRun();
    } catch {
      // ignore
    }
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
    step.value = "error";
    void refreshStandupCommand();
  }
}

useKeyLayer(
  [
    {
      key: "ArrowDown",
      handler: () => {
        if (step.value !== "idle") return false;
        selectedAction.value = selectedAction.value === 0 ? 1 : 0;
      },
    },
    {
      key: "ArrowUp",
      handler: () => {
        if (step.value !== "idle") return false;
        selectedAction.value = selectedAction.value === 0 ? 1 : 0;
      },
    },
    {
      key: "Enter",
      handler: () => {
        if (step.value === "idle") {
          void execute(selectedAction.value === 0);
          return;
        }
        if (step.value === "result" || step.value === "error") {
          emit("dismiss");
          return;
        }
        return false;
      },
    },
    {
      key: "Escape",
      handler: () => {
        if (step.value === "result" || step.value === "error") {
          emit("dismiss");
          return;
        }
        return false; // let palette handle back
      },
    },
  ],
  { priority: KEY_PRIORITY.COMMAND },
);

function humanAgo(iso: string): string {
  const t = new Date(iso).getTime();
  if (Number.isNaN(t)) return iso;
  const m = Math.floor((Date.now() - t) / 60_000);
  if (m < 1) return "just now";
  if (m < 60) return `${m}m ago`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h ago`;
  return `${Math.floor(h / 24)}d ago`;
}
</script>

<template>
  <template v-if="step === 'idle'">
    <div class="standup-idle">
      <div class="last-run-card" :class="{ recent: lastRun && !lastRun.error }">
        <template v-if="!lastRun">
          <div class="lr-headline">Standup has not been run yet</div>
        </template>
        <template v-else-if="lastRun.error">
          <div class="lr-headline error">Last run failed</div>
          <div class="lr-meta">{{ humanAgo(lastRun.at) }}</div>
          <div class="lr-error">{{ lastRun.error }}</div>
        </template>
        <template v-else>
          <div class="lr-headline">
            Last run <span class="lr-time">{{ humanAgo(lastRun.at) }}</span>
          </div>
          <div class="lr-meta">
            {{ lastRun.issueCount }} issue{{ lastRun.issueCount === 1 ? '' : 's' }} ·
            {{ lastRun.postedToTeams ? 'posted to Teams' : 'no Teams post' }}
          </div>
        </template>
      </div>

      <div class="action-list" role="listbox">
        <button
          class="action"
          :class="{ selected: selectedAction === 0 }"
          @click="execute(true)"
          @mouseenter="selectedAction = 0"
        >
          <span class="action-icon">📤</span>
          <span class="action-body">
            <span class="action-name">Fetch &amp; Post to Teams</span>
            <span class="action-desc">Pull Jira, copy to clipboard, post Adaptive Card</span>
          </span>
        </button>
        <button
          class="action"
          :class="{ selected: selectedAction === 1 }"
          @click="execute(false)"
          @mouseenter="selectedAction = 1"
        >
          <span class="action-icon">👁</span>
          <span class="action-body">
            <span class="action-name">Fetch &amp; Preview Only</span>
            <span class="action-desc">Pull Jira and show inline. No Teams post.</span>
          </span>
        </button>
      </div>
    </div>
    <StatusBar hint="↑↓ Select  ⏎ Run  ⎋ Back" />
  </template>

  <template v-else-if="step === 'running'">
    <LoadingView message="Fetching Jira tasks..." />
  </template>

  <template v-else-if="step === 'result' && result">
    <StandupReportView :result="result" />
    <StatusBar hint="⏎ Close  ⎋ Close" />
  </template>

  <template v-else-if="step === 'error' && error">
    <ErrorView :error="error" />
    <div class="close-row">
      <button class="confirm-btn" @click="emit('dismiss')">Close</button>
    </div>
    <StatusBar hint="⏎ Close  ⎋ Close" />
  </template>
</template>

<style scoped>
.standup-idle {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 14px 16px;
}

.last-run-card {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  padding: 10px 12px;
  background: var(--bg-hover);
}

.last-run-card.recent {
  border-color: rgba(96, 165, 250, 0.35);
  background: rgba(96, 165, 250, 0.08);
}

.lr-headline {
  font-size: 13px;
  color: var(--text-primary);
  font-weight: 500;
}

.lr-headline.error {
  color: #f87171;
}

.lr-time {
  color: var(--text-secondary);
  font-weight: 400;
  margin-left: 6px;
}

.lr-meta {
  font-size: 11px;
  color: var(--text-secondary);
  margin-top: 2px;
  font-family: var(--font-mono);
}

.lr-error {
  font-size: 11px;
  color: #f87171;
  margin-top: 4px;
  font-family: var(--font-mono);
  white-space: pre-wrap;
}

.action-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.action {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 12px;
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  background: transparent;
  cursor: pointer;
  text-align: left;
  font-family: inherit;
  color: inherit;
  transition: background 0.1s ease, border-color 0.1s ease;
}

.action:hover,
.action.selected {
  background: var(--bg-selected);
  border-color: rgba(96, 165, 250, 0.35);
}

.action-icon {
  font-size: 18px;
  flex-shrink: 0;
  width: 24px;
  text-align: center;
}

.action-body {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.action-name {
  font-size: 13px;
  font-weight: 500;
  color: var(--text-primary);
}

.action-desc {
  font-size: 11px;
  color: var(--text-secondary);
}

.close-row {
  display: flex;
  justify-content: center;
  padding: 0 20px 16px;
}

.confirm-btn {
  padding: 3px 14px;
  border: 1px solid var(--border-subtle);
  background: transparent;
  color: var(--text-secondary);
  border-radius: var(--radius-sm);
  font-size: 11px;
  font-family: var(--font-sans);
  cursor: pointer;
}

.confirm-btn:hover {
  border-color: var(--text-secondary);
  color: var(--text-primary);
}
</style>
