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

const { step, cwd, initialPrompt, worktree, error, result, projects, reset, browse, launch, selectRecent, togglePin } =
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

const filteredProjects = computed(() => {
  const q = cwd.value.trim().toLowerCase();
  if (!q) return projects.value;
  return projects.value.filter(
    (p) => p.cwd.toLowerCase().includes(q) || p.displayName.toLowerCase().includes(q),
  );
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
  // Keep these in sync with PANEL_DEFAULTS["session-detail"] in
  // app/src/composables/useMissionControl.ts — both code paths create the
  // same window type.
  const win = new WebviewWindow(label, {
    width: 880,
    height: 760,
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
      <ul v-if="showRecents && filteredProjects.length" class="nc-recents">
        <li
          v-for="p in filteredProjects"
          :key="p.cwd"
          class="nc-recent-item"
        >
          <button
            class="nc-pin"
            :class="{ pinned: p.pinned }"
            :title="p.pinned ? 'Unpin' : 'Pin'"
            @mousedown.prevent="togglePin(p.cwd, !p.pinned)"
          >
            {{ p.pinned ? "★" : "☆" }}
          </button>
          <span class="nc-recent-pick" @mousedown.prevent="selectRecent(p.cwd)">
            <span class="nc-recent-name">{{ p.displayName }}</span>
            <span class="nc-recent-cwd">{{ p.cwd }}</span>
          </span>
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
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 10px;
  color: var(--text-secondary);
}

.nc-recent-item:hover {
  background: rgba(96, 165, 250, 0.06);
}

.nc-recent-pick {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  cursor: pointer;
}

.nc-recent-pick:hover .nc-recent-name {
  color: var(--text-primary);
}

.nc-recent-name {
  font-family: var(--font-sans);
  font-size: 12px;
  color: var(--text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.nc-recent-cwd {
  font-size: 10px;
  color: var(--text-placeholder);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.nc-pin {
  flex-shrink: 0;
  padding: 0 4px;
  background: transparent;
  border: 0;
  color: var(--text-placeholder);
  font-size: 14px;
  cursor: pointer;
  line-height: 1;
}

.nc-pin:hover {
  color: var(--accent-yellow, #fbbf24);
}

.nc-pin.pinned {
  color: var(--accent-yellow, #fbbf24);
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
