<script setup lang="ts">
import { ref, computed } from "vue";
import { useListNavigation } from "@/composables/useListNavigation";
import type { IdentityImposter } from "@/lib/tauri";
import CommandInput from "../CommandInput.vue";

const props = defineProps<{
  imposters: IdentityImposter[];
  selected: string | null;
}>();

const emit = defineEmits<{
  select: [imposter: string];
  deleteCustom: [name: string];
}>();

const query = ref("");
const listRef = ref<HTMLElement | null>(null);

const filtered = computed(() => {
  if (!query.value) return props.imposters;
  const q = query.value.toLowerCase();
  return props.imposters.filter((imp) => imp.name.toLowerCase().includes(q));
});

const { selectedIndex, resetIndex } = useListNavigation({
  itemCount: () => filtered.value.length,
  onSelect: (i) => emit("select", filtered.value[i].name),
  onEnterEmpty: () => {
    if (query.value.trim()) emit("select", query.value.trim());
  },
  extraKeys: [
    {
      key: "Delete",
      handler: () => {
        if (filtered.value.length === 0) return false;
        const imp = filtered.value[selectedIndex.value];
        if (imp?.isCustom) {
          emit("deleteCustom", imp.name);
          return;
        }
        return false;
      },
    },
  ],
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
    placeholder="Select imposter login..."
    @update="onUpdate"
  />
  <div class="picker-divider" />
  <div ref="listRef" class="picker-list">
    <div v-if="filtered.length === 0 && query.trim()" class="empty use-custom">
      Press Enter to use <strong>{{ query.trim() }}</strong>
    </div>
    <div v-else-if="filtered.length === 0" class="empty">No matching imposters</div>
    <div
      v-for="(imp, i) in filtered"
      :key="imp.name"
      class="picker-item"
      :class="{ selected: i === selectedIndex }"
      @click="emit('select', imp.name)"
      @mouseenter="selectedIndex = i"
    >
      <span class="picker-name">{{ imp.name }}</span>
      <span v-if="imp.name === props.selected" class="picker-labels">current</span>
      <span v-else-if="imp.isCustom" class="picker-labels custom-badge">custom</span>
      <button
        v-if="imp.isCustom"
        class="remove-btn"
        title="Delete custom entry (Del)"
        @click.stop="emit('deleteCustom', imp.name)"
      >
        <svg viewBox="0 0 16 16" fill="currentColor" width="12" height="12">
          <path d="M3.72 3.72a.75.75 0 0 1 1.06 0L8 6.94l3.22-3.22a.75.75 0 1 1 1.06 1.06L9.06 8l3.22 3.22a.75.75 0 1 1-1.06 1.06L8 9.06l-3.22 3.22a.75.75 0 0 1-1.06-1.06L6.94 8 3.72 4.78a.75.75 0 0 1 0-1.06Z" />
        </svg>
      </button>
    </div>
  </div>
</template>

<style src="./picker-shared.css" scoped></style>
