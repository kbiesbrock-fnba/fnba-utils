<script setup lang="ts">
import { computed, onMounted, onUnmounted, nextTick, ref } from "vue";
import { useNewClaudeSession } from "@/composables/useNewClaudeSession";
import { useCommandKeys } from "@/composables/useCommandKeys";
import StatusBar from "../StatusBar.vue";
import LoadingView from "../LoadingView.vue";
import ErrorView from "../ErrorView.vue";
import { isTauri } from "@/lib/tauri";
import { hashStr } from "@/lib/hash";

const emit = defineEmits<{
  back: [];
  dismiss: [];
}>();

const { step, cwd, initialPrompt, worktree, error, result, recents, reset, browse, launch, selectRecent } =
  useNewClaudeSession();

const cwdInput = ref<HTMLInputElement | null>(null);
const showRecents = ref(false);

function hideRecentsSoon() {
  // Defer so a mousedown on a list item can fire selectRecent before we hide.
  setTimeout(() => (showRecents.value = false), 150);
}

onMounted(async () => {
  reset();
  await nextTick();
  cwdInput.value?.focus();
});

onUnmounted(() => reset());

const filteredRecents = computed(() => {
  const q = cwd.value.trim().toLowerCase();
  if (!q) return recents.value;
  return recents.value.filter((p) => p.toLowerCase().includes(q));
});

async function openSessionDetail(sessionId: string, sessionCwd: string, pid: number) {
  if (!isTauri) return;
  // Mirror useMissionControl's openOrFocusPanel pattern, but we don't need the
  // full positioning logic here — let Mission Control handle that on next focus.
  const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
  const label = `session-detail:${hashStr(sessionId)}`;
  const existing = await WebviewWindow.getByLabel(label);
  if (existing) {
    await existing.show();
    await existing.setFocus();
    return;
  }
  const params = new URLSearchParams({
    sessionId,
    cwd: sessionCwd,
    pid: String(pid),
  });
  const url = `index.html#session-detail?${params.toString()}`;
  const win = new WebviewWindow(label, {
    width: 440,
    height: 640,
    minWidth: 360,
    minHeight: 400,
    resizable: true,
    decorations: false,
    shadow: false,
    transparent: true,
    backgroundColor: "#00000000",
    visible: false,
    alwaysOnTop: true,
    skipTaskbar: true,
    title: "Session Detail",
    url,
  });
  await win.once("tauri://created", async () => {
    await win.show();
    await win.setFocus();
  });
}

async function onLaunch() {
  await launch();
  if (step.value === "done" && result.value) {
    await openSessionDetail(result.value.sessionId, result.value.cwd, result.value.pid);
    emit("dismiss");
  }
}

useCommandKeys({
  step,
  goBack: () => {
    if (step.value === "error") {
      reset();
      return true;
    }
    return false;
  },
  emitBack: () => emit("back"),
  emitDismiss: () => emit("dismiss"),
  escapeDismissSteps: ["form"],
  enterActions: {
    form: onLaunch,
    error: () => {
      reset();
    },
  },
});

defineExpose({ step });
</script>

<template>
  <template v-if="step === 'form'">
    <div class="nc-form">
      <label class="nc-label">Working Directory</label>
      <div class="nc-cwd-row">
        <input
          ref="cwdInput"
          v-model="cwd"
          class="nc-input"
          placeholder="/mnt/c/dev/your-project"
          spellcheck="false"
          autocomplete="off"
          @focus="showRecents = true"
          @blur="hideRecentsSoon"
        />
        <button class="nc-browse" type="button" @mousedown.prevent="browse">Browse…</button>
      </div>
      <ul v-if="showRecents && filteredRecents.length" class="nc-recents">
        <li
          v-for="path in filteredRecents"
          :key="path"
          class="nc-recent-item"
          @mousedown.prevent="selectRecent(path)"
        >
          {{ path }}
        </li>
      </ul>

      <label class="nc-label nc-label-spaced">Initial Prompt (optional)</label>
      <textarea
        v-model="initialPrompt"
        class="nc-textarea"
        rows="3"
        placeholder="Skip with Enter, or type a message to send immediately."
        spellcheck="false"
      />

      <label class="nc-checkbox">
        <input type="checkbox" v-model="worktree" />
        <span>Launch in a fresh git worktree</span>
      </label>

      <div v-if="error" class="nc-inline-error">{{ error }}</div>

      <button class="nc-launch" type="button" @click="onLaunch">Launch</button>
    </div>
    <StatusBar hint="⏎ Launch  ⎋ Cancel" />
  </template>

  <template v-else-if="step === 'launching'">
    <LoadingView :message="`Starting Claude in ${cwd}...`" />
  </template>

  <template v-else-if="step === 'error'">
    <ErrorView :error="error ?? 'Launch failed'" />
    <div class="nc-close-row">
      <button class="nc-launch" type="button" @click="reset">Try again</button>
    </div>
    <StatusBar hint="⏎ Retry  ⎋ Cancel" />
  </template>
</template>

<style scoped>
.nc-form {
  padding: 18px 20px 8px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.nc-label {
  font-size: 11px;
  font-weight: 600;
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.08em;
}

.nc-label-spaced {
  margin-top: 10px;
}

.nc-cwd-row {
  display: flex;
  gap: 6px;
  align-items: center;
}

.nc-input,
.nc-textarea {
  flex: 1;
  padding: 6px 10px;
  background: var(--bg-secondary, rgba(255, 255, 255, 0.04));
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  font-family: var(--font-mono);
  font-size: 12px;
  outline: none;
  resize: vertical;
}

.nc-input:focus,
.nc-textarea:focus {
  border-color: rgba(96, 165, 250, 0.5);
}

.nc-browse {
  padding: 6px 12px;
  background: transparent;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  font-size: 11px;
  cursor: pointer;
}

.nc-browse:hover {
  border-color: var(--text-secondary);
  color: var(--text-primary);
}

.nc-recents {
  list-style: none;
  margin: 0;
  padding: 4px 0;
  background: var(--bg-secondary, rgba(255, 255, 255, 0.04));
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  max-height: 140px;
  overflow-y: auto;
  font-family: var(--font-mono);
  font-size: 11px;
}

.nc-recent-item {
  padding: 4px 10px;
  color: var(--text-secondary);
  cursor: pointer;
}

.nc-recent-item:hover {
  background: rgba(96, 165, 250, 0.08);
  color: var(--text-primary);
}

.nc-checkbox {
  display: flex;
  gap: 8px;
  align-items: center;
  font-size: 12px;
  color: var(--text-secondary);
  margin-top: 8px;
  cursor: pointer;
  user-select: none;
}

.nc-checkbox input {
  accent-color: var(--accent-blue, #60a5fa);
}

.nc-inline-error {
  margin-top: 6px;
  font-size: 11px;
  color: var(--accent-red, #f87171);
}

.nc-launch {
  margin-top: 14px;
  align-self: center;
  padding: 6px 18px;
  background: rgba(96, 165, 250, 0.12);
  border: 1px solid rgba(96, 165, 250, 0.4);
  color: var(--accent-blue, #60a5fa);
  border-radius: var(--radius-sm);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
}

.nc-launch:hover {
  background: rgba(96, 165, 250, 0.2);
}

.nc-close-row {
  display: flex;
  justify-content: center;
  padding: 0 20px 16px;
}
</style>
