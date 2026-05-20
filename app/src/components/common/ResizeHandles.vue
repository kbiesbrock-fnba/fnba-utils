<script setup lang="ts">
import { isTauri } from "@/lib/tauri";

/**
 * Eight invisible resize zones (4 edges + 4 corners) overlaid on the window
 * viewport. Each one calls Tauri's `startResizeDragging` with the matching
 * direction on mousedown, so frameless windows still get drag-to-resize even
 * without OS decorations.
 *
 * Subtle by default (transparent); the edge zones brighten + thicken on hover
 * so the resize affordance is discoverable. Use `position: fixed; inset: 0`
 * so the overlay doesn't depend on the parent being positioned.
 */

type Direction =
  | "North"
  | "South"
  | "East"
  | "West"
  | "NorthEast"
  | "NorthWest"
  | "SouthEast"
  | "SouthWest";

async function startResize(direction: Direction, e: MouseEvent) {
  if (!isTauri) return;
  e.preventDefault();
  e.stopPropagation();
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const { ResizeDirection } = await import("@tauri-apps/api/window");
    await getCurrentWindow().startResizeDragging(ResizeDirection[direction]);
  } catch (err) {
    console.warn("[resize] startResizeDragging failed", err);
  }
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
  transition: background-color 0.12s ease, width 0.12s ease, height 0.12s ease;
}

.rh:hover {
  background: rgba(96, 165, 250, 0.28);
}

/* Edges — thin, span between the corner squares so corner cursors win at the
   four extremes. Hover bumps the visible thickness for clearer affordance. */
.rh-n {
  top: 0;
  left: 10px;
  right: 10px;
  height: 4px;
  cursor: ns-resize;
}
.rh-s {
  bottom: 0;
  left: 10px;
  right: 10px;
  height: 4px;
  cursor: ns-resize;
}
.rh-e {
  top: 10px;
  bottom: 10px;
  right: 0;
  width: 4px;
  cursor: ew-resize;
}
.rh-w {
  top: 10px;
  bottom: 10px;
  left: 0;
  width: 4px;
  cursor: ew-resize;
}

.rh-n:hover,
.rh-s:hover {
  height: 8px;
}
.rh-e:hover,
.rh-w:hover {
  width: 8px;
}

/* Corners — 12px squares; cursor flips correctly for each diagonal. */
.rh-ne {
  top: 0;
  right: 0;
  width: 12px;
  height: 12px;
  cursor: nesw-resize;
}
.rh-nw {
  top: 0;
  left: 0;
  width: 12px;
  height: 12px;
  cursor: nwse-resize;
}
.rh-se {
  bottom: 0;
  right: 0;
  width: 12px;
  height: 12px;
  cursor: nwse-resize;
}
.rh-sw {
  bottom: 0;
  left: 0;
  width: 12px;
  height: 12px;
  cursor: nesw-resize;
}

.rh-ne:hover,
.rh-nw:hover,
.rh-se:hover,
.rh-sw:hover {
  width: 16px;
  height: 16px;
}
</style>
