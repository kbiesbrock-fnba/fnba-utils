<script setup lang="ts">
import { ref, watch, nextTick } from "vue";
import type { PaletteCommand } from "@/commands/types";
import CommandItem from "./CommandItem.vue";

const props = defineProps<{
  commands: PaletteCommand[];
  selectedIndex: number;
}>();

const emit = defineEmits<{
  select: [index: number];
}>();

const listRef = ref<HTMLElement | null>(null);

watch(
  () => props.selectedIndex,
  () => {
    nextTick(() => {
      const list = listRef.value;
      if (!list) return;
      const item = list.children[props.selectedIndex] as HTMLElement | undefined;
      item?.scrollIntoView({ block: "nearest" });
    });
  },
);
</script>

<template>
  <div ref="listRef" class="command-list" role="listbox">
    <div v-if="commands.length === 0" class="empty">No matching commands</div>
    <CommandItem
      v-for="(cmd, i) in commands"
      :key="cmd.id"
      :icon="cmd.icon"
      :name="cmd.name"
      :description="cmd.description"
      :selected="i === selectedIndex"
      @click="emit('select', i)"
    />
  </div>
</template>

<style scoped>
.command-list {
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
</style>
