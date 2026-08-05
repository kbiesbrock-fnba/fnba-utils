<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from "vue";
import type { ClipboardEntrySummary } from "@/lib/tauri";
import { deriveLabel } from "@/lib/clipboardLabel";

const props = defineProps<{
  entry: ClipboardEntrySummary;
  selected: boolean;
}>();

const emit = defineEmits<{
  (e: "select"): void;
  (e: "togglePin"): void;
  (e: "delete"): void;
  (e: "open"): void;
  (e: "pasteOriginal"): void;
  (e: "pasteWithLabel"): void;
  (e: "pasteWithLabelOriginal"): void;
  (e: "copyObfuscated"): void;
  (e: "copyOriginal"): void;
}>();

const ageLabel = computed(() => humanAgo(props.entry.capturedAt));
const kindLabel = computed(() => deriveLabel(props.entry));
const sourceLabel = computed(() => {
  const s = props.entry.sourceProcess;
  if (!s) return "";
  return s.replace(/\.exe$/i, "");
});

// --- Right-click context menu ---

const menuOpen = ref(false);
const menuX = ref(0);
const menuY = ref(0);

function openContextMenu(e: MouseEvent) {
  e.preventDefault();
  emit("select");
  menuX.value = e.clientX;
  menuY.value = e.clientY;
  menuOpen.value = true;
  document.addEventListener("click", onOutsideClick, true);
  document.addEventListener("keydown", onMenuKey);
}

function closeMenu() {
  menuOpen.value = false;
  document.removeEventListener("click", onOutsideClick, true);
  document.removeEventListener("keydown", onMenuKey);
}

function onOutsideClick() {
  closeMenu();
}

function onMenuKey(e: KeyboardEvent) {
  if (e.key === "Escape") closeMenu();
}

onBeforeUnmount(closeMenu);

function runAction(action: "open" | "pasteOriginal" | "pasteWithLabel" | "pasteWithLabelOriginal" | "copyObfuscated" | "copyOriginal" | "togglePin" | "delete") {
  closeMenu();
  // Switch explicitly so Vue's emit overloads narrow to a literal event name.
  switch (action) {
    case "open": emit("open"); break;
    case "pasteOriginal": emit("pasteOriginal"); break;
    case "pasteWithLabel": emit("pasteWithLabel"); break;
    case "pasteWithLabelOriginal": emit("pasteWithLabelOriginal"); break;
    case "copyObfuscated": emit("copyObfuscated"); break;
    case "copyOriginal": emit("copyOriginal"); break;
    case "togglePin": emit("togglePin"); break;
    case "delete": emit("delete"); break;
  }
}

