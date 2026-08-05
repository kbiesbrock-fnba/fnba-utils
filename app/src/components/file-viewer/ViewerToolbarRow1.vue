<script setup lang="ts">
// Row 1 of the File Viewer toolbar — the Preview/Edit/Split layout toggle,
// identical for both viewers, plus two named slots for parent-authored
// buttons: #file-actions (Open/Save/Save As) and #utility-actions
// (Find/Clear/+New). Each slot's divider auto-hides when the slot is empty.
//
// Row 2 (JSON's Format/Tree/Flatten/Schema/Diff/A-Z) is JSON-only plain
// inline markup in JsonViewerApp.vue — nothing to share there.
defineProps<{
  layoutMode: "preview" | "edit" | "split";
}>();

defineEmits<{
  "update:layoutMode": [value: "preview" | "edit" | "split"];
}>();
</script>

<template>
  <div class="toolbar-row1">
    <div class="toolbar-buttons">
      <button
        :class="{ active: layoutMode === 'preview' }"
        @click="$emit('update:layoutMode', 'preview')"
        title="Rendered preview"
      >
        👁 Preview
      </button>
      <button
        :class="{ active: layoutMode === 'edit' }"
        @click="$emit('update:layoutMode', 'edit')"
        title="Edit source"
      >
        ✏ Edit
      </button>
      <button
        :class="{ active: layoutMode === 'split' }"
        @click="$emit('update:layoutMode', 'split')"
        title="Edit and preview side by side"
      >
        ⇆ Split
      </button>
      <span v-if="$slots['file-actions']" class="toolbar-divider"></span>
      <slot name="file-actions" />
      <span v-if="$slots['utility-actions']" class="toolbar-divider"></span>
      <slot name="utility-actions" />
    </div>
  </div>
</template>

<style scoped>
.toolbar-row1 {
  display: flex;
  align-items: center;
  padding: 10px 16px;
  background: #2d2d2d;
  border-bottom: 1px solid #404040;
  flex-shrink: 0;
}

.toolbar-buttons {
  display: flex;
  gap: 8px;
  align-items: center;
}

.toolbar-buttons button,
.toolbar-buttons :slotted(button) {
  padding: 6px 12px;
  background: #404040;
  border: 1px solid #555;
  color: #aaa;
  border-radius: 4px;
  cursor: pointer;
  font-size: 12px;
  transition: all 0.15s;
}

.toolbar-buttons button:hover,
.toolbar-buttons :slotted(button:hover) {
  background: #505050;
  color: #ddd;
}

.toolbar-buttons button.active,
.toolbar-buttons :slotted(button.active) {
  background: #4CAF50;
  border-color: #45a049;
  color: white;
}

.toolbar-divider {
  width: 1px;
  height: 22px;
  background: #555;
  margin: 0 4px;
}

.toolbar-buttons button.util-btn,
.toolbar-buttons :slotted(button.util-btn) {
  background: #2d2d2d;
}

.toolbar-buttons button.util-btn:hover,
.toolbar-buttons :slotted(button.util-btn:hover) {
  background: #3a3a3a;
  color: #ddd;
}
</style>
