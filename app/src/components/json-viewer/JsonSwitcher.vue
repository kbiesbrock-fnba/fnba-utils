<script setup lang="ts">
import { ref, onMounted, onUnmounted } from "vue";
import { readRegistry, removeEntry } from "../../lib/jsonViewerRegistry";
import { openNewJsonViewerWindow } from "../../lib/jsonViewerWindow";

interface SwitcherRow {
  label: string;
  preview: string;
  focusedAt: number;
}

const rows = ref<SwitcherRow[]>([]);
const selectedIdx = ref(0);
let unlistenRefresh: (() => void) | null = null;

async function refresh() {
  const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
  const allWindows = await WebviewWindow.getAll();
  const liveLabels = new Set(
    allWindows.map((w) => w.label).filter((l) => l.startsWith("json-viewer:")),
  );

  const registry = readRegistry();

  // Prune stale entries (windows that are no longer alive).
  for (const label of Object.keys(registry)) {
    if (!liveLabels.has(label)) {
      removeEntry(label);
    }
  }

  // Build rows: live windows joined with registry data, sorted by focusedAt desc.
  const built: SwitcherRow[] = [];
  for (const label of liveLabels) {
    const entry = registry[label];
    built.push({
      label,
      preview: entry?.preview ?? "",
      focusedAt: entry?.focusedAt ?? 0,
    });
  }
  built.sort((a, b) => b.focusedAt - a.focusedAt);
  rows.value = built;

  // Clamp selection to valid range.
  if (selectedIdx.value >= rows.value.length) {
    selectedIdx.value = 0;
  }
}

async function activateRow(idx: number) {
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");

  if (idx >= rows.value.length) {
    // "New window" row.
    await openNewJsonViewerWindow();
  } else {
    const row = rows.value[idx];
    const w = await WebviewWindow.getByLabel(row.label);
    if (w) {
      await w.unminimize().catch(() => {});
      await w.show().catch(() => {});
      await w.setFocus().catch(() => {});
    }
  }
  await getCurrentWindow().hide();
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "ArrowDown") {
    e.preventDefault();
    // +1 wraps around; rows.value.length is the "New window" virtual row.
    selectedIdx.value = (selectedIdx.value + 1) % (rows.value.length + 1);
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    selectedIdx.value =
      (selectedIdx.value - 1 + rows.value.length + 1) % (rows.value.length + 1);
  } else if (e.key === "Enter") {
    e.preventDefault();
    void activateRow(selectedIdx.value);
  } else if (e.key === "Escape") {
    e.preventDefault();
    void (async () => {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().hide();
    })();
  }
}

function onWindowBlur() {
  void (async () => {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().hide();
  })();
}

onMounted(async () => {
  await refresh();
  window.addEventListener("keydown", onKeydown);
  window.addEventListener("blur", onWindowBlur);

  const { listen } = await import("@tauri-apps/api/event");
  unlistenRefresh = await listen("json-switcher-refresh", async () => {
    await refresh();
    selectedIdx.value = 0;
  });
});

onUnmounted(() => {
  window.removeEventListener("keydown", onKeydown);
  window.removeEventListener("blur", onWindowBlur);
  if (unlistenRefresh) {
    unlistenRefresh();
    unlistenRefresh = null;
  }
});
</script>

<template>
  <div class="switcher-overlay">
    <div class="switcher-panel">
      <div class="switcher-header">JSON Windows</div>
      <div class="switcher-list">
        <div
          v-for="(row, idx) in rows"
          :key="row.label"
          class="switcher-row"
          :class="{ selected: selectedIdx === idx }"
          @click="activateRow(idx)"
          @mouseenter="selectedIdx = idx"
        >
          <span class="row-preview">{{ row.preview || "(empty)" }}</span>
          <span class="row-label">{{ row.label.replace("json-viewer:", "#") }}</span>
        </div>
        <!-- "New window" row -->
        <div
          class="switcher-row new-window"
          :class="{ selected: selectedIdx === rows.length }"
          @click="activateRow(rows.length)"
          @mouseenter="selectedIdx = rows.length"
        >
          <span class="row-preview">&#xFF0B; New window</span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.switcher-overlay {
  position: fixed;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
}

.switcher-panel {
  width: 340px;
  max-height: 400px;
  background: #1e1e1e;
  border: 1px solid #404040;
  border-radius: 8px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.6);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.switcher-header {
  padding: 10px 14px 8px;
  font-size: 11px;
  font-weight: 600;
  color: #888;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  border-bottom: 1px solid #333;
  flex-shrink: 0;
}

.switcher-list {
  overflow-y: auto;
  flex: 1;
  padding: 4px 0;
}

.switcher-row {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 10px;
  padding: 8px 14px;
  cursor: pointer;
  border-radius: 4px;
  margin: 1px 4px;
  transition: background 0.1s;
}

.switcher-row:hover,
.switcher-row.selected {
  background: #2d2d2d;
}

.row-preview {
  font-size: 13px;
  color: #e0e0e0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
  min-width: 0;
}

.row-label {
  font-size: 10px;
  color: #666;
  flex-shrink: 0;
  font-family: monospace;
}

.switcher-row.new-window .row-preview {
  color: #4CAF50;
}
</style>
