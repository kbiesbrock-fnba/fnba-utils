<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import type { SessionDetail } from "@/lib/tauri";
import { updateSessionLabel } from "@/lib/tauri";
import PinButton from "@/components/common/PinButton.vue";
import { displayNameForSession, formatElapsed } from "@/lib/format";

const props = defineProps<{ detail: SessionDetail; pinned: boolean }>();
const emit = defineEmits<{ togglePin: []; labelChanged: [label: string | null] }>();

// Local override of the label so the edit reflects immediately while the
// command round-trips. Falls back to detail.label.
const labelOverride = ref<string | null | undefined>(undefined);
const effectiveLabel = computed(() =>
  labelOverride.value !== undefined ? labelOverride.value : (props.detail.label ?? null),
);

const displayName = computed(() => {
  if (effectiveLabel.value) return effectiveLabel.value;
  return displayNameForSession(props.detail.name, props.detail.cwd);
});

const editing = ref(false);
const draft = ref("");
const labelInput = ref<HTMLInputElement | null>(null);

async function startEdit() {
  draft.value = effectiveLabel.value ?? "";
  editing.value = true;
  await nextTick();
  labelInput.value?.focus();
  labelInput.value?.select();
}

async function commitEdit() {
  if (!editing.value) return;
  editing.value = false;
  const value = draft.value.trim();
  const next = value === "" ? null : value;
  // Optimistic update.
  labelOverride.value = next;
  try {
    await updateSessionLabel(props.detail.sessionId, next);
    emit("labelChanged", next);
  } catch (e) {
    console.warn("[session-header] updateSessionLabel failed", e);
    labelOverride.value = undefined; // revert to whatever detail has
  }
}

function cancelEdit() {
  editing.value = false;
  draft.value = "";
}

function onLabelKey(e: KeyboardEvent) {
  if (e.key === "Enter") {
    e.preventDefault();
    commitEdit();
  } else if (e.key === "Escape") {
    e.preventDefault();
    cancelEdit();
  }
}

const elapsed = computed(() => formatElapsed(props.detail.startedAt));

const statusColor = computed(() => {
  if (props.detail.status === "busy") return "var(--accent-yellow)";
  if (props.detail.status === "dead") return "var(--accent-red)";
  return "var(--accent-green)";
});

// Tmux session name and copy-attach action (Wave 1 — IntelliJ follow-along).
const attachCommand = computed(() => `tmux attach -t claude-${props.detail.sessionId}`);
const copied = ref(false);
async function copyAttach() {
  try {
    await navigator.clipboard.writeText(attachCommand.value);
    copied.value = true;
    setTimeout(() => (copied.value = false), 1200);
  } catch {
    const ta = document.createElement("textarea");
    ta.value = attachCommand.value;
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    try {
      document.execCommand("copy");
      copied.value = true;
      setTimeout(() => (copied.value = false), 1200);
    } catch {
      /* give up */
    }
    document.body.removeChild(ta);
  }
}
</script>

<template>
  <div class="sd-header" data-tauri-drag-region>
    <div class="sd-title-row">
      <input
        v-if="editing"
        ref="labelInput"
        v-model="draft"
        class="sd-name-input"
        spellcheck="false"
        autocomplete="off"
        placeholder="Label this session…"
        @keydown="onLabelKey"
        @blur="commitEdit"
      />
      <span
        v-else
        class="sd-name"
        :title="`${detail.cwd}\n(click to edit label)`"
        @click="startEdit"
      >
        {{ displayName }}
      </span>
      <PinButton :pinned="pinned" @toggle="emit('togglePin')" />
      <div class="sd-status">
        <span class="sd-status-dot" :style="{ background: statusColor }" />
        <span class="sd-status-text">{{ detail.status }}</span>
      </div>
    </div>
    <div class="sd-badges">
      <span class="sd-badge pid">PID {{ detail.pid }}</span>
      <span v-if="detail.gitBranch" class="sd-badge branch">{{ detail.gitBranch }}</span>
      <span v-if="detail.worktreePath" class="sd-badge worktree" :title="detail.worktreePath">worktree</span>
      <span class="sd-badge elapsed">{{ elapsed }}</span>
      <button
        class="sd-badge attach"
        :title="`Copy: ${attachCommand}`"
        @click="copyAttach"
      >
        {{ copied ? "✓ copied" : "tmux attach" }}
      </button>
    </div>
    <div class="sd-cwd" :title="detail.cwd">{{ detail.cwd }}</div>
  </div>
</template>

<style scoped>
.sd-header {
  padding: 12px 14px 10px;
  -webkit-app-region: drag;
}

.sd-title-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.sd-name {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
  cursor: text;
  -webkit-app-region: no-drag;
}

.sd-name:hover {
  color: var(--accent-blue);
}

.sd-name-input {
  flex: 1;
  min-width: 0;
  font-size: 14px;
  font-weight: 600;
  font-family: inherit;
  background: rgba(96, 165, 250, 0.08);
  border: 1px solid rgba(96, 165, 250, 0.4);
  border-radius: var(--radius-sm);
  padding: 2px 6px;
  color: var(--text-primary);
  outline: none;
  -webkit-app-region: no-drag;
}

.sd-status {
  display: flex;
  align-items: center;
  gap: 5px;
  flex-shrink: 0;
}

.sd-status-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
}

.sd-status-text {
  font-size: 11px;
  color: var(--text-secondary);
  text-transform: capitalize;
}

.sd-badges {
  display: flex;
  gap: 6px;
  margin-top: 6px;
  flex-wrap: wrap;
}

.sd-badge {
  font-size: 10px;
  padding: 1px 6px;
  border-radius: 3px;
  font-weight: 500;
  letter-spacing: 0.3px;
}

.sd-badge.pid {
  background: rgba(255, 255, 255, 0.06);
  color: var(--text-secondary);
  text-transform: uppercase;
}

.sd-badge.branch {
  background: rgba(96, 165, 250, 0.15);
  color: var(--accent-blue);
  font-family: var(--font-mono);
}

.sd-badge.elapsed {
  background: rgba(255, 255, 255, 0.06);
  color: var(--text-secondary);
}

.sd-badge.worktree {
  background: rgba(168, 85, 247, 0.15);
  color: rgb(196, 132, 252);
  font-family: var(--font-mono);
}

.sd-badge.attach {
  background: rgba(52, 211, 153, 0.12);
  color: var(--accent-green);
  font-family: var(--font-mono);
  border: 1px solid rgba(52, 211, 153, 0.3);
  cursor: pointer;
  -webkit-app-region: no-drag;
}

.sd-badge.attach:hover {
  background: rgba(52, 211, 153, 0.2);
}

.sd-cwd {
  margin-top: 6px;
  font-size: 11px;
  font-family: var(--font-mono);
  color: var(--text-placeholder);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
