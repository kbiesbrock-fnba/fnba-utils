<script setup lang="ts">
const emit = defineEmits<{
  back: [];
  dismiss: [];
}>();

async function openJsonViewer() {
  const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
  const win = await WebviewWindow.getByLabel("json-viewer");
  if (win) {
    await win.show();
    await win.setFocus();
  }
  emit("dismiss");
}
</script>

<template>
  <div class="launcher">
    <button @click="openJsonViewer" class="btn primary">Open JSON Viewer</button>
  </div>
</template>

<style scoped>
.launcher {
  padding: 16px;
}

.btn {
  width: 100%;
  padding: 12px;
  background: #4CAF50;
  color: white;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  font-size: 14px;
  font-weight: 500;
  transition: background 0.2s;
}

.btn:hover {
  background: #45a049;
}

.btn:active {
  opacity: 0.9;
}
</style>
