<script setup lang="ts">
defineProps<{
  hint: string;
}>();
</script>

<template>
  <div class="status-bar">
    <span class="hint">{{ hint }}</span>
  </div>
</template>

<style scoped>
.status-bar {
  padding: 8px 16px;
  border-top: 1px solid var(--border-subtle);
  display: flex;
  justify-content: flex-end;
}

.hint {
  font-size: 11px;
  color: var(--text-placeholder);
  font-family: var(--font-mono);
  letter-spacing: 0.02em;
}
</style>
