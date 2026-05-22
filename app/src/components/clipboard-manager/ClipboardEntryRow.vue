<script setup lang="ts">
import { computed } from "vue";
import type { ClipboardEntrySummary } from "@/lib/tauri";

const props = defineProps<{
  entry: ClipboardEntrySummary;
  selected: boolean;
}>();

defineEmits<{
  (e: "select"): void;
  (e: "togglePin"): void;
  (e: "delete"): void;
  (e: "open"): void;
}>();

const ageLabel = computed(() => humanAgo(props.entry.capturedAt));
const sourceLabel = computed(() => {
  const s = props.entry.sourceProcess;
  if (!s) return "";
  return s.replace(/\.exe$/i, "");
});

function humanAgo(epoch: number): string {
  const m = Math.floor((Date.now() - epoch) / 60_000);
  if (m < 1) return "just now";
  if (m < 60) return `${m}m`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h`;
  return `${Math.floor(h / 24)}d`;
}
</script>

<template>
  <li
    class="row"
    :class="{ selected, sensitive: entry.sensitive }"
    @click="$emit('select')"
    @dblclick="$emit('open')"
  >
    <div class="thumb" v-if="entry.kind === 'image' && entry.thumbBase64">
      <img :src="`data:image/png;base64,${entry.thumbBase64}`" alt="" />
    </div>
    <div class="thumb thumb-icon" v-else>
      <span v-if="entry.kind === 'image'" aria-hidden="true">img</span>
      <span v-else-if="entry.kind === 'html'" aria-hidden="true">html</span>
      <span v-else aria-hidden="true">txt</span>
    </div>

    <div class="body">
      <div class="preview" :class="{ masked: entry.sensitive }">
        <template v-if="entry.sensitive">
          <span class="lock" title="Sensitive entry">[locked]</span>
          <span class="masked-text">{{ "•".repeat(12) }}</span>
        </template>
        <template v-else-if="entry.kind === 'image'">
          {{ entry.width }}x{{ entry.height }} image
        </template>
        <template v-else>
          {{ entry.textPreview || "(empty)" }}
        </template>
      </div>
      <div class="meta">
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
.row.sensitive .preview {
  color: rgba(255, 200, 130, 0.85);
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
.lock {
  margin-right: 6px;
  color: rgba(255, 200, 130, 0.9);
  font-style: normal;
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}
.masked-text {
  letter-spacing: 4px;
}
.meta {
  display: flex;
  gap: 8px;
  margin-top: 3px;
  font-size: 11px;
  color: rgba(255, 255, 255, 0.5);
  align-items: center;
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
</style>
