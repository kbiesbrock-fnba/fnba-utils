<script setup lang="ts">
withDefaults(
  defineProps<{
    hint: string;
    showVersion?: boolean;
  }>(),
  { showVersion: false },
);

const appVersion = __APP_VERSION__;
</script>

<template>
  <div class="status-bar">
    <span v-if="showVersion" class="version" :title="`FNBA Utils ${appVersion}`">v{{ appVersion }}</span>
    <span v-else class="version-spacer" />
    <span class="hint">{{ hint }}</span>
  </div>
</template>

<style scoped>
.status-bar {
  padding: 8px 16px;
  border-top: 1px solid var(--border-subtle);
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
}

.hint {
  font-size: 11px;
  color: var(--text-placeholder);
  font-family: var(--font-mono);
  letter-spacing: 0.02em;
}

.version {
  font-size: 11px;
  color: var(--text-placeholder);
  font-family: var(--font-mono);
  letter-spacing: 0.02em;
  opacity: 0.7;
  max-width: 50%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.version-spacer {
  flex: 0 0 auto;
}
</style>
