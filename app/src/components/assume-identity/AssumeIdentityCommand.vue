<script setup lang="ts">
import { onMounted, onUnmounted } from "vue";
import { useAssumeIdentity } from "../../composables/useAssumeIdentity";
import CommandInput from "../CommandInput.vue";
import StatusBar from "../StatusBar.vue";
import UserPicker from "./UserPicker.vue";
import ConnectionPicker from "./ConnectionPicker.vue";
import AssumeIdentityResult from "./AssumeIdentityResult.vue";

const emit = defineEmits<{
  back: [];
  dismiss: [];
}>();

const {
  step,
  users,
  connections,
  selectedUser,
  selectedConnection,
  result,
  error,
  loading,
  loadData,
  reset,
  selectUser,
  selectConnection,
  execute,
  goBack,
} = useAssumeIdentity();

onMounted(() => {
  reset();
  loadData();
});

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.preventDefault();
    e.stopPropagation();
    if (step.value === "result" || step.value === "error") {
      emit("dismiss");
    } else if (!goBack()) {
      emit("back");
    }
  }
  if (e.key === "Enter") {
    if (step.value === "confirm") {
      e.preventDefault();
      e.stopPropagation();
      execute();
    }
    if (step.value === "result" || step.value === "error") {
      e.preventDefault();
      e.stopPropagation();
      emit("dismiss");
    }
  }
}

onMounted(() => window.addEventListener("keydown", onKeydown, true));
onUnmounted(() => window.removeEventListener("keydown", onKeydown, true));
</script>

<template>
  <template v-if="step === 'user'">
    <UserPicker :users="users" @select="selectUser" />
    <StatusBar hint="↑↓ Navigate  ⏎ Select  ⎋ Back" />
  </template>

  <template v-else-if="step === 'connection'">
    <ConnectionPicker :connections="connections" @select="selectConnection" />
    <StatusBar hint="↑↓ Navigate  ⏎ Select  ⎋ Back" />
  </template>

  <template v-else-if="step === 'confirm'">
    <div class="confirm-view">
      <div class="confirm-header">Assume Identity</div>
      <div class="confirm-row">
        <span class="confirm-label">User</span>
        <span class="confirm-value">{{ selectedUser?.username }}</span>
        <span class="confirm-meta">{{ selectedUser?.labels }}</span>
      </div>
      <div class="confirm-row">
        <span class="confirm-label">Server</span>
        <span class="confirm-value">{{ selectedConnection }}</span>
      </div>
    </div>
    <StatusBar hint="⏎ Confirm  ⎋ Back" />
  </template>

  <template v-else-if="step === 'executing'">
    <div class="loading-view">
      <div class="spinner" />
      <span>Connecting to {{ selectedConnection }}...</span>
    </div>
  </template>

  <template v-else-if="step === 'result' && result">
    <AssumeIdentityResult :result="result" />
    <StatusBar hint="⏎ Close  ⎋ Close" />
  </template>

  <template v-else-if="step === 'error'">
    <div class="error-view">
      <div class="error-header">Error</div>
      <pre class="error-message">{{ error }}</pre>
    </div>
    <StatusBar hint="⏎ Close  ⎋ Close" />
  </template>
</template>

<style scoped>
.confirm-view {
  padding: 20px;
}

.confirm-header {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  margin-bottom: 16px;
}

.confirm-row {
  display: flex;
  align-items: baseline;
  gap: 12px;
  padding: 8px 0;
}

.confirm-label {
  font-size: 12px;
  color: var(--text-secondary);
  width: 60px;
  flex-shrink: 0;
}

.confirm-value {
  font-size: 15px;
  font-family: var(--font-mono);
  color: var(--text-primary);
}

.confirm-meta {
  font-size: 12px;
  color: var(--text-secondary);
  margin-left: auto;
}

.loading-view {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 12px;
  padding: 48px 20px;
  color: var(--text-secondary);
  font-size: 14px;
}

.spinner {
  width: 18px;
  height: 18px;
  border: 2px solid var(--border-subtle);
  border-top-color: var(--accent-blue);
  border-radius: 50%;
  animation: spin 0.6s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.error-view {
  padding: 20px;
}

.error-header {
  font-size: 14px;
  font-weight: 600;
  color: var(--accent-red);
  margin-bottom: 12px;
}

.error-message {
  font-size: 13px;
  font-family: var(--font-mono);
  color: var(--text-secondary);
  white-space: pre-wrap;
  word-break: break-word;
  line-height: 1.5;
}
</style>
