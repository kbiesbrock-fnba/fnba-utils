<script setup lang="ts">
import { computed, onMounted, onUnmounted } from "vue";
import { useAssumeIdentity } from "@/composables/useAssumeIdentity";
import { useCommandKeys } from "@/composables/useCommandKeys";
import StatusBar from "../StatusBar.vue";
import LoadingView from "../LoadingView.vue";
import ErrorView from "../ErrorView.vue";
import ImposterPicker from "./ImposterPicker.vue";
import UserPicker from "./UserPicker.vue";
import ConnectionPicker from "./ConnectionPicker.vue";
import AssumeIdentityResult from "./AssumeIdentityResult.vue";
import AssociateRightsResult from "./AssociateRightsResult.vue";

const emit = defineEmits<{
  back: [];
  dismiss: [];
}>();

const {
  step,
  imposters,
  selectedImposter,
  users,
  connections,
  selectedUser,
  selectedConnections,
  runResults,
  executingProgress,
  error,
  loading,
  recentUsers,
  searchServer,
  inspectedAssociate,
  inspectedRights,
  loadData,
  reset,
  selectImposter,
  selectUser,
  selectConnections,
  pinUser,
  unpinFavorite,
  removeRecentUser,
  viewRights,
  assumeInspected,
  execute,
  deleteCustomConnection,
  deleteCustomImposter,
  goBack,
} = useAssumeIdentity();

const selectedUserLabels = computed(() => {
  if (!selectedUser.value) return [];
  const name = selectedUser.value.username;
  return [...new Set(users.value.filter((u) => u.username === name).map((u) => u.label))];
});

onMounted(async () => {
  reset();
  await loadData();
});

onUnmounted(() => reset());

useCommandKeys({
  step,
  goBack,
  emitBack: () => emit("back"),
  emitDismiss: () => emit("dismiss"),
  escapeDismissSteps: ["error"],
  // On the combined result step Escape does nothing (but is swallowed so it
  // can't fall through and close/back the palette); only Enter closes.
  escapeNoopSteps: ["result"],
  enterActions: {
    userRights: () => assumeInspected(),
    confirm: () => execute(),
    result: () => emit("dismiss"),
    error: () => emit("dismiss"),
  },
});

defineExpose({ step });
</script>

