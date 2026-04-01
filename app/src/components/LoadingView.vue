<script setup lang="ts">
defineProps<{
  message: string;
}>();
</script>

<template>
  <div class="loading-view">
    <div class="spinner" />
    <span>{{ message }}</span>
  </div>
</template>

<style scoped>
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
</style>
