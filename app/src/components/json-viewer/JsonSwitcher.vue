<script setup lang="ts">
import { ref, onMounted, onUnmounted } from "vue";
import { readRegistry as readJsonRegistry, removeEntry as removeJsonEntry } from "../../lib/jsonViewerRegistry";
import { readRegistry as readMdRegistry, removeEntry as removeMdEntry } from "../../lib/markdownViewerRegistry";
import { openNewJsonViewerWindow } from "../../lib/jsonViewerWindow";
import { openNewMarkdownViewerWindow } from "../../lib/markdownViewerWindow";

interface SwitcherRow {
  label: string;
  kind: "json" | "md";
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
    allWindows
      .map((w) => w.label)
      .filter((l) => l.startsWith("json-viewer:") || l.startsWith("markdown-viewer:")),
  );

  const jsonRegistry = readJsonRegistry();
  const mdRegistry = readMdRegistry();

  // Prune stale entries (windows that are no longer alive).
  for (const label of Object.keys(jsonRegistry)) {
    if (!liveLabels.has(label)) {
      removeJsonEntry(label);
    }
  }
  for (const label of Object.keys(mdRegistry)) {
    if (!liveLabels.has(label)) {
      removeMdEntry(label);
    }
  }

  // Build rows: live windows joined with registry data, sorted by focusedAt desc.
  const built: SwitcherRow[] = [];
  for (const label of liveLabels) {
    const isMd = label.startsWith("markdown-viewer:");
    const entry = isMd ? mdRegistry[label] : jsonRegistry[label];
    built.push({
      label,
      kind: isMd ? "md" : "json",
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
    // Two virtual rows: rows.length = New JSON, rows.length + 1 = New Markdown.
    if (idx === rows.value.length + 1) {
      await openNewMarkdownViewerWindow();
    } else {
      await openNewJsonViewerWindow();
    }
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
    // +2 for the two trailing virtual rows (New JSON, New Markdown).
    selectedIdx.value = (selectedIdx.value + 1) % (rows.value.length + 2);
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    selectedIdx.value =
      (selectedIdx.value - 1 + rows.value.length + 2) % (rows.value.length + 2);
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
      <div class="switcher-header">Viewer Windows</div>
      <div class="switcher-list">
        <div
          v-for="(row, idx) in rows"
          :key="row.label"
          class="switcher-row"
          :class="{ selected: selectedIdx === idx }"
          @click="activateRow(idx)"
          @mouseenter="selectedIdx = idx"
        >
          <span class="row-icon">{{ row.kind === 'md' ? '📝' : '🔍' }}</span>
          <span class="row-preview">{{ row.preview || "(empty)" }}</span>
          <span class="row-label">{{ row.label.replace(/^(json|markdown)-viewer:/, "#") }}</span>
        </div>
        <!-- "New JSON window" row -->
        <div
          class="switcher-row new-window"
          :class="{ selected: selectedIdx === rows.length }"
          @click="activateRow(rows.length)"
          @mouseenter="selectedIdx = rows.length"
        >
          <span class="row-icon">🔍</span>
          <span class="row-preview">&#xFF0B; New JSON window</span>
        </div>
        <!-- "New Markdown window" row -->
        <div
          class="switcher-row new-window"
          :class="{ selected: selectedIdx === rows.length + 1 }"
          @click="activateRow(rows.length + 1)"
          @mouseenter="selectedIdx = rows.length + 1"
        >
          <span class="row-icon">📝</span>
          <span class="row-preview">&#xFF0B; New Markdown window</span>
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

.row-icon {
  font-size: 13px;
  flex-shrink: 0;
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
