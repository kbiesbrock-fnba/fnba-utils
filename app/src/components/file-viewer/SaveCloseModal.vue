<script setup lang="ts">
// Unsaved-changes close-prompt modal shared by every file-backed File Viewer
// body (JSON, Markdown). Markup + CSS lifted verbatim from the close-modal
// block that used to live only in MarkdownViewerApp.vue. Caller pre-formats
// the display name (e.g. `baseName(filePath) ?? "This document"`).
defineProps<{
  show: boolean;
  label: string;
}>();

defineEmits<{
  save: [];
  discard: [];
  cancel: [];
}>();
</script>

<template>
  <div v-if="show" class="close-modal-backdrop">
    <div class="close-modal">
      <div class="cm-title">Save changes?</div>
      <div class="cm-msg">{{ label }} has unsaved changes.</div>
      <div class="cm-actions">
        <button class="cm-btn cm-primary" @click="$emit('save')">Save</button>
        <button class="cm-btn" @click="$emit('discard')">Don't save</button>
        <button class="cm-btn" @click="$emit('cancel')">Cancel</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.close-modal-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.close-modal {
  background: #2d2d2d;
  border: 1px solid #555;
  border-radius: 8px;
  padding: 24px;
  width: 320px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.cm-title {
  font-size: 15px;
  font-weight: 600;
  color: #f0f0f0;
}

.cm-msg {
  font-size: 13px;
  color: #aaa;
  word-break: break-all;
}

.cm-actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
  margin-top: 4px;
}

.cm-btn {
  padding: 6px 14px;
  background: #404040;
  border: 1px solid #555;
  color: #ddd;
  border-radius: 4px;
  cursor: pointer;
  font-size: 13px;
  transition: background 0.12s;
}

.cm-btn:hover {
  background: #505050;
}

.cm-btn.cm-primary {
  background: #4CAF50;
  border-color: #45a049;
  color: white;
}

.cm-btn.cm-primary:hover {
  background: #45a049;
}
</style>
