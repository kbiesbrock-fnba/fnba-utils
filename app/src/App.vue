<script setup lang="ts">
import CommandPalette from "./components/CommandPalette.vue";
import { usePalette } from "./composables/usePalette";

const { dismiss } = usePalette();

function onBackdropClick(e: MouseEvent) {
  if ((e.target as HTMLElement).classList.contains("backdrop")) {
    dismiss();
  }
}
</script>

<template>
  <div class="backdrop" @mousedown="onBackdropClick">
    <CommandPalette />
  </div>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.35);
  backdrop-filter: blur(8px);
  display: flex;
  align-items: flex-start;
  justify-content: center;
  padding-top: 15vh;
  animation: fade-in 0.15s ease-out;
}

@keyframes fade-in {
  from { opacity: 0; }
  to { opacity: 1; }
}
</style>
