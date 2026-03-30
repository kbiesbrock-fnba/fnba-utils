<script setup lang="ts">
import { ref, computed } from "vue";
import { useListNavigation } from "../../composables/useListNavigation";
import CommandInput from "../CommandInput.vue";

const props = defineProps<{
  connections: string[];
}>();

const emit = defineEmits<{
  select: [connection: string];
}>();

const query = ref("");
const listRef = ref<HTMLElement | null>(null);

const filtered = computed(() => {
  if (!query.value) return props.connections;
  const q = query.value.toLowerCase();
  return props.connections.filter((c) => c.toLowerCase().includes(q));
});

const { selectedIndex, resetIndex } = useListNavigation({
  itemCount: () => filtered.value.length,
  onSelect: (i) => emit("select", filtered.value[i]),
  onEnterEmpty: () => {
    if (query.value.trim()) emit("select", query.value.trim());
  },
  listRef,
});

function onUpdate(value: string) {
  query.value = value;
  resetIndex();
}
</script>

<template>
  <CommandInput
    :value="query"
    placeholder="Select connection..."
    @update="onUpdate"
  />
  <div class="picker-divider" />
  <div ref="listRef" class="picker-list">
    <div v-if="filtered.length === 0 && query.trim()" class="empty use-custom">
      Press Enter to use <strong>{{ query.trim() }}</strong>
    </div>
    <div v-else-if="filtered.length === 0" class="empty">No matching connections</div>
    <div
      v-for="(conn, i) in filtered"
      :key="conn"
      class="picker-item"
      :class="{ selected: i === selectedIndex }"
      @click="emit('select', conn)"
      @mouseenter="selectedIndex = i"
    >
      <span class="picker-name">{{ conn }}</span>
    </div>
  </div>
</template>

<style scoped>
.picker-divider {
  height: 1px;
  background: var(--border-subtle);
}

.picker-list {
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

.use-custom strong {
  font-family: var(--font-mono);
  color: var(--text-primary);
}

.picker-item {
  display: flex;
  align-items: center;
  padding: 10px 16px;
  cursor: pointer;
  transition: background 0.1s ease;
  border-left: 3px solid transparent;
}

.picker-item:hover {
  background: var(--bg-hover);
}

.picker-item.selected {
  background: var(--bg-selected);
  border-left-color: var(--accent-blue);
}

.picker-name {
  font-size: 14px;
  font-family: var(--font-mono);
  color: var(--text-primary);
}
</style>
