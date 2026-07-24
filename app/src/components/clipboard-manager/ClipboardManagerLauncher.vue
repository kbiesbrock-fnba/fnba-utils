<script setup lang="ts">
import { onMounted } from "vue";

const emit = defineEmits<{
  back: [];
  dismiss: [];
}>();

onMounted(async () => {
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
