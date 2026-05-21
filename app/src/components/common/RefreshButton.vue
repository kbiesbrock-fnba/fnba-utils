<script setup lang="ts">
import { onBeforeUnmount, ref } from "vue";

interface Props {
  /** Async refresh handler. Resolve = success badge; reject = error badge. */
  onRefresh: () => Promise<unknown> | unknown;
  title?: string;
  disabled?: boolean;
  /** How long to show the success/error badge before reverting, in ms. */
  resultDuration?: number;
  /** Minimum visible spin duration so fast refreshes still register, in ms. */
  minSpinMs?: number;
}
const props = withDefaults(defineProps<Props>(), {
  title: "Refresh",
  disabled: false,
  resultDuration: 1500,
  minSpinMs: 400,
});

type State = "idle" | "spinning" | "success" | "error";
const state = ref<State>("idle");
let resultTimer: ReturnType<typeof setTimeout> | null = null;

function clearResultTimer() {
  if (resultTimer) {
    clearTimeout(resultTimer);
    resultTimer = null;
  }
}

async function handleClick() {
  if (props.disabled || state.value === "spinning") return;
  clearResultTimer();
  state.value = "spinning";
  const start = performance.now();
  let next: State;
  try {
    await props.onRefresh();
    next = "success";
  } catch {
    next = "error";
  }
  const elapsed = performance.now() - start;
  if (elapsed < props.minSpinMs) {
    await new Promise((r) => setTimeout(r, props.minSpinMs - elapsed));
  }
  state.value = next;
  resultTimer = setTimeout(() => {
    state.value = "idle";
    resultTimer = null;
  }, props.resultDuration);
}

onBeforeUnmount(clearResultTimer);
</script>

<template>
  <button
    class="refresh-btn"
    :class="[`state-${state}`, { disabled }]"
    :title="title"
    :disabled="disabled || state === 'spinning'"
    @click.stop="handleClick"
  >
    <!-- Refresh (idle + spinning) -->
    <svg
      v-if="state === 'idle' || state === 'spinning'"
      viewBox="0 0 16 16"
      fill="currentColor"
      width="11"
      height="11"
    >
      <path d="M11.534 7h3.932a.25.25 0 0 1 .192.41l-1.966 2.36a.25.25 0 0 1-.384 0l-1.966-2.36a.25.25 0 0 1 .192-.41zm-11 2h3.932a.25.25 0 0 0 .192-.41L2.692 6.23a.25.25 0 0 0-.384 0L.342 8.59A.25.25 0 0 0 .534 9z" />
      <path d="M8 3c-1.552 0-2.94.707-3.857 1.818a.5.5 0 1 1-.771-.636A6.002 6.002 0 0 1 13.917 7H12.9A5.002 5.002 0 0 0 8 3zM3.1 9a5.002 5.002 0 0 0 8.757 2.182.5.5 0 1 1 .771.636A6.002 6.002 0 0 1 2.083 9H3.1z" />
    </svg>
    <!-- Success: check -->
    <svg
      v-else-if="state === 'success'"
      viewBox="0 0 16 16"
      fill="currentColor"
      width="12"
      height="12"
    >
      <path d="M13.854 3.646a.5.5 0 0 1 0 .708l-7 7a.5.5 0 0 1-.708 0l-3.5-3.5a.5.5 0 1 1 .708-.708L6.5 10.293l6.646-6.647a.5.5 0 0 1 .708 0z" />
    </svg>
    <!-- Error: X -->
    <svg
      v-else
      viewBox="0 0 16 16"
      fill="currentColor"
      width="11"
      height="11"
    >
      <path d="M3.72 3.72a.75.75 0 0 1 1.06 0L8 6.94l3.22-3.22a.75.75 0 1 1 1.06 1.06L9.06 8l3.22 3.22a.75.75 0 1 1-1.06 1.06L8 9.06l-3.22 3.22a.75.75 0 0 1-1.06-1.06L6.94 8 3.72 4.78a.75.75 0 0 1 0-1.06Z" />
    </svg>
  </button>
</template>

<style scoped>
.refresh-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  padding: 0;
  border: none;
  background: transparent;
  color: var(--text-secondary);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: background 0.1s ease, color 0.15s ease;
}

.refresh-btn:hover:not(.disabled):not(:disabled) {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.refresh-btn.disabled,
.refresh-btn:disabled {
  cursor: default;
}

.refresh-btn.disabled {
  opacity: 0.5;
}

.refresh-btn.state-success {
  color: var(--accent-green, #4ade80);
}

.refresh-btn.state-error {
  color: var(--accent-red, #f87171);
}

@keyframes refresh-btn-spin {
  to { transform: rotate(360deg); }
}

.refresh-btn.state-spinning svg {
  animation: refresh-btn-spin 0.8s linear infinite;
}
</style>
