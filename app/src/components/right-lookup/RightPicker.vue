<script setup lang="ts">
import { ref, computed, onUnmounted } from "vue";
import CommandInput from "../CommandInput.vue";
import { searchAssociates, type RightInfo, type RightAssociate } from "../../lib/tauri";
import { useListNavigation } from "../../composables/useListNavigation";

const props = defineProps<{
  rights: RightInfo[];
}>();

const emit = defineEmits<{
  selectRight: [right: RightInfo];
  selectAssociate: [assoc: RightAssociate];
}>();

const query = ref("");
const listRef = ref<HTMLElement | null>(null);
const matchedAssociates = ref<RightAssociate[]>([]);
const searchingAssociates = ref(false);
let debounceTimer: ReturnType<typeof setTimeout> | null = null;
let searchVersion = 0;

const filteredRights = computed(() => {
  if (!query.value) return props.rights;
  const q = query.value.toLowerCase();
  return props.rights.filter((r) => r.rightName.toLowerCase().includes(q));
});

type DisplayRow =
  | { kind: "header"; label: string }
  | { kind: "right"; right: RightInfo; flatIndex: number }
  | { kind: "associate"; assoc: RightAssociate; flatIndex: number }
  | { kind: "searching" };

const displayRows = computed(() => {
  const rows: DisplayRow[] = [];
  let idx = 0;

  if (filteredRights.value.length > 0) {
    if (query.value) rows.push({ kind: "header", label: "Rights" });
    for (const right of filteredRights.value) {
      rows.push({ kind: "right", right, flatIndex: idx++ });
    }
  }

  if (query.value.trim().length >= 2) {
    rows.push({ kind: "header", label: "Associates" });
    if (searchingAssociates.value) {
      rows.push({ kind: "searching" });
    } else if (matchedAssociates.value.length > 0) {
      for (const assoc of matchedAssociates.value) {
        rows.push({ kind: "associate", assoc, flatIndex: idx++ });
      }
    }
  }

  return rows;
});

const totalSelectable = computed(() => {
  return displayRows.value.filter(
    (r) => r.kind === "right" || r.kind === "associate",
  ).length;
});

function selectAtIndex(index: number) {
  const row = displayRows.value.find(
    (r) => (r.kind === "right" || r.kind === "associate") && r.flatIndex === index,
  );
  if (!row) return;
  if (row.kind === "right") emit("selectRight", row.right);
  else if (row.kind === "associate") emit("selectAssociate", row.assoc);
}

const { selectedIndex, resetIndex } = useListNavigation({
  itemCount: totalSelectable,
  onSelect: selectAtIndex,
  listRef,
  scrollStrategy: "selected-class",
});

function onUpdate(value: string) {
  query.value = value;
  resetIndex();

  if (debounceTimer) clearTimeout(debounceTimer);
  const version = ++searchVersion;

  if (value.trim().length >= 2) {
    searchingAssociates.value = true;
    debounceTimer = setTimeout(async () => {
      try {
        matchedAssociates.value = await searchAssociates(value.trim());
      } catch {
        matchedAssociates.value = [];
      } finally {
        if (version === searchVersion) searchingAssociates.value = false;
      }
    }, 300);
  } else {
    matchedAssociates.value = [];
    searchingAssociates.value = false;
  }
}

onUnmounted(() => {
  if (debounceTimer) clearTimeout(debounceTimer);
});
</script>

<template>
  <CommandInput
    :value="query"
    placeholder="Search rights or associates..."
    @update="onUpdate"
  />
  <div class="picker-divider" />
  <div ref="listRef" class="picker-list">
    <div v-if="displayRows.length === 0" class="empty">No matching rights</div>
    <template v-for="(row, i) in displayRows" :key="i">
      <div v-if="row.kind === 'header'" class="section-header">
        {{ row.label }}
      </div>
      <div
        v-else-if="row.kind === 'right'"
        class="picker-item"
        :class="{ selected: row.flatIndex === selectedIndex }"
        @click="emit('selectRight', row.right)"
        @mouseenter="selectedIndex = row.flatIndex"
      >
        <span class="picker-name">{{ row.right.rightName }}</span>
        <span class="picker-id">#{{ row.right.rightId }}</span>
      </div>
      <div
        v-else-if="row.kind === 'associate'"
        class="picker-item assoc-item"
        :class="{ selected: row.flatIndex === selectedIndex }"
        @click="emit('selectAssociate', row.assoc)"
        @mouseenter="selectedIndex = row.flatIndex"
      >
        <span class="assoc-nick">{{ row.assoc.nickname ?? '—' }}</span>
        <span class="assoc-name">
          {{ [row.assoc.firstName, row.assoc.lastName].filter(Boolean).join(' ') || '—' }}
        </span>
        <span class="assoc-id">#{{ row.assoc.assocId }}</span>
      </div>
      <div v-else-if="row.kind === 'searching'" class="searching-row">
        <div class="mini-spinner" />
        <span>Searching...</span>
      </div>
    </template>
  </div>
</template>

<style scoped>
@import "../assume-identity/picker-shared.css";

.picker-item {
  justify-content: space-between;
  padding: 10px 16px;
}

.picker-id {
  font-size: 12px;
  font-family: var(--font-mono);
  color: var(--text-secondary);
}

.assoc-item {
  gap: 12px;
  justify-content: flex-start;
}

.assoc-nick {
  font-size: 14px;
  font-family: var(--font-mono);
  color: var(--text-primary);
  min-width: 100px;
}

.assoc-name {
  font-size: 13px;
  color: var(--text-secondary);
  flex: 1;
}

.assoc-id {
  font-size: 12px;
  font-family: var(--font-mono);
  color: var(--text-secondary);
}

.searching-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 16px;
  color: var(--text-secondary);
  font-size: 13px;
}

.mini-spinner {
  width: 14px;
  height: 14px;
  border: 2px solid var(--border-subtle);
  border-top-color: var(--accent-blue);
  border-radius: 50%;
  animation: spin 0.6s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
