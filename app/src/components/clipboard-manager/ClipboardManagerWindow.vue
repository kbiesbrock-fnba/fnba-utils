<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { useClipboardManager, type Filter } from "@/composables/useClipboardManager";
import ClipboardEntryRow from "./ClipboardEntryRow.vue";
import { isTauri, onClipboardWindowShown } from "@/lib/tauri";

const {
  entries,
  selectedId,
  selected,
  detail,
  detailLoading,
  query,
  filter,
  loading,
  error,
  selectIndex,
  selectFirst,
  selectLast,
  paste,
  togglePin,
  remove,
  clearAll,
  close,
} = useClipboardManager();

const searchInput = ref<HTMLInputElement | null>(null);
const listEl = ref<HTMLUListElement | null>(null);
const showSettings = ref(false);

const filters: { key: Filter; label: string }[] = [
  { key: "all", label: "All" },
  { key: "text", label: "Text" },
  { key: "html", label: "HTML" },
  { key: "image", label: "Images" },
  { key: "pinned", label: "Pinned" },
];

const detailText = computed(() => {
  if (!detail.value) return null;
  if (detail.value.sensitive) return null;
  if (detail.value.kind === "image") return null;
  return detail.value.textContent ?? "";
});

watch(selectedId, () => {
  void nextTick(() => {
    const el = listEl.value?.querySelector<HTMLElement>(".row.selected");
    el?.scrollIntoView({ block: "nearest" });
  });
});

function focusSearch() {
  const el = searchInput.value;
  if (!el) return;
  el.focus();
  el.select();
}

function onKey(e: KeyboardEvent) {
  // Always-active shortcuts
  if (e.key === "Escape") {
    e.preventDefault();
    close();
    return;
  }
  if (e.key === "ArrowDown") {
    e.preventDefault();
    selectIndex(1);
    return;
  }
  if (e.key === "ArrowUp") {
    e.preventDefault();
    selectIndex(-1);
    return;
  }
  if (e.key === "PageDown") {
    e.preventDefault();
    selectIndex(8);
    return;
  }
  if (e.key === "PageUp") {
    e.preventDefault();
    selectIndex(-8);
    return;
  }
  if (e.key === "Home" && (e.ctrlKey || e.metaKey)) {
    e.preventDefault();
    selectFirst();
    return;
  }
  if (e.key === "End" && (e.ctrlKey || e.metaKey)) {
    e.preventDefault();
    selectLast();
    return;
  }
  if (e.key === "Enter") {
    e.preventDefault();
    // Ctrl/Cmd+Enter = copy only (no auto-paste); plain Enter = paste back.
    void paste({ simulate: !(e.ctrlKey || e.metaKey) });
    return;
  }

  // Bindings that should NOT fire while typing in the search box.
  const inSearch = document.activeElement === searchInput.value;
  if (!inSearch) {
    if (e.key === "Delete" || (e.key === "Backspace" && (e.ctrlKey || e.metaKey))) {
      e.preventDefault();
      void remove();
      return;
    }
    if (e.key.toLowerCase() === "p" && !e.ctrlKey && !e.metaKey && !e.altKey) {
      e.preventDefault();
      void togglePin();
      return;
    }
    if (e.key === "/") {
      e.preventDefault();
      focusSearch();
      return;
    }
  }
}

let unsubShown: (() => void) | null = null;

onMounted(async () => {
  window.addEventListener("keydown", onKey);
  void nextTick(focusSearch);
  // Re-focus the search every time the window is re-shown via the global
  // hotkey, even if the component itself is already mounted. The composable
  // also subscribes to this event (to clear filters); both subscriptions
  // fire independently.
  unsubShown = await onClipboardWindowShown(() => {
    void nextTick(focusSearch);
  });
});

onUnmounted(() => {
  window.removeEventListener("keydown", onKey);
  if (unsubShown) unsubShown();
});

async function startDrag() {
  if (!isTauri) return;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().startDragging();
}

async function startResize(
  dir:
    | "North"
    | "South"
    | "East"
    | "West"
    | "NorthWest"
    | "NorthEast"
    | "SouthWest"
    | "SouthEast",
  e: MouseEvent,
) {
  e.preventDefault();
  e.stopPropagation();
  if (!isTauri) return;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().startResizeDragging(dir);
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}
</script>

