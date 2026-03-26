<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick, watch } from "vue";
import type { RightInfo, RightAssociate } from "../../lib/tauri";
import { prefillUsername } from "../../composables/useAssumeIdentity";
import { usePalette } from "../../composables/usePalette";
import { assumeIdentityCommand } from "../../commands/assume-identity";

const props = defineProps<{
  right: RightInfo;
  associates: RightAssociate[];
}>();

const selectedIndex = ref(0);
const listRef = ref<HTMLElement | null>(null);
const copiedIndex = ref<number | null>(null);
const { selectCommand } = usePalette();

function scrollToSelected() {
  nextTick(() => {
    const list = listRef.value;
    if (!list) return;
    const item = list.children[selectedIndex.value] as HTMLElement | undefined;
    item?.scrollIntoView({ block: "nearest" });
  });
}

watch(selectedIndex, scrollToSelected);

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

function onKeydown(e: KeyboardEvent) {
  if (props.associates.length === 0) return;

  if (e.key === "ArrowDown") {
    e.preventDefault();
    selectedIndex.value =
      (selectedIndex.value + 1) % props.associates.length;
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    selectedIndex.value =
      (selectedIndex.value - 1 + props.associates.length) %
      props.associates.length;
  } else if (e.key === "Enter") {
    e.preventDefault();
    e.stopPropagation();
    copyNickname(props.associates[selectedIndex.value], selectedIndex.value);
  }
}

onMounted(() => window.addEventListener("keydown", onKeydown, true));
onUnmounted(() => window.removeEventListener("keydown", onKeydown, true));
</script>

<template>
  <div class="result-header">
    <span class="right-name">{{ right.rightName }}</span>
    <span class="badge">{{ associates.length }} associate{{ associates.length !== 1 ? 's' : '' }}</span>
  </div>
  <div class="picker-divider" />
  <div ref="listRef" class="result-list">
    <div v-if="associates.length === 0" class="empty">
      No associates have this right
    </div>
    <div
      v-for="(assoc, i) in associates"
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
      <button
        class="assume-btn"
        title="Assume this identity"
        @click.stop="assumeIdentity(assoc)"
      >
        Assume
      </button>
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

.assume-btn {
  padding: 2px 10px;
  border: 1px solid var(--border-subtle);
  background: transparent;
  color: var(--text-secondary);
  border-radius: var(--radius-sm);
  font-size: 11px;
  font-family: var(--font-sans);
  cursor: pointer;
  opacity: 0;
  transition: opacity 0.1s ease, border-color 0.15s ease, color 0.15s ease;
}

.result-row:hover .assume-btn,
.result-row.selected .assume-btn {
  opacity: 1;
}

.assume-btn:hover {
  border-color: var(--text-secondary);
  color: var(--text-primary);
}
</style>
