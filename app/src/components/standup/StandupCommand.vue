<script setup lang="ts">
import { ref, computed, onMounted, watch } from "vue";
import {
  previewStandup,
  postStandupToTeams,
  type StandupRunResult,
} from "@/lib/tauri";
import { openExternal } from "@/lib/external";
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

type Step = "loading" | "preview" | "posting" | "posted" | "error";

const step = ref<Step>("loading");
const result = ref<StandupRunResult | null>(null);
const error = ref<string | null>(null);

const teamsConfigured = computed(() => result.value?.teamsConfigured ?? false);
const teamsChannelUrl = computed(() => result.value?.teamsChannelUrl ?? null);

onMounted(() => {
  void fetchPreview();
});

async function fetchPreview() {
  step.value = "loading";
  error.value = null;
  try {
    result.value = await previewStandup();
    step.value = "preview";
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
    step.value = "error";
  }
}

async function postToTeams() {
  if (!result.value || !teamsConfigured.value) return;
  step.value = "posting";
  error.value = null;
  try {
    result.value = await postStandupToTeams(result.value.report);
    step.value = "posted";
    void refreshStandupCommand();
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
    step.value = "error";
  }
}

// Fire openExternal(teamsChannelUrl) exactly once when we enter "posted",
// so Teams pops to the channel.
watch(step, (s) => {
  if (s === "posted" && teamsChannelUrl.value) {
    void openExternal(teamsChannelUrl.value);
  }
});

useKeyLayer(
  [
    {
      key: "Enter",
      handler: () => {
        if (step.value === "preview") {
          if (teamsConfigured.value) void postToTeams();
          return;
        }
        if (step.value === "posted" || step.value === "error") {
          emit("dismiss");
          return;
        }
        return false;
      },
    },
    {
      key: "Escape",
      handler: () => {
        if (step.value === "posted" || step.value === "error") {
          emit("dismiss");
          return;
        }
        return false; // let palette handle back
      },
    },
  ],
  { priority: KEY_PRIORITY.COMMAND },
);
</script>

<template>
  <template v-if="step === 'loading'">
    <LoadingView message="Fetching Jira tasks..." />
  </template>

  <template v-else-if="step === 'preview' && result">
    <StandupReportView :result="result" />
    <div v-if="!teamsConfigured" class="config-hint warn">
      Set <code>standup.teams_webhook_url</code> in <code>~/.fnba-utils/config.yaml</code>
      to enable posting.
    </div>
    <div class="action-row">
      <button class="btn secondary" @click="fetchPreview" title="Re-fetch from Jira">
        ↻ Refresh
      </button>
      <button
        class="btn primary"
        :disabled="!teamsConfigured"
        @click="postToTeams"
      >
        📤 Post to Teams
      </button>
    </div>
    <StatusBar :hint="teamsConfigured ? '⏎ Post  ⎋ Back' : '⎋ Back'" />
  </template>

  <template v-else-if="step === 'posting'">
    <LoadingView message="Posting to Teams..." />
  </template>

  <template v-else-if="step === 'posted' && result">
    <StandupReportView :result="result" />
    <div v-if="!teamsChannelUrl" class="config-hint">
      Set <code>standup.teams_channel_url</code> in <code>~/.fnba-utils/config.yaml</code>
      to auto-open the channel after posting.
    </div>
    <div class="action-row single">
      <button class="btn secondary" @click="emit('dismiss')">Close</button>
    </div>
    <StatusBar hint="⏎ Close  ⎋ Close" />
  </template>

  <template v-else-if="step === 'error' && error">
    <ErrorView :error="error" />
    <div class="action-row">
      <button class="btn secondary" @click="fetchPreview">Retry</button>
      <button class="btn secondary" @click="emit('dismiss')">Close</button>
    </div>
    <StatusBar hint="⏎ Close  ⎋ Close" />
  </template>
</template>

<style scoped>
.action-row {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 8px 16px 14px;
}

.action-row.single {
  justify-content: center;
}

.btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 14px;
  border-radius: var(--radius-sm);
  font-family: var(--font-sans);
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: background 0.1s ease, border-color 0.1s ease, color 0.1s ease;
}

.btn.primary {
  background: rgba(96, 165, 250, 0.18);
  border: 1px solid rgba(96, 165, 250, 0.55);
  color: #93c5fd;
}

.btn.primary:hover:not(:disabled) {
  background: rgba(96, 165, 250, 0.28);
  border-color: rgba(96, 165, 250, 0.8);
  color: #bfdbfe;
}

.btn.primary:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.btn.secondary {
  background: transparent;
  border: 1px solid var(--border-subtle);
  color: var(--text-secondary);
}

.btn.secondary:hover {
  border-color: var(--text-secondary);
  color: var(--text-primary);
}

.config-hint {
  font-size: 11px;
  color: var(--text-secondary);
  padding: 0 16px 4px;
  line-height: 1.5;
}

.config-hint.warn {
  color: #fbbf24;
}

.config-hint code {
  font-family: var(--font-mono);
  font-size: 10.5px;
  color: var(--text-primary);
  background: var(--bg-hover);
  padding: 1px 5px;
  border-radius: 3px;
}
</style>
