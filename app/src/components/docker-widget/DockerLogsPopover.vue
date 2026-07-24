<script setup lang="ts">
defineProps<{
  text: string;
  title: string;
}>();

const emit = defineEmits<{
  (e: "copy"): void;
  (e: "close"): void;
}>();
</script>

<template>
  <div class="logs-overlay">
    <div class="logs-popover">
      <div class="logs-header">
        <span class="logs-title">{{ title }}</span>
        <div class="logs-actions">
          <button class="action-btn" title="Copy logs" @click="emit('copy')">&#x1F4CB; Copy</button>
          <button class="action-btn close-btn" title="Close" @click="emit('close')">&#x2715;</button>
        </div>
      </div>
      <pre class="logs-body">{{ text || '(no output)' }}</pre>
    </div>
  </div>
</template>

<style scoped>
.logs-overlay {
  position: absolute;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  display: flex;
  align-items: flex-end;
  z-index: 100;
}

.logs-popover {
  width: 100%;
  max-height: 220px;
  background: #1a1a1a;
  border: 1px solid #333;
  border-bottom: none;
  border-radius: 4px 4px 0 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.logs-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 5px 8px;
  background: #252525;
  border-bottom: 1px solid #333;
  flex-shrink: 0;
}

.logs-title {
  font-size: 11px;
  color: #aaa;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
  min-width: 0;
}

.logs-actions {
  display: flex;
  gap: 4px;
  flex-shrink: 0;
}

.action-btn {
  padding: 2px 7px;
  background: #333;
  border: 1px solid #444;
  color: #aaa;
  border-radius: 3px;
  cursor: pointer;
  font-size: 10px;
  transition: background 0.1s;
}

.action-btn:hover {
  background: #404040;
  color: #ddd;
}

.close-btn:hover {
  background: #5a1a1a;
  border-color: #f85149;
  color: #f85149;
}

.logs-body {
  margin: 0;
  padding: 8px;
  overflow-y: auto;
  overflow-x: auto;
  flex: 1;
  font-family: "Monaco", "Menlo", "Ubuntu Mono", monospace;
  font-size: 10px;
  line-height: 1.4;
  color: #b0b0b0;
  white-space: pre;
  background: #1a1a1a;
}
</style>
