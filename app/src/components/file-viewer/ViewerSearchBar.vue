<script setup lang="ts">
// Ctrl+F reveal-on-demand search bar shared by both File Viewer bodies.
// `matchIndex == null` → JSON's shape ("N results", no prev/next).
// Both matchCount and matchIndex set → Markdown's shape ("N of M" + prev/next).
// Rendered via v-if by the host, so every reveal is a fresh mount — hence
// the unconditional focus-on-mount below.
import { ref, computed, onMounted } from "vue";

const props = defineProps<{
  modelValue: string;
  placeholder?: string;
  matchCount?: number | null;
  matchIndex?: number | null;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: string];
  close: [];
  next: [];
  prev: [];
}>();

const inputRef = ref<HTMLInputElement | null>(null);

const localValue = computed({
  get: () => props.modelValue,
  set: (v: string) => emit("update:modelValue", v),
});

onMounted(() => {
  inputRef.value?.focus();
});
</script>

<template>
  <div class="search-bar">
    <span class="search-icon">🔍</span>
    <input
      ref="inputRef"
      v-model="localValue"
      type="text"
      :placeholder="placeholder ?? 'Search...'"
      class="search-input"
      @keydown.esc.stop="$emit('close')"
      @keydown.enter.exact.prevent="$emit('next')"
      @keydown.enter.shift.exact.prevent="$emit('prev')"
    />
    <span v-if="matchIndex == null" class="search-count">
      {{ matchCount ? `${matchCount} result${matchCount === 1 ? '' : 's'}` : '' }}
    </span>
    <template v-else>
      <span class="search-count">{{ matchCount ? `${matchIndex} of ${matchCount}` : "0 of 0" }}</span>
      <button class="search-nav-btn" title="Previous match (Shift+Enter)" @click="$emit('prev')">▲</button>
      <button class="search-nav-btn" title="Next match (Enter)" @click="$emit('next')">▼</button>
    </template>
    <button class="search-close-btn" title="Close (Esc)" @click="$emit('close')">✕</button>
  </div>
</template>

<style scoped>
.search-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 16px;
  background: #262626;
  border-bottom: 1px solid #404040;
  flex-shrink: 0;
}

.search-icon {
  font-size: 14px;
}

.search-input {
  background: #333;
  border: 1px solid #555;
  border-radius: 4px;
  color: inherit;
  outline: none;
  flex: 1;
  padding: 6px 10px;
  font-size: 13px;
}

.search-input:focus {
  border-color: #4CAF50;
}

.search-count {
  font-size: 11px;
  color: #888;
  white-space: nowrap;
  min-width: 60px;
  text-align: right;
}

.search-nav-btn,
.search-close-btn {
  background: #404040;
  border: 1px solid #555;
  color: #aaa;
  border-radius: 4px;
  cursor: pointer;
  font-size: 11px;
  padding: 4px 8px;
  transition: background 0.12s, color 0.12s;
}

.search-nav-btn:hover,
.search-close-btn:hover {
  background: #505050;
  color: #ddd;
}
</style>
