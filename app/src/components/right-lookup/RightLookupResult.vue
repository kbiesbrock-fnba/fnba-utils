<script setup lang="ts">
import { ref, computed } from "vue";
import type { RightInfo, RightAssociate } from "@/lib/tauri";
import { prefillUsername } from "@/composables/useAssumeIdentity";
import { usePalette } from "@/composables/usePalette";
import { assumeIdentityCommand } from "@/commands/assume-identity";
import { useListNavigation } from "@/composables/useListNavigation";
import CommandInput from "../CommandInput.vue";

const props = defineProps<{
  right: RightInfo;
  associates: RightAssociate[];
}>();

const query = ref("");
const activeAction = ref<"copy" | "assume">("assume");
const listRef = ref<HTMLElement | null>(null);
const copiedIndex = ref<number | null>(null);
const { selectCommand } = usePalette();

const filtered = computed(() => {
  if (!query.value) return props.associates;
  const q = query.value.toLowerCase();
  return props.associates.filter(
    (a) =>
      (a.nickname?.toLowerCase().includes(q)) ||
      (a.firstName?.toLowerCase().includes(q)) ||
      (a.lastName?.toLowerCase().includes(q)) ||
      String(a.assocId).includes(q),
  );
});

function copyNickname(assoc: RightAssociate, index: number) {
  const nick = assoc.nickname ?? String(assoc.assocId);
  navigator.clipboard.writeText(nick).then(() => {
    copiedIndex.value = index;
    setTimeout(() => (copiedIndex.value = null), 1500);
  });
}

function assumeIdentity(assoc: RightAssociate) {
  const nick = assoc.nickname ?? String(assoc.assocId);
  prefillUsername.value = nick;
  selectCommand(assumeIdentityCommand);
}

const { selectedIndex, resetIndex } = useListNavigation({
  itemCount: () => filtered.value.length,
  onSelect: (i) => {
    const assoc = filtered.value[i];
    if (activeAction.value === "assume") assumeIdentity(assoc);
    else copyNickname(assoc, i);
  },
  extraKeys: [
    {
      key: "Tab",
      handler: () => {
        activeAction.value = activeAction.value === "copy" ? "assume" : "copy";
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
  <div class="result-header">
    <span class="right-name">{{ right.rightName }}</span>
    <span class="badge">
      <template v-if="query && filtered.length !== associates.length">{{ filtered.length }}/</template>{{ associates.length }} associate{{ associates.length !== 1 ? 's' : '' }}
    </span>
  </div>
  <CommandInput
    :value="query"
    placeholder="Filter by username or name..."
    @update="onUpdate"
  />
  <div class="picker-divider" />
  <div ref="listRef" class="result-list">
    <div v-if="associates.length === 0" class="empty">
      No associates have this right
    </div>
    <div v-else-if="filtered.length === 0" class="empty">
      No matching associates
    </div>
    <div
      v-for="(assoc, i) in filtered"
      :key="assoc.assocId"
      class="result-row"
      :class="{ selected: i === selectedIndex }"
      @click="copyNickname(assoc, i)"
      @mouseenter="selectedIndex = i"
    >
      <span class="assoc-id">{{ assoc.assocId }}</span>
      <span class="assoc-nick">
        {{ assoc.nickname ?? '—' }}
        <span v-if="copiedIndex === i" class="copied-badge">Copied!</span>
      </span>
      <span class="assoc-name">
        {{ [assoc.firstName, assoc.lastName].filter(Boolean).join(' ') || '—' }}
      </span>
      <div class="row-actions">
        <button
          class="action-btn"
          :class="{ active: i === selectedIndex && activeAction === 'copy' }"
          title="Copy nickname"
          @click.stop="copyNickname(assoc, i)"
        >
          Copy
        </button>
        <button
          class="action-btn"
          :class="{ active: i === selectedIndex && activeAction === 'assume' }"
          title="Assume this identity"
          @click.stop="assumeIdentity(assoc)"
        >
          Assume
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.result-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
}

.right-name {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
}

.badge {
  font-size: 12px;
  color: var(--text-secondary);
  background: var(--bg-hover);
  padding: 2px 8px;
  border-radius: var(--radius-sm);
}

.picker-divider {
  height: 1px;
  background: var(--border-subtle);
}

.result-list {
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

.result-row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 16px;
  cursor: pointer;
  transition: background 0.1s ease;
  border-left: 3px solid transparent;
}

.result-row:hover {
  background: var(--bg-hover);
}

.result-row.selected {
  background: var(--bg-selected);
  border-left-color: var(--accent-blue);
}

.assoc-id {
  font-size: 12px;
  font-family: var(--font-mono);
  color: var(--text-secondary);
  min-width: 50px;
}

.assoc-nick {
  font-size: 14px;
  font-family: var(--font-mono);
  color: var(--text-primary);
  min-width: 100px;
  display: flex;
  align-items: center;
  gap: 6px;
}

.copied-badge {
  font-size: 11px;
  color: var(--accent-green, #4ade80);
  font-family: var(--font-sans);
}

.assoc-name {
  font-size: 13px;
  color: var(--text-secondary);
  flex: 1;
}

.row-actions {
  display: flex;
  gap: 4px;
  opacity: 0;
  transition: opacity 0.1s ease;
}

.result-row:hover .row-actions,
.result-row.selected .row-actions {
  opacity: 1;
}

.action-btn {
  padding: 2px 10px;
  border: 1px solid var(--border-subtle);
  background: transparent;
  color: var(--text-secondary);
  border-radius: var(--radius-sm);
  font-size: 11px;
  font-family: var(--font-sans);
  cursor: pointer;
  transition: border-color 0.15s ease, color 0.15s ease, background 0.15s ease;
}

.action-btn:hover {
  border-color: var(--text-secondary);
  color: var(--text-primary);
}

.action-btn.active {
  border-color: var(--accent-blue);
  color: var(--text-primary);
  background: var(--bg-hover);
}
</style>
