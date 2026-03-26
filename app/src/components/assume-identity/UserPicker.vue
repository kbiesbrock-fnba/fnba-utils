<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onUnmounted } from "vue";
import type { IdentityUser } from "../../lib/tauri";
import CommandInput from "../CommandInput.vue";

const props = defineProps<{
  users: IdentityUser[];
}>();

const emit = defineEmits<{
  select: [user: IdentityUser];
}>();

const query = ref("");
const selectedIndex = ref(0);
const listRef = ref<HTMLElement | null>(null);

const filtered = computed(() => {
  if (!query.value) return props.users;
  const q = query.value.toLowerCase();
  return props.users.filter(
    (u) =>
      u.username.toLowerCase().includes(q) ||
      u.labels.toLowerCase().includes(q),
  );
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
  } else if (e.key === "Enter" && filtered.value.length > 0) {
    e.preventDefault();
    e.stopPropagation();
    emit("select", filtered.value[selectedIndex.value]);
  }
}

onMounted(() => window.addEventListener("keydown", onKeydown, true));
onUnmounted(() => window.removeEventListener("keydown", onKeydown, true));
</script>

<template>
  <CommandInput :value="query" placeholder="Select user..." @update="onUpdate" />
  <div class="picker-divider" />
  <div ref="listRef" class="picker-list">
    <div v-if="filtered.length === 0" class="empty">No matching users</div>
    <div
      v-for="(user, i) in filtered"
      :key="user.username"
      class="picker-item"
      :class="{ selected: i === selectedIndex }"
      @click="emit('select', user)"
      @mouseenter="selectedIndex = i"
    >
      <span class="picker-name">{{ user.username }}</span>
      <span class="picker-labels">{{ user.labels }}</span>
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
  padding: 8px 16px;
  gap: 12px;
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

.picker-labels {
  margin-left: auto;
  font-size: 12px;
  color: var(--text-secondary);
}
</style>
