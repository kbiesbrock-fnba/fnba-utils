<script setup lang="ts">
import { ref, computed } from "vue";
import type { IdentityUser } from "@/lib/tauri";
import type { RecentEntry } from "@/composables/useAssumeIdentity";
import { useListNavigation } from "@/composables/useListNavigation";
import CommandInput from "../CommandInput.vue";
import LabelPrompt from "./LabelPrompt.vue";

const props = defineProps<{
  users: IdentityUser[];
  recentUsers: RecentEntry[];
}>();

const emit = defineEmits<{
  select: [user: IdentityUser];
  removeRecent: [username: string];
}>();

const query = ref("");
const listRef = ref<HTMLElement | null>(null);
const labelMode = ref<{ username: string } | null>(null);

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
  if (props.recentUsers.length > 0) {
    const recentItems: { user: IdentityUser; displayLabel: string }[] = [];
    for (const recent of props.recentUsers) {
      const entries = matching.filter((u) => u.username === recent.username);
      const matchesQuery = !q || recent.username.toLowerCase().includes(q) || recent.label.toLowerCase().includes(q);
      if (entries.length > 0 || matchesQuery) {
        const user = entries.length > 0
          ? entries[0]
          : { username: recent.username, label: recent.label };
        const userLabels = entries.length > 0
          ? [...new Set(entries.map((e) => e.label))].sort().join(" / ")
          : recent.label;
        const connPart = recent.connectionLabel || recent.connectionServer;
        const displayLabel = connPart ? `${userLabels} · ${connPart}` : userLabels;
        recentItems.push({ user, displayLabel });
      }
    }
    if (recentItems.length > 0) {
      rows.push({ kind: "header", title: "Recently Used" });
      for (const item of recentItems) {
        rows.push({
          kind: "user",
          user: item.user,
          displayLabel: item.displayLabel,
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
  itemCount: () => labelMode.value ? 0 : displayData.value.totalItems,
  onSelect: (i) => {
    const row = getRowAtIndex(i);
    if (row) emit("select", row.user);
  },
  onEnterEmpty: () => {
    if (query.value.trim()) {
      labelMode.value = { username: query.value.trim() };
    }
  },
  extraKeys: [
    {
      key: "Delete",
      handler: () => {
        if (labelMode.value) return false;
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

function onLabelConfirm(label: string) {
  if (!labelMode.value) return;
  emit("select", { username: labelMode.value.username, label });
  labelMode.value = null;
}

function onLabelCancel() {
  labelMode.value = null;
}
</script>

<template>
  <LabelPrompt
    v-if="labelMode"
    :value="labelMode.username"
    default-label="Other"
    @confirm="onLabelConfirm"
    @cancel="onLabelCancel"
  />
  <template v-else>
    <CommandInput key="query" :value="query" placeholder="Select user..." @update="onUpdate" />
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
</template>

<style src="./picker-shared.css" scoped></style>
<style scoped>
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
