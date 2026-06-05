<script setup lang="ts">
defineProps<{
  icon: string;
  name: string;
  description: string;
  selected: boolean;
  /** Single-character hotkey badge (e.g. "1"-"9"). Null hides the slot but
   *  preserves the gutter width so the icon column doesn't shift between
   *  hotkey-on and hotkey-off list modes. */
  hotkey?: string | null;
}>();
</script>

<template>
  <div class="item" :class="{ selected }" role="option" :aria-selected="selected">
    <span class="item-hotkey" :class="{ visible: hotkey }">{{ hotkey ?? "" }}</span>
    <span class="item-icon">{{ icon }}</span>
    <span class="item-name">{{ name }}</span>
    <span class="item-desc">{{ description }}</span>
  </div>
</template>

<style scoped>
.item {
  display: flex;
  align-items: center;
  padding: 10px 16px;
  gap: 12px;
  cursor: pointer;
  transition: background 0.1s ease;
  border-left: 3px solid transparent;
}

.item:hover {
  background: var(--bg-hover);
}

.item.selected {
  background: var(--bg-selected);
  border-left-color: var(--accent-blue);
}

.item-hotkey {
  flex-shrink: 0;
  width: 18px;
  height: 18px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-family: var(--font-mono);
  font-size: 10.5px;
  font-weight: 600;
  color: var(--text-placeholder);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--bg-hover);
  opacity: 0;
  transition: opacity 0.1s ease;
}

.item-hotkey.visible {
  opacity: 1;
}

.item.selected .item-hotkey.visible {
  color: var(--accent-blue);
  border-color: var(--accent-blue);
}

.item-icon {
  font-size: 18px;
  flex-shrink: 0;
  width: 24px;
  text-align: center;
}

.item-name {
  font-size: 14px;
  font-weight: 500;
  color: var(--text-primary);
}

.item-desc {
  margin-left: auto;
  font-size: 12px;
  color: var(--text-secondary);
}
</style>
