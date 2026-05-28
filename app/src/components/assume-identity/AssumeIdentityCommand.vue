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
  selectedConnection,
  result,
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
  selectConnection,
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

function goBackWithResultReset(): boolean {
  if (step.value === "result") {
    step.value = "connection";
    selectedConnection.value = null;
    result.value = null;
    return true;
  }
  return goBack();
}

useCommandKeys({
  step,
  goBack: goBackWithResultReset,
  emitBack: () => emit("back"),
  emitDismiss: () => emit("dismiss"),
  escapeDismissSteps: ["error"],
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
    <ConnectionPicker :connections="connections" @select="selectConnection" @delete-custom="deleteCustomConnection" />
    <StatusBar hint="↑↓ Navigate  ⏎ Select  ⎋ Back" />
  </template>

  <template v-else-if="step === 'confirm'">
    <div class="confirm-view">
      <div class="confirm-header">Becoming</div>
      <div class="confirm-identity">{{ selectedUser?.username }}</div>
      <div v-if="selectedUserLabels.length" class="confirm-labels">{{ selectedUserLabels.join(' · ') }}</div>
      <div class="confirm-detail">
        <span class="confirm-on">on</span>
        <span class="confirm-connection">{{ selectedConnection?.server }}</span>
      </div>
      <div class="confirm-detail">
        <span class="confirm-on">as</span>
        <span class="confirm-connection">{{ selectedImposter }}</span>
      </div>
      <button class="confirm-btn" @click="execute">Go</button>
    </div>
    <StatusBar hint="⏎ Confirm  ⎋ Back" />
  </template>

  <template v-else-if="step === 'executing'">
    <LoadingView :message="`Becoming ${selectedUser?.username} on ${selectedConnection?.server}...`" />
  </template>

  <template v-else-if="step === 'result' && result">
    <AssumeIdentityResult :result="result" />
    <div class="close-row">
      <button class="confirm-btn" @click="emit('dismiss')">Close</button>
    </div>
    <StatusBar hint="⏎ Close  ⎋ Back" />
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

.close-row {
  display: flex;
  justify-content: center;
  padding: 0 20px 16px;
}

</style>
