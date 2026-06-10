<script setup lang="ts">
import { getCurrentWindow } from "@tauri-apps/api/window";

/**
 * Eight invisible resize zones (4 edges + 4 corners) overlaid on the window.
 * Each one calls Tauri's `startResizeDragging` synchronously on mousedown so
 * frameless windows can still be resized.
 *
 * `ResizeDirection` in @tauri-apps/api/window v2 is a type-only string union,
 * not a runtime enum — so we pass the string literals directly instead of
 * importing the symbol. The component-local `ResizeDir` keeps strict typing
 * at call sites.
 */

type ResizeDir =
  | "North"
  | "South"
  | "East"
  | "West"
  | "NorthEast"
  | "NorthWest"
  | "SouthEast"
  | "SouthWest";

function startResize(direction: ResizeDir, e: MouseEvent) {
  e.preventDefault();
  e.stopPropagation();
  getCurrentWindow()
    .startResizeDragging(direction)
    .catch((err) => console.warn("[resize] startResizeDragging failed", err));
}
</script>

<template>
  <div class="rh-overlay">
    <div class="rh rh-n" @mousedown="(e) => startResize('North', e)" />
    <div class="rh rh-s" @mousedown="(e) => startResize('South', e)" />
    <div class="rh rh-e" @mousedown="(e) => startResize('East', e)" />
    <div class="rh rh-w" @mousedown="(e) => startResize('West', e)" />
    <div class="rh rh-ne" @mousedown="(e) => startResize('NorthEast', e)" />
    <div class="rh rh-nw" @mousedown="(e) => startResize('NorthWest', e)" />
    <div class="rh rh-se" @mousedown="(e) => startResize('SouthEast', e)" />
    <div class="rh rh-sw" @mousedown="(e) => startResize('SouthWest', e)" />
  </div>
</template>

<style scoped>
.rh-overlay {
  position: fixed;
  inset: 0;
  pointer-events: none;
  z-index: 9999;
}

.rh {
  position: absolute;
  pointer-events: auto;
  background: transparent;
}

.rh-n {
  top: 0;
  left: 20px;
  right: 20px;
  height: 10px;
  cursor: ns-resize;
}
.rh-s {
  bottom: 0;
  left: 20px;
  right: 20px;
  height: 10px;
  cursor: ns-resize;
}
.rh-e {
  top: 20px;
  bottom: 20px;
  right: 0;
  width: 10px;
  cursor: ew-resize;
}
.rh-w {
  top: 20px;
  bottom: 20px;
  left: 0;
  width: 10px;
  cursor: ew-resize;
}

.rh-ne {
  top: 0;
  right: 0;
  width: 20px;
  height: 20px;
  cursor: nesw-resize;
}
.rh-nw {
  top: 0;
  left: 0;
  width: 20px;
  height: 20px;
  cursor: nwse-resize;
}
.rh-se {
  bottom: 0;
  right: 0;
  width: 20px;
  height: 20px;
  cursor: nwse-resize;
}
.rh-sw {
  bottom: 0;
  left: 0;
  width: 20px;
  height: 20px;
  cursor: nesw-resize;
}
</style>
