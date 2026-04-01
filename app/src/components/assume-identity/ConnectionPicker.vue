<script setup lang="ts">
import { ref, computed } from "vue";
import { useListNavigation } from "@/composables/useListNavigation";
import type { IdentityConnection } from "@/lib/tauri";
import CommandInput from "../CommandInput.vue";
import LabelPrompt from "./LabelPrompt.vue";

const props = defineProps<{
  connections: IdentityConnection[];
}>();

const emit = defineEmits<{
  select: [connection: IdentityConnection];
}>();

const query = ref("");
const listRef = ref<HTMLElement | null>(null);
const labelMode = ref<{ server: string } | null>(null);

type DisplayRow =
  | { kind: "header"; title: string }
  | { kind: "connection"; conn: IdentityConnection; flatIndex: number };

const displayData = computed(() => {
  const q = query.value.toLowerCase();
  const matching = q
    ? props.connections.filter(
        (c) => c.label.toLowerCase().includes(q) || c.server.toLowerCase().includes(q),
      )
    : props.connections;

  const rows: DisplayRow[] = [];
  let flatIndex = 0;

  // Group by label, preserve existing sort order within groups
  const groups = new Map<string, IdentityConnection[]>();
  for (const c of matching) {
    const arr = groups.get(c.label) ?? [];
    arr.push(c);
    groups.set(c.label, arr);
  }

  for (const [label, conns] of groups) {
    rows.push({ kind: "header", title: label });
    for (const conn of conns) {
      rows.push({ kind: "connection", conn, flatIndex: flatIndex++ });
    }
  }

  return { rows, totalItems: flatIndex };
});

function getRowAtIndex(index: number) {
  for (const row of displayData.value.rows) {
    if (row.kind === "connection" && row.flatIndex === index) return row;
  }
  return undefined;
}

const { selectedIndex, resetIndex } = useListNavigation({
  itemCount: () => labelMode.value ? 0 : displayData.value.totalItems,
  onSelect: (i) => {
    const row = getRowAtIndex(i);
    if (row) emit("select", row.conn);
  },
  onEnterEmpty: () => {
    if (query.value.trim()) {
      labelMode.value = { server: query.value.trim() };
    }
  },
  listRef,
  scrollStrategy: "data-index",
});

function onUpdate(value: string) {
  query.value = value;
  resetIndex();
}

function onLabelConfirm(label: string) {
  if (!labelMode.value) return;
  emit("select", { label, server: labelMode.value.server });
  labelMode.value = null;
}

function onLabelCancel() {
  labelMode.value = null;
}
</script>

<template>
  <LabelPrompt
    v-if="labelMode"
    :value="labelMode.server"
    default-label="Local"
    @confirm="onLabelConfirm"
    @cancel="onLabelCancel"
  />
  <template v-else>
    <CommandInput
      key="query"
      :value="query"
      placeholder="Select connection..."
      @update="onUpdate"
    />
    <div class="picker-divider" />
    <div ref="listRef" class="picker-list">
      <div v-if="displayData.totalItems === 0 && query.trim()" class="empty use-custom">
        Press Enter to use <strong>{{ query.trim() }}</strong>
      </div>
      <div v-else-if="displayData.totalItems === 0" class="empty">No matching connections</div>
      <template v-for="(row, i) in displayData.rows" :key="i">
        <div v-if="row.kind === 'header'" class="section-header">
          {{ row.title }}
        </div>
        <div
          v-else
          class="picker-item"
          :class="{ selected: row.flatIndex === selectedIndex }"
          :data-index="row.flatIndex"
          @click="emit('select', row.conn)"
          @mouseenter="selectedIndex = row.flatIndex"
        >
          <span class="picker-name">{{ row.conn.server }}</span>
        </div>
      </template>
    </div>
  </template>
</template>

<style src="./picker-shared.css" scoped></style>
