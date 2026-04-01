<script setup lang="ts">
import { ref } from "vue";
import { useListNavigation } from "../../composables/useListNavigation";
import CommandInput from "../CommandInput.vue";

const props = defineProps<{
  value: string;
  placeholder?: string;
  defaultLabel: string;
}>();

const emit = defineEmits<{
  confirm: [label: string];
  cancel: [];
}>();

const query = ref("");

useListNavigation({
  itemCount: () => 0,
  onEnterEmpty: () => {
    emit("confirm", query.value.trim() || props.defaultLabel);
  },
  extraKeys: [
    {
      key: "Escape",
      handler: () => {
        emit("cancel");
      },
    },
  ],
});

function onUpdate(value: string) {
  query.value = value;
}
</script>

<template>
  <CommandInput
    :value="query"
    :placeholder="placeholder ?? `Label for ${props.value}...`"
    @update="onUpdate"
  />
  <div class="picker-divider" />
  <div class="label-prompt">
    <span class="label-prompt-value">{{ props.value }}</span>
    <span class="label-prompt-hint">Enter a label, or press Enter for &ldquo;{{ defaultLabel }}&rdquo;</span>
  </div>
</template>

<style scoped>
.picker-divider {
  height: 1px;
  background: var(--border-subtle);
}

.label-prompt {
  padding: 24px 16px;
  text-align: center;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.label-prompt-value {
  font-size: 16px;
  font-family: var(--font-mono);
  font-weight: 600;
  color: var(--text-primary);
}

.label-prompt-hint {
  font-size: 13px;
  color: var(--text-secondary);
}
</style>