<template>
  <div class="panel">
    <div class="resize-edge resize-n" @mousedown="startResize('North', $event)" />
    <div class="resize-edge resize-s" @mousedown="startResize('South', $event)" />
    <div class="resize-edge resize-e" @mousedown="startResize('East', $event)" />
    <div class="resize-edge resize-w" @mousedown="startResize('West', $event)" />
    <div class="resize-corner resize-nw" @mousedown="startResize('NorthWest', $event)" />
    <div class="resize-corner resize-ne" @mousedown="startResize('NorthEast', $event)" />
    <div class="resize-corner resize-sw" @mousedown="startResize('SouthWest', $event)" />
    <div class="resize-corner resize-se" @mousedown="startResize('SouthEast', $event)" />

    <header class="header" @mousedown="startDrag">
      <div class="title-group">
        <span class="title">Clipboard</span>
        <span class="hint">Enter = paste, Ctrl+Enter = copy only, P = pin, Del = remove</span>
      </div>
      <div class="header-actions">
        <button class="icon-btn" title="Settings" @click.stop="showSettings = !showSettings">
          ⚙
        </button>
        <button class="icon-btn" title="Close" @click.stop="close">×</button>
      </div>
    </header>

    <div class="search-row" @mousedown.stop>
      <input
        ref="searchInput"
        v-model="query"
        type="text"
        placeholder="Search clipboard…"
        class="search-input"
        spellcheck="false"
        autocomplete="off"
      />
    </div>

    <nav class="filter-row" @mousedown.stop>
      <button
        v-for="f in filters"
        :key="f.key"
        class="filter-chip"
        :class="{ active: filter === f.key }"
        @click="filter = f.key"
      >
        {{ f.label }}
      </button>
    </nav>

    <div v-if="showSettings" class="settings-row" @mousedown.stop>
      <button class="text-btn" @click="clearAll(false)">Clear non-pinned</button>
      <button class="text-btn danger" @click="clearAll(true)">Clear everything</button>
    </div>

    <main class="body">
      <ul v-if="entries.length" ref="listEl" class="list" @mousedown.stop>
        <ClipboardEntryRow
          v-for="entry in entries"
          :key="entry.id"
          :entry="entry"
          :selected="entry.id === selectedId"
          @select="selectedId = entry.id"
          @togglePin="togglePin(entry.id)"
          @delete="remove(entry.id)"
          @open="paste({ simulate: true })"
        />
      </ul>
      <div v-else-if="loading" class="empty">Loading…</div>
      <div v-else class="empty">
        <p>{{ query ? "No matches." : "Clipboard history is empty." }}</p>
        <p class="hint">Copy something to start building history.</p>
      </div>

      <section v-if="selected" class="detail">
        <div class="detail-meta">
          <span class="badge">{{ selected.kind }}</span>
          <span v-if="selected.sensitive" class="badge sensitive">sensitive</span>
          <span v-if="selected.pinned" class="badge pinned">pinned</span>
          <span class="dim">{{ formatBytes(selected.byteSize) }}</span>
          <span v-if="selected.sourceProcess" class="dim">{{ selected.sourceProcess }}</span>
        </div>
        <div v-if="detailLoading" class="dim">Loading content…</div>
        <div v-else-if="!detail" class="dim">No content.</div>
        <div v-else-if="detail.sensitive" class="dim italic">
          Sensitive content hidden. Press Enter to reveal and paste.
        </div>
        <img
          v-else-if="detail.kind === 'image' && detail.imageBase64"
          class="detail-image"
          :src="`data:image/png;base64,${detail.imageBase64}`"
          :alt="`${detail.width}x${detail.height} image`"
        />
        <pre v-else-if="detailText !== null" class="detail-text">{{ detailText }}</pre>
      </section>
    </main>

    <footer v-if="error" class="error-bar">{{ error }}</footer>
  </div>
</template>

<style scoped>
.panel {
  position: fixed;
  inset: 0;
  display: flex;
  flex-direction: column;
  background: rgba(20, 24, 33, 0.96);
  color: rgba(255, 255, 255, 0.92);
  border-radius: 12px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  box-shadow: 0 24px 48px rgba(0, 0, 0, 0.45);
  overflow: hidden;
  font-family: ui-sans-serif, system-ui, -apple-system, Segoe UI, Roboto, sans-serif;
}

