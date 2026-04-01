<script setup lang="ts">
import { ref, computed } from "vue";
import { useListNavigation } from "@/composables/useListNavigation";
import CommandInput from "../CommandInput.vue";

const props = defineProps<{
  imposters: string[];
  selected: string | null;
}>();

const emit = defineEmits<{
  select: [imposter: string];
}>();

const query = ref("");
const listRef = ref<HTMLElement | null>(null);

const filtered = computed(() => {
  if (!query.value) return props.imposters;
  const q = query.value.toLowerCase();
  return props.imposters.filter((imp) => imp.toLowerCase().includes(q));
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
      :key="imp"
      class="picker-item"
      :class="{ selected: i === selectedIndex }"
      @click="emit('select', imp)"
      @mouseenter="selectedIndex = i"
    >
      <span class="picker-name">{{ imp }}</span>
      <span v-if="imp === props.selected" class="picker-labels">current</span>
    </div>
  </div>
</template>

<style src="./picker-shared.css" scoped></style>
