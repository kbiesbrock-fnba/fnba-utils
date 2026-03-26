<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { useAssumeIdentity } from "../../composables/useAssumeIdentity";

const copied = ref(false);
function copyError() {
  if (!error.value) return;
  navigator.clipboard.writeText(error.value).then(() => {
    copied.value = true;
    setTimeout(() => (copied.value = false), 2000);
  });
}
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
  recentUsernames,
  loadData,
  reset,
  selectUser,
  selectConnection,
  execute,
  removeRecentUser,
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
    <UserPicker :users="users" :recent-usernames="recentUsernames" @select="selectUser" @remove-recent="removeRecentUser" />
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
        <span class="confirm-meta">{{ selectedUser?.label }}</span>
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
      <div class="error-header">
        <span>Error</span>
        <button class="copy-btn" @click="copyError">
          <svg v-if="!copied" viewBox="0 0 20 20" fill="currentColor" width="14" height="14">
            <path d="M7 3.5A1.5 1.5 0 018.5 2h3.879a1.5 1.5 0 011.06.44l3.122 3.12A1.5 1.5 0 0117 6.622V12.5a1.5 1.5 0 01-1.5 1.5h-1v-3.379a3 3 0 00-.879-2.121L10.5 5.379A3 3 0 008.379 4.5H7v-1z" />
            <path d="M4.5 6A1.5 1.5 0 003 7.5v9A1.5 1.5 0 004.5 18h7a1.5 1.5 0 001.5-1.5v-5.879a1.5 1.5 0 00-.44-1.06L9.44 6.44A1.5 1.5 0 008.378 6H4.5z" />
          </svg>
          <svg v-else viewBox="0 0 20 20" fill="currentColor" width="14" height="14">
            <path fill-rule="evenodd" d="M16.704 4.153a.75.75 0 01.143 1.052l-8 10.5a.75.75 0 01-1.127.075l-4.5-4.5a.75.75 0 011.06-1.06l3.894 3.893 7.48-9.817a.75.75 0 011.05-.143z" clip-rule="evenodd" />
          </svg>
          {{ copied ? 'Copied' : 'Copy' }}
        </button>
      </div>
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
  overflow-y: auto;
  max-height: 380px;
}

.error-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 14px;
  font-weight: 600;
  color: var(--accent-red);
  margin-bottom: 12px;
}

.copy-btn {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 4px 10px;
  border: 1px solid var(--border-input);
  background: var(--bg-hover);
  color: var(--text-secondary);
  border-radius: var(--radius-sm);
  font-size: 12px;
  font-family: var(--font-sans);
  cursor: pointer;
  transition: background 0.1s ease, color 0.1s ease;
}

.copy-btn:hover {
  background: var(--bg-selected);
  color: var(--text-primary);
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
