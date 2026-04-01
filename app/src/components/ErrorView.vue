<script setup lang="ts">
import { ref } from "vue";

const props = defineProps<{
  error: string;
}>();

const copied = ref(false);

function copyError() {
  navigator.clipboard.writeText(props.error).then(() => {
    copied.value = true;
    setTimeout(() => (copied.value = false), 2000);
  });
}
</script>

<template>
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
</template>

<style scoped>
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
