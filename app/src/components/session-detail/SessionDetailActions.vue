<script setup lang="ts">
import { ref } from "vue";
import { useSessionDetail } from "@/composables/useSessionDetail";

const { kill, openCwd, copyInfo } = useSessionDetail();

const copied = ref(false);
const confirmKill = ref(false);
let killTimer: ReturnType<typeof setTimeout> | null = null;

function handleCopy() {
  copyInfo();
  copied.value = true;
  setTimeout(() => (copied.value = false), 1500);
}

function handleKill() {
  if (!confirmKill.value) {
    confirmKill.value = true;
    killTimer = setTimeout(() => (confirmKill.value = false), 3000);
    return;
  }
  if (killTimer) clearTimeout(killTimer);
  confirmKill.value = false;
  kill();
}
</script>

<template>
  <div class="sd-actions">
    <button class="sd-action" title="Open in Explorer" @click="openCwd">
      <svg viewBox="0 0 16 16" fill="currentColor" width="14" height="14">
        <path d="M1 3.5A1.5 1.5 0 0 1 2.5 2h2.764c.958 0 1.76.56 2.311 1.184C7.985 3.648 8.48 4 9 4h4.5A1.5 1.5 0 0 1 15 5.5v7a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 1 12.5v-9z" />
      </svg>
      <span>Open Folder</span>
    </button>
    <button class="sd-action" :class="{ copied }" title="Copy session info" @click="handleCopy">
      <svg viewBox="0 0 16 16" fill="currentColor" width="14" height="14">
        <path v-if="!copied" d="M0 6.75C0 5.784.784 5 1.75 5h1.5a.75.75 0 0 1 0 1.5h-1.5a.25.25 0 0 0-.25.25v7.5c0 .138.112.25.25.25h7.5a.25.25 0 0 0 .25-.25v-1.5a.75.75 0 0 1 1.5 0v1.5A1.75 1.75 0 0 1 9.25 16h-7.5A1.75 1.75 0 0 1 0 14.25v-7.5z" />
        <path v-if="!copied" d="M5 1.75C5 .784 5.784 0 6.75 0h7.5C15.216 0 16 .784 16 1.75v7.5A1.75 1.75 0 0 1 14.25 11h-7.5A1.75 1.75 0 0 1 5 9.25v-7.5zm1.75-.25a.25.25 0 0 0-.25.25v7.5c0 .138.112.25.25.25h7.5a.25.25 0 0 0 .25-.25v-7.5a.25.25 0 0 0-.25-.25h-7.5z" />
        <path v-if="copied" d="M13.78 4.22a.75.75 0 0 1 0 1.06l-7.25 7.25a.75.75 0 0 1-1.06 0L2.22 9.28a.751.751 0 0 1 .018-1.042.751.751 0 0 1 1.042-.018L6 10.94l6.72-6.72a.75.75 0 0 1 1.06 0z" />
      </svg>
      <span>{{ copied ? "Copied" : "Copy Info" }}</span>
    </button>
    <button
      class="sd-action danger"
      :class="{ confirm: confirmKill }"
      :title="confirmKill ? 'Click again to confirm' : 'Kill session'"
      @click="handleKill"
    >
      <svg viewBox="0 0 16 16" fill="currentColor" width="14" height="14">
        <path d="M3.72 3.72a.75.75 0 0 1 1.06 0L8 6.94l3.22-3.22a.749.749 0 0 1 1.275.326.749.749 0 0 1-.215.734L9.06 8l3.22 3.22a.749.749 0 0 1-.326 1.275.749.749 0 0 1-.734-.215L8 9.06l-3.22 3.22a.751.751 0 0 1-1.042-.018.751.751 0 0 1-.018-1.042L6.94 8 3.72 4.78a.75.75 0 0 1 0-1.06z" />
      </svg>
      <span>{{ confirmKill ? "Confirm Kill" : "Kill" }}</span>
    </button>
  </div>
</template>

<style scoped>
.sd-actions {
  display: flex;
  gap: 6px;
  padding: 10px 14px;
}

.sd-action {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 5px;
  padding: 6px 8px;
  border: 1px solid var(--border-subtle);
  background: transparent;
  color: var(--text-secondary);
  border-radius: var(--radius-sm);
  font-size: 11px;
  cursor: pointer;
  transition: all 0.1s ease;
  white-space: nowrap;
}

.sd-action:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
  border-color: rgba(255, 255, 255, 0.12);
}

.sd-action.copied {
  color: var(--accent-green);
  border-color: rgba(52, 211, 153, 0.3);
}

.sd-action.danger:hover {
  color: var(--accent-red);
  border-color: rgba(248, 113, 113, 0.3);
}

.sd-action.danger.confirm {
  background: rgba(248, 113, 113, 0.15);
  color: var(--accent-red);
  border-color: rgba(248, 113, 113, 0.4);
}
</style>
