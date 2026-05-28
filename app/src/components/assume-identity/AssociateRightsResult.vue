<script setup lang="ts">
import type { RightInfo, RightAssociate } from "@/lib/tauri";
import { useListNavigation } from "@/composables/useListNavigation";
import { ref } from "vue";

const props = defineProps<{
  associate: RightAssociate;
  rights: RightInfo[];
}>();

const emit = defineEmits<{
  assume: [];
}>();

const listRef = ref<HTMLElement | null>(null);

const { selectedIndex } = useListNavigation({
  itemCount: () => props.rights.length,
  listRef,
});
</script>

<template>
  <div class="result-header">
    <div class="assoc-info">
      <span class="assoc-nick">{{ associate.nickname ?? associate.assocId }}</span>
      <span class="assoc-name">
        {{ [associate.firstName, associate.lastName].filter(Boolean).join(' ') }}
      </span>
    </div>
    <div class="header-actions">
      <span class="badge">{{ rights.length }} right{{ rights.length !== 1 ? 's' : '' }}</span>
      <button v-if="associate.login" class="assume-btn" @click="emit('assume')">Assume</button>
    </div>
  </div>
  <div class="picker-divider" />
  <div ref="listRef" class="result-list">
    <div v-if="rights.length === 0" class="empty">
      No rights found for this associate
    </div>
    <div
      v-for="(right, i) in rights"
      :key="right.rightId"
      class="result-row"
      :class="{ selected: i === selectedIndex }"
      @mouseenter="selectedIndex = i"
    >
      <span class="right-name">{{ right.rightName }}</span>
      <span class="right-id">#{{ right.rightId }}</span>
    </div>
  </div>
</template>

<style scoped>
.result-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
}

.assoc-info {
  display: flex;
  align-items: baseline;
  gap: 8px;
}

.assoc-nick {
  font-size: 14px;
  font-weight: 600;
  font-family: var(--font-mono);
  color: var(--text-primary);
}

.assoc-name {
  font-size: 13px;
  color: var(--text-secondary);
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.badge {
  font-size: 12px;
  color: var(--text-secondary);
  background: var(--bg-hover);
  padding: 2px 8px;
  border-radius: var(--radius-sm);
}

.assume-btn {
  padding: 2px 10px;
  border: 1px solid var(--border-subtle);
  background: transparent;
  color: var(--text-secondary);
  border-radius: var(--radius-sm);
  font-size: 11px;
  font-family: var(--font-sans);
  cursor: pointer;
  transition: border-color 0.15s ease, color 0.15s ease;
}

.assume-btn:hover {
  border-color: var(--text-secondary);
  color: var(--text-primary);
}

.picker-divider {
  height: 1px;
  background: var(--border-subtle);
}

.result-list {
  flex: 1;
  overflow-y: auto;
  padding: 4px 0;
  max-height: 320px;
}

.empty {
  padding: 24px 16px;
  text-align: center;
  color: var(--text-secondary);
  font-size: 14px;
}

.result-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 16px;
  border-left: 3px solid transparent;
  transition: background 0.1s ease;
}

.result-row:hover {
  background: var(--bg-hover);
}

.result-row.selected {
  background: var(--bg-selected);
  border-left-color: var(--accent-blue);
}

.right-name {
  font-size: 14px;
  color: var(--text-primary);
}

.right-id {
  font-size: 12px;
  font-family: var(--font-mono);
  color: var(--text-secondary);
}
</style>
