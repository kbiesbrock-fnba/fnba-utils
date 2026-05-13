<script setup lang="ts">
interface Props {
  pinned: boolean;
  size?: number;
  pinTitle?: string;
  unpinTitle?: string;
}
const props = withDefaults(defineProps<Props>(), {
  size: 22,
  pinTitle: "Pin",
  unpinTitle: "Unpin",
});
defineEmits<{ (e: "toggle"): void }>();
const iconSize = () => Math.round(props.size / 2);
</script>

<template>
  <button
    class="pin-btn"
    :class="{ active: pinned }"
    :title="pinned ? unpinTitle : pinTitle"
    :style="{ width: `${size}px`, height: `${size}px` }"
    @click="$emit('toggle')"
  >
    <svg
      viewBox="0 0 16 16"
      fill="currentColor"
      :width="iconSize()"
      :height="iconSize()"
    >
      <path d="M9.828.722a.5.5 0 0 1 .354.146l4.95 4.95a.5.5 0 0 1 0 .707c-.48.48-1.072.588-1.503.588-.177 0-.335-.018-.46-.039l-3.134 3.134a6 6 0 0 1 .16 1.013c.046.702-.032 1.687-.72 2.375a.5.5 0 0 1-.707 0l-2.829-2.828-3.182 3.182a.5.5 0 0 1-.707-.708l3.182-3.182L2.398 8.23a.5.5 0 0 1 0-.707c.688-.688 1.673-.767 2.375-.72a6 6 0 0 1 1.013.16l3.134-3.133a3 3 0 0 1-.04-.461c0-.43.109-1.022.589-1.503a.5.5 0 0 1 .353-.146z" />
    </svg>
  </button>
</template>

<style scoped>
.pin-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  background: transparent;
  color: var(--text-secondary);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: background 0.1s ease, color 0.1s ease;
  padding: 0;
  -webkit-app-region: no-drag;
}

.pin-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.pin-btn.active {
  color: var(--accent-blue);
}

.pin-btn.active:hover {
  color: var(--accent-blue);
  background: rgba(96, 165, 250, 0.12);
}
</style>
