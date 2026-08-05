<script setup lang="ts">
// Presentational title bar shared by every File Viewer body (JSON, Markdown).
// Markup + CSS lifted verbatim from the identical title-bar block that used
// to be duplicated in JsonViewerApp.vue and MarkdownViewerApp.vue. Not built
// on the common PinButton.vue — that component uses the app's blue-accent
// theme, while these viewers use a self-contained dark/green theme that must
// not shift.
defineProps<{
  title: string;
  pinned: boolean;
  isMaximized: boolean;
}>();

defineEmits<{
  pin: [];
  minimize: [];
  maximize: [];
  close: [];
}>();
</script>

<template>
  <div class="title-bar" data-tauri-drag-region>
    <span class="tb-title" data-tauri-drag-region>{{ title }}</span>
    <div class="tb-buttons">
      <button class="tb-btn" :class="{ active: pinned }" @click="$emit('pin')" title="Keep on top">📌</button>
      <button class="tb-btn" @click="$emit('minimize')" title="Minimize">—</button>
      <button class="tb-btn" @click="$emit('maximize')" :title="isMaximized ? 'Restore' : 'Maximize'">{{ isMaximized ? '🗗' : '🗖' }}</button>
      <button class="tb-btn close" @click="$emit('close')" title="Close (Esc)">✕</button>
    </div>
  </div>
</template>

<style scoped>
.title-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 32px;
  padding: 0 8px 0 12px;
  background: #252525;
  border-bottom: 1px solid #404040;
  flex-shrink: 0;
  -webkit-app-region: drag;
  user-select: none;
}

.tb-title {
  font-size: 12px;
  color: #aaa;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
  min-width: 0;
  -webkit-app-region: drag;
}

.tb-buttons {
  display: flex;
  gap: 2px;
  flex-shrink: 0;
  -webkit-app-region: no-drag;
}

.tb-btn {
  width: 28px;
  height: 24px;
  padding: 0;
  background: transparent;
  border: none;
  color: #888;
  border-radius: 3px;
  cursor: pointer;
  font-size: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background 0.12s, color 0.12s;
  -webkit-app-region: no-drag;
}

.tb-btn:hover {
  background: #3a3a3a;
  color: #ddd;
}

.tb-btn.active {
  color: #4CAF50;
}

.tb-btn.active:hover {
  background: #3a3a3a;
  color: #66bb6a;
}

.tb-btn.close:hover {
  background: #c0392b;
  color: white;
}
</style>
