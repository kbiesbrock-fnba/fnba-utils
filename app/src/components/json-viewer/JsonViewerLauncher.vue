<script setup lang="ts">
import { openNewJsonViewerWindow } from "../../lib/jsonViewerWindow";

const emit = defineEmits<{
  back: [];
  dismiss: [];
}>();

// Reuse the static primary window (Win+Shift+J target) if it's hidden;
// otherwise spawn a fresh one so repeated launches stack up windows.
async function openJsonViewer() {
  const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
  const primary = await WebviewWindow.getByLabel("json-viewer");
  if (primary && !(await primary.isVisible())) {
    await primary.show();
    await primary.setFocus();
  } else {
    await openNewJsonViewerWindow();
  }
  emit("dismiss");
}

async function openNewWindow() {
  await openNewJsonViewerWindow();
  emit("dismiss");
}
</script>

<template>
  <div class="launcher">
    <button @click="openJsonViewer" class="btn primary">Open JSON Viewer</button>
    <button @click="openNewWindow" class="btn secondary">Open in New Window</button>
  </div>
</template>

<style scoped>
.launcher {
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.btn {
  width: 100%;
  padding: 12px;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  font-size: 14px;
  font-weight: 500;
  transition: background 0.2s;
}

.btn.primary {
  background: #4CAF50;
  color: white;
}

.btn.primary:hover {
  background: #45a049;
}

.btn.secondary {
  background: #3a3a3a;
  color: #ddd;
}

.btn.secondary:hover {
  background: #454545;
}

.btn:active {
  opacity: 0.9;
}
</style>
