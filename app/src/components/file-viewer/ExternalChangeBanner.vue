<script setup lang="ts">
// External-on-disk-change banner shared by every file-backed File Viewer body
// (JSON, Markdown). Markup + CSS lifted verbatim from the banner block that
// used to live only in MarkdownViewerApp.vue.
defineProps<{
  state: null | "changed" | "deleted";
  dirty: boolean;
}>();

defineEmits<{
  reload: [];
  "open-disk-copy": [];
  "keep-mine": [];
  "save-again": [];
  dismiss: [];
}>();
</script>

<template>
  <div v-if="state === 'changed'" class="ext-banner">
    <span class="ext-msg">⚠ This file changed on disk{{ dirty ? " — you have unsaved edits" : "" }}.</span>
    <span class="ext-actions">
      <button class="ext-btn ext-primary" @click="$emit('reload')">{{ dirty ? "Reload (discard mine)" : "Reload" }}</button>
      <button v-if="dirty" class="ext-btn" @click="$emit('open-disk-copy')">Open disk copy ↗</button>
      <button class="ext-btn" @click="$emit('keep-mine')">Keep mine</button>
    </span>
  </div>
  <div v-else-if="state === 'deleted'" class="ext-banner ext-deleted">
    <span class="ext-msg">⚠ This file was deleted on disk.</span>
    <span class="ext-actions">
      <button class="ext-btn ext-primary" @click="$emit('save-again')">Save again</button>
      <button class="ext-btn" @click="$emit('dismiss')">Dismiss</button>
    </span>
  </div>
</template>

<style scoped>
.ext-banner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 14px;
  background: #3a2f1a;
  border-bottom: 1px solid #5c4a1f;
  color: #e0c66a;
  font-size: 12px;
  flex-shrink: 0;
  gap: 12px;
}

.ext-banner.ext-deleted {
  background: #3a1e1e;
  border-bottom-color: #5c2a2a;
  color: #e08080;
}

.ext-msg {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ext-actions {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
}

.ext-btn {
  padding: 4px 10px;
  background: #4a3a20;
  border: 1px solid #7a6030;
  color: #e0c66a;
  border-radius: 4px;
  cursor: pointer;
  font-size: 11px;
  transition: background 0.12s, color 0.12s;
  white-space: nowrap;
}

.ext-btn:hover {
  background: #5a4a28;
  color: #f0d87a;
}

.ext-banner.ext-deleted .ext-btn {
  background: #4a2020;
  border-color: #7a3030;
  color: #e08080;
}

.ext-banner.ext-deleted .ext-btn:hover {
  background: #5a2828;
  color: #f09090;
}

.ext-btn.ext-primary {
  background: #4CAF50;
  border-color: #45a049;
  color: white;
}

.ext-btn.ext-primary:hover {
  background: #45a049;
  color: white;
}
</style>
