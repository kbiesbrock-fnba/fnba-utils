<script setup lang="ts">
defineProps<{
  icon: string;
  name: string;
  description: string;
  selected: boolean;
}>();
</script>

<template>
  <div class="item" :class="{ selected }" role="option" :aria-selected="selected">
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