function humanAgo(epoch: number): string {
  const now = Date.now();
  const mins = Math.floor((now - epoch) / 60_000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins}m`;
  // Past an hour, a coarse "3h"/"2d" is less useful than the actual wall-clock
  // time it was copied. Same-day copies show just the time; older ones include
  // the date so a day-old copy isn't ambiguous.
  const when = new Date(epoch);
  const time = when.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  if (when.toDateString() === new Date(now).toDateString()) return time;
  const date = when.toLocaleDateString([], { month: "short", day: "numeric" });
  return `${date} ${time}`;
}
</script>

<template>
  <li
    class="row"
    :class="{ selected, sensitive: entry.sensitive }"
    @click="$emit('select')"
    @dblclick="$emit('open')"
    @contextmenu="openContextMenu"
  >
    <div class="thumb" v-if="entry.kind === 'image' && entry.thumbBase64">
      <img :src="`data:image/png;base64,${entry.thumbBase64}`" alt="" />
    </div>
    <div class="thumb thumb-icon" v-else>
      <span aria-hidden="true">{{ kindLabel }}</span>
    </div>

    <div class="body">
      <div v-if="entry.label" class="label" :title="entry.label">
        {{ entry.label }}
      </div>
      <div
        class="preview"
        :class="{ obfuscated: entry.sensitive, secondary: !!entry.label }"
      >
        <template v-if="entry.kind === 'image' && !entry.textPreview">
          {{ entry.width }}x{{ entry.height }} image
        </template>
        <template v-else>
          {{ entry.textPreview || "(empty)" }}
        </template>
      </div>
      <div class="meta">
        <span v-if="entry.sensitive" class="pii-tag" :title="entry.piiKinds.join(', ') || 'sensitive'">
          {{ entry.piiKinds.length ? entry.piiKinds.join(' · ') : 'sensitive' }}
        </span>
        <span v-if="entry.pinned" class="pin-tag" title="Pinned">pinned</span>
        <span class="age">{{ ageLabel }}</span>
        <span v-if="sourceLabel" class="source">{{ sourceLabel }}</span>
      </div>
    </div>

    <div class="actions">
      <button
        class="icon-btn"
        :title="entry.pinned ? 'Unpin' : 'Pin'"
        @click.stop="$emit('togglePin')"
      >
        {{ entry.pinned ? "★" : "☆" }}
      </button>
      <button
        class="icon-btn danger"
        title="Delete"
        @click.stop="$emit('delete')"
      >
        ×
      </button>
    </div>
  </li>

  <Teleport to="body">
    <ul
      v-if="menuOpen"
      class="ctx-menu"
      :style="{ left: menuX + 'px', top: menuY + 'px' }"
      @click.stop
      @mousedown.stop
    >
      <li @click="runAction('open')">
        {{ entry.sensitive ? "Paste obfuscated" : "Paste" }}
        <kbd>Enter</kbd>
      </li>
      <li v-if="entry.sensitive" @click="runAction('pasteOriginal')" class="danger">
        Paste original
        <kbd>Shift+Enter</kbd>
      </li>
      <li v-if="entry.label" @click="runAction('pasteWithLabel')">
        Paste with label
        <kbd>Ctrl+L</kbd>
      </li>
      <li v-if="entry.label && entry.sensitive" @click="runAction('pasteWithLabelOriginal')" class="danger">
        Paste with label (original)
        <kbd>Ctrl+Shift+L</kbd>
      </li>
      <li @click="runAction('copyObfuscated')">
        {{ entry.sensitive ? "Copy obfuscated" : "Copy" }}
        <kbd>Ctrl+Enter</kbd>
      </li>
      <li v-if="entry.sensitive" @click="runAction('copyOriginal')" class="danger">
        Copy original
        <kbd>Ctrl+Alt+Enter</kbd>
      </li>
      <li class="separator"></li>
      <li @click="runAction('togglePin')">
        {{ entry.pinned ? "Unpin" : "Pin" }}
      </li>
      <li @click="runAction('delete')" class="danger">Delete</li>
    </ul>
  </Teleport>
</template>

<style scoped>
.row {
  display: flex;
  gap: 10px;
  padding: 8px 10px;
  border-radius: 8px;
  cursor: pointer;
  align-items: center;
  border: 1px solid transparent;
  transition: background-color 0.08s, border-color 0.08s;
}
.row:hover {
  background: rgba(255, 255, 255, 0.04);
}
.row.selected {
  background: rgba(96, 165, 250, 0.18);
  border-color: rgba(96, 165, 250, 0.5);
}
.row.sensitive {
  border-left: 2px solid rgba(255, 200, 130, 0.5);
}
.row.sensitive .preview.obfuscated {
  color: rgba(255, 200, 130, 0.95);
  font-style: italic;
}

.thumb {
  flex: 0 0 44px;
  width: 44px;
  height: 44px;
  background: rgba(255, 255, 255, 0.05);
  border-radius: 6px;
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
}
.thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.thumb-icon span {
  font-size: 10px;
  text-transform: uppercase;
  color: rgba(255, 255, 255, 0.6);
  letter-spacing: 0.5px;
}

.body {
  flex: 1;
  min-width: 0;
}
.label {
  font-size: 13px;
  font-weight: 600;
  color: rgba(255, 255, 255, 0.98);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  margin-bottom: 2px;
}
.preview {
  font-size: 13px;
  color: rgba(255, 255, 255, 0.92);
  overflow: hidden;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  text-overflow: ellipsis;
  word-break: break-word;
}
.preview.secondary {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.55);
  -webkit-line-clamp: 1;
}
.meta {
  display: flex;
  gap: 8px;
  margin-top: 3px;
  font-size: 11px;
  color: rgba(255, 255, 255, 0.5);
  align-items: center;
}
.pii-tag {
  color: rgba(255, 200, 130, 0.95);
  text-transform: uppercase;
  font-size: 10px;
  letter-spacing: 0.5px;
  background: rgba(255, 200, 130, 0.12);
  padding: 1px 6px;
  border-radius: 999px;
}
.pin-tag {
  color: rgba(250, 204, 21, 0.85);
  text-transform: uppercase;
  font-size: 10px;
  letter-spacing: 0.5px;
}

.actions {
  display: flex;
  gap: 2px;
  opacity: 0;
  transition: opacity 0.08s;
}
.row:hover .actions,
.row.selected .actions {
  opacity: 1;
}
.icon-btn {
  background: transparent;
  border: none;
  color: rgba(255, 255, 255, 0.6);
  cursor: pointer;
  font-size: 14px;
  padding: 4px 6px;
  border-radius: 4px;
}
.icon-btn:hover {
  background: rgba(255, 255, 255, 0.08);
  color: rgba(255, 255, 255, 0.95);
}
.icon-btn.danger:hover {
  color: rgba(248, 113, 113, 0.95);
}

/* Context menu */
.ctx-menu {
  position: fixed;
  z-index: 1000;
  margin: 0;
  padding: 4px;
  min-width: 220px;
  list-style: none;
  background: rgba(20, 24, 33, 0.98);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 6px;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.5);
  font-family: ui-sans-serif, system-ui, -apple-system, Segoe UI, Roboto, sans-serif;
  color: rgba(255, 255, 255, 0.9);
}
.ctx-menu li {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 6px 10px;
  font-size: 12px;
  cursor: pointer;
  border-radius: 4px;
  gap: 16px;
}
.ctx-menu li:hover {
  background: rgba(96, 165, 250, 0.18);
}
.ctx-menu li.danger {
  color: rgba(255, 200, 130, 0.95);
}
.ctx-menu li.danger:hover {
  background: rgba(255, 200, 130, 0.14);
}
.ctx-menu li.separator {
  height: 1px;
  padding: 0;
  margin: 4px 6px;
  background: rgba(255, 255, 255, 0.08);
  cursor: default;
}
.ctx-menu li.separator:hover {
  background: rgba(255, 255, 255, 0.08);
}
.ctx-menu kbd {
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 10px;
  color: rgba(255, 255, 255, 0.5);
  background: rgba(255, 255, 255, 0.05);
  padding: 1px 4px;
  border-radius: 3px;
}
</style>