.header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 12px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  cursor: grab;
  user-select: none;
}
.title-group {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.title {
  font-weight: 600;
  font-size: 13px;
  letter-spacing: 0.3px;
}
.hint {
  font-size: 10px;
  color: rgba(255, 255, 255, 0.4);
}
.header-actions {
  display: flex;
  gap: 4px;
}
.icon-btn {
  background: transparent;
  border: none;
  color: rgba(255, 255, 255, 0.7);
  cursor: pointer;
  padding: 4px 8px;
  border-radius: 4px;
  font-size: 14px;
}
.icon-btn:hover {
  background: rgba(255, 255, 255, 0.08);
}

.search-row {
  padding: 8px 12px;
}
.search-input {
  width: 100%;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.08);
  color: rgba(255, 255, 255, 0.92);
  padding: 6px 10px;
  border-radius: 6px;
  font-size: 13px;
  outline: none;
}
.search-input:focus {
  border-color: rgba(96, 165, 250, 0.6);
  background: rgba(255, 255, 255, 0.07);
}

.filter-row {
  display: flex;
  gap: 6px;
  padding: 0 12px 8px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
}
.filter-chip {
  background: transparent;
  border: 1px solid rgba(255, 255, 255, 0.08);
  color: rgba(255, 255, 255, 0.6);
  padding: 3px 10px;
  font-size: 11px;
  border-radius: 999px;
  cursor: pointer;
}
.filter-chip.active {
  background: rgba(96, 165, 250, 0.18);
  border-color: rgba(96, 165, 250, 0.5);
  color: rgba(220, 235, 255, 0.95);
}

.settings-row {
  display: flex;
  gap: 8px;
  padding: 8px 12px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  background: rgba(255, 255, 255, 0.02);
}
.text-btn {
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid rgba(255, 255, 255, 0.08);
  color: rgba(255, 255, 255, 0.85);
  padding: 4px 10px;
  border-radius: 4px;
  font-size: 11px;
  cursor: pointer;
}
.text-btn.danger {
  color: rgba(248, 113, 113, 0.95);
  border-color: rgba(248, 113, 113, 0.25);
}

.body {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
}
.list {
  flex: 1;
  list-style: none;
  margin: 0;
  padding: 6px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.empty {
  padding: 32px;
  text-align: center;
  color: rgba(255, 255, 255, 0.55);
  font-size: 13px;
}
.empty .hint {
  margin-top: 6px;
}

.detail {
  border-top: 1px solid rgba(255, 255, 255, 0.06);
  padding: 10px 12px;
  max-height: 40%;
  overflow-y: auto;
}
.detail-meta {
  display: flex;
  gap: 6px;
  margin-bottom: 8px;
  align-items: center;
  flex-wrap: wrap;
}
.badge {
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  padding: 2px 6px;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.06);
  color: rgba(255, 255, 255, 0.75);
}
.badge.sensitive {
  background: rgba(255, 200, 130, 0.15);
  color: rgba(255, 200, 130, 0.95);
}
.badge.pinned {
  background: rgba(250, 204, 21, 0.15);
  color: rgba(250, 204, 21, 0.95);
}
.dim {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.5);
}
.italic {
  font-style: italic;
}
.detail-text {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, monospace;
  font-size: 12px;
  background: rgba(255, 255, 255, 0.03);
  padding: 8px;
  border-radius: 6px;
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
  color: rgba(220, 235, 255, 0.92);
}
.detail-image {
  max-width: 100%;
  max-height: 200px;
  display: block;
  border-radius: 4px;
}

.error-bar {
  padding: 8px 12px;
  background: rgba(248, 113, 113, 0.15);
  color: rgba(254, 202, 202, 0.95);
  font-size: 12px;
  border-top: 1px solid rgba(248, 113, 113, 0.3);
}

/* Resize handles for decorations:false */
.resize-edge {
  position: absolute;
  z-index: 10;
}
.resize-n { top: 0; left: 6px; right: 6px; height: 4px; cursor: ns-resize; }
.resize-s { bottom: 0; left: 6px; right: 6px; height: 4px; cursor: ns-resize; }
.resize-e { top: 6px; right: 0; bottom: 6px; width: 4px; cursor: ew-resize; }
.resize-w { top: 6px; left: 0; bottom: 6px; width: 4px; cursor: ew-resize; }
.resize-corner {
  position: absolute;
  z-index: 11;
  width: 8px;
  height: 8px;
}
.resize-nw { top: 0; left: 0; cursor: nwse-resize; }
.resize-ne { top: 0; right: 0; cursor: nesw-resize; }
.resize-sw { bottom: 0; left: 0; cursor: nesw-resize; }
.resize-se { bottom: 0; right: 0; cursor: nwse-resize; }
</style>
