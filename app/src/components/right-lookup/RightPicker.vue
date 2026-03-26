<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onUnmounted } from "vue";
import CommandInput from "../CommandInput.vue";
import type { RightInfo } from "../../lib/tauri";

const props = defineProps<{
  rights: RightInfo[];
}>();

const emit = defineEmits<{
  select: [right: RightInfo];
}>();

const query = ref("");
const selectedIndex = ref(0);
const listRef = ref<HTMLElement | null>(null);

const filtered = computed(() => {
  if (!query.value) return props.rights;
  const q = query.value.toLowerCase();
  return props.rights.filter((r) => r.rightName.toLowerCase().includes(q));
});

function scrollToSelected() {
  nextTick(() => {
    const list = listRef.value;
    if (!list) return;
    const item = list.children[selectedIndex.value] as HTMLElement | undefined;
    item?.scrollIntoView({ block: "nearest" });
  });
}

watch(selectedIndex, scrollToSelected);

function onUpdate(value: string) {
  query.value = value;
  selectedIndex.value = 0;
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "ArrowDown") {
    e.preventDefault();
    if (filtered.value.length > 0) {
      selectedIndex.value =
        (selectedIndex.value + 1) % filtered.value.length;
    }
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    if (filtered.value.length > 0) {
      selectedIndex.value =
        (selectedIndex.value - 1 + filtered.value.length) %
        filtered.value.length;
    }
  } else if (e.key === "Enter") {
    e.preventDefault();
    e.stopPropagation();
    if (filtered.value.length > 0) {
      emit("select", filtered.value[selectedIndex.value]);
    }
  }
}

onMounted(() => window.addEventListener("keydown", onKeydown, true));
onUnmounted(() => window.removeEventListener("keydown", onKeydown, true));
</script>

<template>
  <CommandInput
    :value="query"
    placeholder="Search rights..."
    @update="onUpdate"
  />
  <div class="picker-divider" />
  <div ref="listRef" class="picker-list">
    <div v-if="filtered.length === 0" class="empty">No matching rights</div>
    <div
      v-for="(right, i) in filtered"
      :key="right.rightId"
      class="picker-item"
      :class="{ selected: i === selectedIndex }"
      @click="emit('select', right)"
      @mouseenter="selectedIndex = i"
    >
      <span class="picker-name">{{ right.rightName }}</span>
      <span class="picker-id">#{{ right.rightId }}</span>
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

.picker-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
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
  color: var(--text-primary);
}

.picker-id {
  font-size: 12px;
  font-family: var(--font-mono);
  color: var(--text-secondary);
}
</style>
