<script setup lang="ts">
// A horizontal two-pane split with a draggable gutter. Shared by the JSON and
// Markdown viewers so their resize behaviour stays identical. Fill the `left`
// and `right` slots; the gutter between them resizes the panes. Drag to resize,
// double-click the gutter to reset to `defaultRatio`. When `storageKey` is set,
// the ratio persists to localStorage (one shared ratio per key).
import { ref, onUnmounted } from "vue";

interface Props {
  /** localStorage key to persist the split ratio under. Omit for ephemeral. */
  storageKey?: string;
  /** Initial left-pane fraction (0..1). */
  defaultRatio?: number;
  /** Minimum fraction each side may shrink to. */
  min?: number;
}
const props = withDefaults(defineProps<Props>(), {
  defaultRatio: 0.5,
  min: 0.15,
});

function clamp(v: number): number {
  return Math.min(1 - props.min, Math.max(props.min, v));
}

function readInitial(): number {
  if (props.storageKey) {
    try {
      const raw = localStorage.getItem(props.storageKey);
      if (raw != null) {
        const v = parseFloat(raw);
        if (Number.isFinite(v)) return clamp(v);
      }
    } catch {
      // ignore — fall back to default
    }
  }
  return clamp(props.defaultRatio);
}

const ratio = ref(readInitial());
const container = ref<HTMLElement | null>(null);
const dragging = ref(false);

function persist(): void {
  if (!props.storageKey) return;
  try {
    localStorage.setItem(props.storageKey, String(ratio.value));
  } catch {
    // ignore — quota / private mode
  }
}

function onMouseMove(e: MouseEvent): void {
  const el = container.value;
  if (!el) return;
  const rect = el.getBoundingClientRect();
  if (rect.width <= 0) return;
  ratio.value = clamp((e.clientX - rect.left) / rect.width);
}

function onMouseUp(): void {
  if (!dragging.value) return;
  dragging.value = false;
  window.removeEventListener("mousemove", onMouseMove);
  window.removeEventListener("mouseup", onMouseUp);
  persist();
}

function startDrag(e: MouseEvent): void {
  e.preventDefault();
  dragging.value = true;
  window.addEventListener("mousemove", onMouseMove);
  window.addEventListener("mouseup", onMouseUp);
}

function reset(): void {
  ratio.value = clamp(props.defaultRatio);
  persist();
}

onUnmounted(() => {
  window.removeEventListener("mousemove", onMouseMove);
  window.removeEventListener("mouseup", onMouseUp);
});
</script>

<template>
  <div ref="container" class="split-pane" :class="{ dragging }">
    <div class="split-side" :style="{ flex: `0 0 ${ratio * 100}%` }">
      <slot name="left" />
    </div>
    <div
      class="split-gutter"
      :class="{ active: dragging }"
      title="Drag to resize · double-click to reset"
      @mousedown="startDrag"
      @dblclick="reset"
    >
      <div class="split-gutter-line" />
    </div>
    <div class="split-side grow">
      <slot name="right" />
    </div>
  </div>
</template>

<style scoped>
.split-pane {
  display: flex;
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}

.split-side {
  display: flex;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}

.split-side.grow {
  flex: 1 1 0;
}

/* While dragging, stop the panes (especially textareas) from swallowing the
   mousemove or starting a text selection. */
.split-pane.dragging .split-side {
  pointer-events: none;
  user-select: none;
}

.split-gutter {
  flex: 0 0 8px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: col-resize;
  background: transparent;
  -webkit-app-region: no-drag;
}

.split-gutter-line {
  width: 1px;
  height: 100%;
  background: #404040;
  transition: background 0.1s ease, width 0.1s ease;
}

.split-gutter:hover .split-gutter-line,
.split-gutter.active .split-gutter-line {
  width: 3px;
  background: #4caf50;
}
</style>
