<script setup lang="ts">
import { ref, onMounted, watch } from "vue";

const props = defineProps<{
  value: string;
  placeholder: string;
}>();

const emit = defineEmits<{
  update: [value: string];
}>();

const inputRef = ref<HTMLInputElement | null>(null);

onMounted(() => inputRef.value?.focus());

watch(
  () => props.placeholder,
  () => {
    setTimeout(() => inputRef.value?.focus(), 0);
  },
);

function onInput(e: Event) {
  emit("update", (e.target as HTMLInputElement).value);
}
</script>

<template>
  <div class="input-wrapper">
    <svg class="input-icon" viewBox="0 0 20 20" fill="currentColor" width="18" height="18">
      <path
        fill-rule="evenodd"
        d="M9 3.5a5.5 5.5 0 100 11 5.5 5.5 0 000-11zM2 9a7 7 0 1112.452 4.391l3.328 3.329a.75.75 0 11-1.06 1.06l-3.329-3.328A7 7 0 012 9z"
        clip-rule="evenodd"
      />
    </svg>
    <input
      ref="inputRef"
      type="text"
      :value="value"
      :placeholder="placeholder"
      spellcheck="false"
      autocomplete="off"
      @input="onInput"
    />
  </div>
</template>

<style scoped>
.input-wrapper {
  display: flex;
  align-items: center;
  padding: 14px 16px;
  gap: 12px;
}

.input-icon {
  color: var(--text-secondary);
  flex-shrink: 0;
}

input {
  flex: 1;
  background: none;
  border: none;
  outline: none;
  font-size: 16px;
  font-family: var(--font-sans);
  color: var(--text-primary);
}

input::placeholder {
  color: var(--text-placeholder);
}
</style>
