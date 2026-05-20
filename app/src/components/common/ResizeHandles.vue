<script setup lang="ts">
import { getCurrentWindow, ResizeDirection } from "@tauri-apps/api/window";
import { isTauri } from "@/lib/tauri";

/**
 * Eight invisible resize zones (4 edges + 4 corners) overlaid on the window
 * viewport. Each one calls Tauri's `startResizeDragging` synchronously on
 * mousedown so frameless windows can be resized.
 *
 * Notes:
 *   - `getCurrentWindow()` and `ResizeDirection` are imported STATICALLY so
 *     the resize call happens in the same task as the mousedown — Tauri
 *     requires that to hand the drag off to the OS window manager.
 *   - Handles are visually invisible (no background, no border). The OS
 *     cursor changing to a resize arrow is the only affordance; the user
 *     said the previous tinted overlay added noise without helping.
 *   - Sizes are deliberately generous (10 px edges, 20 px corners) so the
 *     grab targets are easy to hit. The window border + decorations are
 *     hidden, so blocking a few pixels of inner UI at the extreme edge is
 *     acceptable — keep meaningful interactive controls inset accordingly.
 */

function startResize(direction: ResizeDirection, e: MouseEvent) {
  if (!isTauri) return;
  e.preventDefault();
  e.stopPropagation();
  getCurrentWindow()
    .startResizeDragging(direction)
    .catch((err) => console.warn("[resize] startResizeDragging failed", err));
}
</script>

<template>
  <div class="rh-overlay">
    <div class="rh rh-n" @mousedown="(e) => startResize(ResizeDirection.North, e)" />
    <div class="rh rh-s" @mousedown="(e) => startResize(ResizeDirection.South, e)" />
    <div class="rh rh-e" @mousedown="(e) => startResize(ResizeDirection.East, e)" />
    <div class="rh rh-w" @mousedown="(e) => startResize(ResizeDirection.West, e)" />
    <div class="rh rh-ne" @mousedown="(e) => startResize(ResizeDirection.NorthEast, e)" />
    <div class="rh rh-nw" @mousedown="(e) => startResize(ResizeDirection.NorthWest, e)" />
    <div class="rh rh-se" @mousedown="(e) => startResize(ResizeDirection.SouthEast, e)" />
    <div class="rh rh-sw" @mousedown="(e) => startResize(ResizeDirection.SouthWest, e)" />
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

/* Edges — 10 px thick, span between corners so the corner cursors win at
   the extremes. */
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

/* Corners — 20×20 squares, diagonal cursors. */
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