<template>
  <template v-if="step === 'imposter'">
    <ImposterPicker :imposters="imposters" :selected="selectedImposter" @select="selectImposter" @delete-custom="deleteCustomImposter" />
    <StatusBar hint="↑↓ Navigate  ⏎ Select  ⎋ Back" />
  </template>

  <template v-else-if="step === 'user'">
    <UserPicker
      :users="users"
      :recent-users="recentUsers"
      :search-server="searchServer"
      @select="selectUser"
      @remove-favorite="unpinFavorite"
      @remove-recent="removeRecentUser"
      @pin="pinUser"
      @view-rights="viewRights"
    />
    <StatusBar hint="↑↓ Navigate  ←→ Scope  ⇥ Rights  1–9 Quick  ⏎ Select  ⎋ Back" />
  </template>

  <template v-else-if="step === 'userRights'">
    <LoadingView v-if="loading && inspectedRights.length === 0" message="Loading rights…" />
    <AssociateRightsResult
      v-else-if="inspectedAssociate"
      :associate="inspectedAssociate"
      :rights="inspectedRights"
      @assume="assumeInspected"
    />
    <StatusBar hint="↑↓ Scroll  ⏎ Assume  ⎋ Back" />
  </template>

  <template v-else-if="step === 'connection'">
    <ConnectionPicker
      :connections="connections"
      :initial-checked="selectedConnections"
      @select="selectConnections"
      @delete-custom="deleteCustomConnection"
    />
    <StatusBar hint="↑↓ Navigate  ␣ Toggle  ⏎ Continue  ⎋ Back" />
  </template>

  <template v-else-if="step === 'confirm'">
    <div class="confirm-view">
      <div class="confirm-header">Becoming</div>
      <div class="confirm-identity">{{ selectedUser?.username }}</div>
      <div v-if="selectedUserLabels.length" class="confirm-labels">{{ selectedUserLabels.join(' · ') }}</div>
      <div class="confirm-detail">
        <span class="confirm-on">as</span>
        <span class="confirm-connection">{{ selectedImposter }}</span>
      </div>
      <div class="confirm-conn-header">
        on {{ selectedConnections.length }} connection{{ selectedConnections.length === 1 ? '' : 's' }}
      </div>
      <ul class="confirm-conn-list">
        <li v-for="conn in selectedConnections" :key="conn.server" class="confirm-conn-row">
          <span class="confirm-connection">{{ conn.server }}</span>
          <span class="confirm-conn-label">{{ conn.label }}</span>
        </li>
      </ul>
      <button class="confirm-btn" @click="execute">Go</button>
    </div>
    <StatusBar hint="⏎ Confirm  ⎋ Back" />
  </template>

  <template v-else-if="step === 'executing'">
    <LoadingView
      :message="executingProgress
        ? `Becoming ${selectedUser?.username} — ${executingProgress.current}/${executingProgress.total} · ${executingProgress.server}...`
        : `Becoming ${selectedUser?.username}...`"
    />
  </template>

  <template v-else-if="step === 'result'">
    <div class="multi-result">
      <div v-for="run in runResults" :key="run.connection.server" class="multi-result-section">
        <div class="multi-result-head">
          <span class="multi-result-server">{{ run.connection.server }}</span>
          <span class="multi-result-label">{{ run.connection.label }}</span>
        </div>
        <AssumeIdentityResult v-if="run.result" :result="run.result" compact />
        <ErrorView v-else-if="run.error" :error="run.error" />
      </div>
    </div>
    <div class="close-row">
      <button class="confirm-btn" @click="emit('dismiss')">Close</button>
    </div>
    <StatusBar hint="⏎ Close" />
  </template>

  <template v-else-if="step === 'error'">
    <ErrorView :error="error!" />
    <div class="close-row">
      <button class="confirm-btn" @click="emit('dismiss')">Close</button>
    </div>
    <StatusBar hint="⏎ Close  ⎋ Close" />
  </template>
</template>

<style scoped>
.confirm-view {
  padding: 28px 20px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
}

.confirm-header {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.08em;
}

.confirm-identity {
  font-size: 20px;
  font-family: var(--font-mono);
  font-weight: 600;
  color: var(--text-primary);
}

.confirm-labels {
  font-size: 12px;
  color: var(--text-secondary);
  margin-top: 2px;
}

.confirm-detail {
  display: flex;
  align-items: baseline;
  gap: 6px;
  font-size: 13px;
  color: var(--text-secondary);
  margin-top: 12px;
  margin-bottom: 16px;
}

.confirm-on {
  color: var(--text-secondary);
}

.confirm-connection {
  font-family: var(--font-mono);
  color: var(--text-primary);
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
  transition: border-color 0.15s ease, color 0.15s ease;
}

.confirm-btn:hover {
  border-color: var(--text-secondary);
  color: var(--text-primary);
}

.confirm-conn-header {
  font-size: 11px;
  font-weight: 600;
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.06em;
  margin-top: 14px;
  margin-bottom: 6px;
}

.confirm-conn-list {
  list-style: none;
  margin: 0 0 16px;
  padding: 0;
  width: 100%;
  max-width: 320px;
  max-height: 160px;
  overflow-y: auto;
}

.confirm-conn-row {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
  padding: 4px 0;
  border-bottom: 1px solid var(--border-subtle);
}

.confirm-conn-row:last-child {
  border-bottom: none;
}

.confirm-conn-label {
  font-size: 12px;
  color: var(--text-secondary);
}

.multi-result {
  overflow-y: auto;
  max-height: 480px;
}

.multi-result-section + .multi-result-section {
  border-top: 1px solid var(--border-subtle);
}

.multi-result-head {
  display: flex;
  align-items: baseline;
  gap: 8px;
  padding: 10px 20px 0;
}

.multi-result-server {
  font-size: 13px;
  font-family: var(--font-mono);
  font-weight: 600;
  color: var(--text-primary);
}

.multi-result-label {
  font-size: 11px;
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.close-row {
  display: flex;
  justify-content: center;
  padding: 0 20px 16px;
}

</style>
