<script setup lang="ts">
import { ref, computed, onUnmounted } from "vue";
import CommandInput from "../CommandInput.vue";
import { searchAssociates, type RightInfo, type RightAssociate } from "@/lib/tauri";
import { useListNavigation } from "@/composables/useListNavigation";
import type { RecentRight } from "@/composables/useRightLookup";

const props = defineProps<{
  rights: RightInfo[];
  recentRights: RecentRight[];
  server: string;
}>();

const emit = defineEmits<{
  selectRight: [right: RightInfo];
  selectAssociate: [assoc: RightAssociate];
  removeRecent: [rightId: number];
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

const filteredRecents = computed(() => {
  const q = query.value.toLowerCase();
  const out: RightInfo[] = [];
  for (const recent of props.recentRights) {
    if (q && !recent.rightName.toLowerCase().includes(q)) continue;
    const match = props.rights.find((r) => r.rightName === recent.rightName);
    out.push(match ?? { rightId: recent.rightId, rightName: recent.rightName });
  }
  return out;
});

type DisplayRow =
  | { kind: "header"; label: string }
  | { kind: "right"; right: RightInfo; flatIndex: number; isRecent: boolean }
  | { kind: "associate"; assoc: RightAssociate; flatIndex: number }
  | { kind: "searching" };

const displayRows = computed(() => {
  const rows: DisplayRow[] = [];
  let idx = 0;

  if (filteredRecents.value.length > 0) {
    rows.push({ kind: "header", label: "Recently Used" });
    for (const right of filteredRecents.value) {
      rows.push({ kind: "right", right, flatIndex: idx++, isRecent: true });
    }
  }

  if (filteredRights.value.length > 0) {
    if (query.value || filteredRecents.value.length > 0) {
      rows.push({ kind: "header", label: "Rights" });
    }
    for (const right of filteredRights.value) {
      rows.push({ kind: "right", right, flatIndex: idx++, isRecent: false });
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

function rowAtIndex(index: number) {
  return displayRows.value.find(
    (r) => (r.kind === "right" || r.kind === "associate") && r.flatIndex === index,
  );
}

function selectAtIndex(index: number) {
  const row = rowAtIndex(index);
  if (!row) return;
  if (row.kind === "right") emit("selectRight", row.right);
  else if (row.kind === "associate") emit("selectAssociate", row.assoc);
}

const { selectedIndex, resetIndex } = useListNavigation({
  itemCount: totalSelectable,
  onSelect: selectAtIndex,
  extraKeys: [
    {
      key: "Delete",
      handler: () => {
        if (totalSelectable.value === 0) return false;
        const row = rowAtIndex(selectedIndex.value);
        if (row?.kind === "right" && row.isRecent) {
          emit("removeRecent", row.right.rightId);
          return;
        }
        return false;
      },
    },
  ],
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
        matchedAssociates.value = await searchAssociates(props.server, value.trim());
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
        <button
          v-if="row.isRecent"
          class="remove-btn"
          title="Remove from recent (Del)"
          @click.stop="emit('removeRecent', row.right.rightId)"
        >
          <svg viewBox="0 0 16 16" fill="currentColor" width="12" height="12">
            <path d="M3.72 3.72a.75.75 0 0 1 1.06 0L8 6.94l3.22-3.22a.75.75 0 1 1 1.06 1.06L9.06 8l3.22 3.22a.75.75 0 1 1-1.06 1.06L8 9.06l-3.22 3.22a.75.75 0 0 1-1.06-1.06L6.94 8 3.72 4.78a.75.75 0 0 1 0-1.06Z" />
          </svg>
        </button>
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
