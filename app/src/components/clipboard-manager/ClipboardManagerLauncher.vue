<script setup lang="ts">
import { onMounted } from "vue";
import { isTauri } from "@/lib/tauri";

const emit = defineEmits<{
  back: [];
  dismiss: [];
}>();

onMounted(async () => {
  if (!isTauri) {
    // Browser dev: the clipboard window doesn't exist as a separate Tauri
    // window, so navigate the current frame to the hash route instead.
    window.location.hash = "#clipboard-manager";
    emit("dismiss");
    return;
  }
  try {
    const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
    const win = await WebviewWindow.getByLabel("clipboard-manager");
    if (win) {
      await win.show();
      await win.setFocus();
    }
  } catch (e) {
    console.warn("clipboard launcher: failed to show window", e);
  } finally {
    emit("dismiss");
  }
});
</script>

<template>
  <div class="launcher">Opening Clipboard…</div>
</template>

<style scoped>
.launcher {
  padding: 24px;
  text-align: center;
  color: rgba(255, 255, 255, 0.7);
  font-size: 13px;
}
</style>
