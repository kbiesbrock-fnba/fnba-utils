<script setup lang="ts">
import { ref, computed } from "vue";
import type { IdentityUser } from "../../lib/tauri";
import { useListNavigation } from "../../composables/useListNavigation";
import CommandInput from "../CommandInput.vue";

const props = defineProps<{
  users: IdentityUser[];
  recentUsernames: string[];
}>();

const emit = defineEmits<{
  select: [user: IdentityUser];
  removeRecent: [username: string];
}>();

const query = ref("");
const listRef = ref<HTMLElement | null>(null);

type DisplayRow =
  | { kind: "header"; title: string }
  | { kind: "user"; user: IdentityUser; displayLabel: string; flatIndex: number; isRecent: boolean };

const displayData = computed(() => {
  const q = query.value.toLowerCase();
  const matching = q
    ? props.users.filter(
        (u) =>
          u.username.toLowerCase().includes(q) ||
          u.label.toLowerCase().includes(q),
      )
    : props.users;

  const rows: DisplayRow[] = [];
  let flatIndex = 0;

  // Recently used section
  if (props.recentUsernames.length > 0) {
    const recentItems: { user: IdentityUser; allLabels: string }[] = [];
    for (const username of props.recentUsernames) {
      const entries = matching.filter((u) => u.username === username);
      if (entries.length > 0) {
        const allLabels = [...new Set(entries.map((e) => e.label))]
          .sort()
          .join(" / ");
        recentItems.push({ user: entries[0], allLabels });
      }
    }
    if (recentItems.length > 0) {
      rows.push({ kind: "header", title: "Recently Used" });
      for (const item of recentItems) {
        rows.push({
          kind: "user",
          user: item.user,
          displayLabel: item.allLabels,
          flatIndex: flatIndex++,
          isRecent: true,
        });
      }
    }
  }

  // Group by label, alphabetically
  const groups = new Map<string, IdentityUser[]>();
  for (const u of matching) {
    const arr = groups.get(u.label) ?? [];
    arr.push(u);
    groups.set(u.label, arr);
  }
  for (const arr of groups.values()) {
    arr.sort((a, b) => a.username.localeCompare(b.username));
  }
  const sortedLabels = [...groups.keys()].sort((a, b) =>
    a.localeCompare(b),
  );

  for (const label of sortedLabels) {
    rows.push({ kind: "header", title: label });
    for (const user of groups.get(label)!) {
      rows.push({
        kind: "user",
        user,
        displayLabel: "",
        flatIndex: flatIndex++,
        isRecent: false,
      });
    }
  }

  return { rows, totalItems: flatIndex };
});

function getRowAtIndex(index: number) {
  for (const row of displayData.value.rows) {
    if (row.kind === "user" && row.flatIndex === index) return row;
  }
  return undefined;
}

const { selectedIndex, resetIndex } = useListNavigation({
  itemCount: () => displayData.value.totalItems,
  onSelect: (i) => {
    const row = getRowAtIndex(i);
    if (row) emit("select", row.user);
  },
  onEnterEmpty: () => {
    if (query.value.trim()) {
      emit("select", { username: query.value.trim(), label: "Custom" });
    }
  },
  extraKeys: [
    {
      key: "Delete",
      handler: () => {
        if (displayData.value.totalItems === 0) return false;
        const row = getRowAtIndex(selectedIndex.value);
        if (row?.isRecent) {
          emit("removeRecent", row.user.username);
          return;
        }
        return false;
      },
    },
  ],
  listRef,
  scrollStrategy: "data-index",
});

function onUpdate(value: string) {
  query.value = value;
  resetIndex();
}
</script>

<template>
  <CommandInput :value="query" placeholder="Select user..." @update="onUpdate" />
  <div class="picker-divider" />
  <div ref="listRef" class="picker-list">
    <div v-if="displayData.totalItems === 0 && query.trim()" class="empty use-custom">
      Press Enter to use <strong>{{ query.trim() }}</strong>
    </div>
    <div v-else-if="displayData.totalItems === 0" class="empty">No matching users</div>
    <template v-for="(row, i) in displayData.rows" :key="i">
      <div v-if="row.kind === 'header'" class="section-header">
        {{ row.title }}
      </div>
      <div
        v-else
        class="picker-item"
        :class="{ selected: row.flatIndex === selectedIndex }"
        :data-index="row.flatIndex"
        @click="emit('select', row.user)"
        @mouseenter="selectedIndex = row.flatIndex"
      >
        <span class="picker-name">{{ row.user.username }}</span>
        <span v-if="row.displayLabel" class="picker-labels">{{ row.displayLabel }}</span>
        <button
          v-if="row.isRecent"
          class="remove-btn"
          title="Remove from recent (Del)"
          @click.stop="emit('removeRecent', row.user.username)"
        >
          <svg viewBox="0 0 16 16" fill="currentColor" width="12" height="12">
            <path d="M3.72 3.72a.75.75 0 0 1 1.06 0L8 6.94l3.22-3.22a.75.75 0 1 1 1.06 1.06L9.06 8l3.22 3.22a.75.75 0 1 1-1.06 1.06L8 9.06l-3.22 3.22a.75.75 0 0 1-1.06-1.06L6.94 8 3.72 4.78a.75.75 0 0 1 0-1.06Z" />
          </svg>
        </button>
      </div>
    </template>
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

.section-header {
  padding: 10px 16px 4px;
  font-size: 11px;
  font-weight: 600;
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  user-select: none;
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

.remove-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  padding: 0;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  opacity: 0;
  transition: opacity 0.1s ease, background 0.1s ease, color 0.1s ease;
  flex-shrink: 0;
}

.picker-item:hover .remove-btn,
.picker-item.selected .remove-btn {
  opacity: 1;
}

.remove-btn:hover {
  background: var(--bg-hover);
  color: var(--accent-red);
}
</style>
