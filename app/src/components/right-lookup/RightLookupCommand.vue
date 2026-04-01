<script setup lang="ts">
import { onMounted } from "vue";
import { useRightLookup } from "../../composables/useRightLookup";
import { usePalette } from "../../composables/usePalette";
import { useCommandKeys } from "../../composables/useCommandKeys";
import RightPicker from "./RightPicker.vue";
import RightLookupResult from "./RightLookupResult.vue";
import AssociateRightsResult from "./AssociateRightsResult.vue";
import StatusBar from "../StatusBar.vue";
import LoadingView from "../LoadingView.vue";
import ErrorView from "../ErrorView.vue";

const emit = defineEmits<{
  back: [];
  dismiss: [];
}>();

const {
  step,
  rights,
  selectedRight,
  selectedAssociate,
  associates,
  associateRights,
  error,
  loadRights,
  reset,
  selectRight,
  selectAssociate,
  goBack,
} = useRightLookup();

const { returningToPrevious } = usePalette();

onMounted(() => {
  if (returningToPrevious.value) {
    returningToPrevious.value = false;
  } else {
    reset();
    loadRights();
  }
});

useCommandKeys({
  step,
  goBack,
  emitBack: () => emit("back"),
  emitDismiss: () => emit("dismiss"),
  escapeDismissSteps: ["error"],
  enterActions: {
    error: () => emit("dismiss"),
  },
});

defineExpose({ step });
</script>

<template>
  <template v-if="step === 'loading'">
    <LoadingView message="Loading rights..." />
  </template>

  <template v-else-if="step === 'rights'">
    <RightPicker :rights="rights" @select-right="selectRight" @select-associate="selectAssociate" />
    <StatusBar hint="↑↓ Navigate  ⏎ Select  ⎋ Back" />
  </template>

  <template v-else-if="step === 'executing'">
    <LoadingView :message="selectedRight ? `Looking up ${selectedRight.rightName}...` : selectedAssociate ? `Looking up ${selectedAssociate.nickname ?? selectedAssociate.assocId}...` : 'Loading...'" />
  </template>

  <template v-else-if="step === 'result' && selectedRight">
    <RightLookupResult :right="selectedRight" :associates="associates" />
    <StatusBar hint="↑↓ Navigate  ⇥ Toggle Copy/Assume  ⏎ Execute  ⎋ Back" />
  </template>

  <template v-else-if="step === 'associateResult' && selectedAssociate">
    <AssociateRightsResult :associate="selectedAssociate" :rights="associateRights" />
    <StatusBar hint="↑↓ Navigate  ⎋ Back" />
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
.close-row {
  display: flex;
  justify-content: center;
  padding: 0 20px 16px;
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
</style>
