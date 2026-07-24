<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount } from "vue";
import { usePalette } from "@/composables/usePalette";
import { useKeyLayer, KEY_PRIORITY } from "@/composables/useKeyLayer";
import { openAppDataFolder, onPaletteShown } from "@/lib/tauri";
import CommandInput from "./CommandInput.vue";
import CommandList from "./CommandList.vue";
import StatusBar from "./StatusBar.vue";

const {
  mode,
  searchQuery,
  selectedIndex,
  activeCommand,
  filteredCommands,
  dismiss,
  back,
  moveSelection,
  confirmSelection,
  chainSelection,
  selectByIndex,
  onSearchChange,
  reset,
} = usePalette();

const commandRef = ref<{ step: string } | null>(null);

const activeBreadcrumbIndex = computed(() => {
  const crumbs = activeCommand.value?.breadcrumbs;
  const step = commandRef.value?.step;
  if (!crumbs || !step) return -1;
  return crumbs.findIndex((b) => b.steps.includes(step));
});

async function openDataFolder() {
  try {
    await openAppDataFolder();
    dismiss();
  } catch (e) {
    console.error("Failed to open app data folder", e);
  }
}

// 1-9 in browsing mode + empty search → launch the Nth command. While the
// user is typing a search query digits fall through so "1" still types into
// the input. 0 is intentionally unbound (no 10th-slot ambiguity).
//
// preventDefault is opt-in here so the digit reaches the search input in the
// fall-through case; we call preventDefault() ourselves inside the handler
// only when we're actually launching a command.
const digitBindings = ["1", "2", "3", "4", "5", "6", "7", "8", "9"].map(
  (key) => ({
    key,
    preventDefault: false,
    handler: (e: KeyboardEvent) => {
      if (mode.value !== "browsing") return false;
      if (searchQuery.value !== "") return false;
      const index = Number(key) - 1;
      if (index >= filteredCommands.value.length) return false;
      e.preventDefault();
      selectByIndex(index);
      return;
    },
  }),
);

useKeyLayer(
  [
    {
      key: "Escape",
      handler: () => { back(); },
    },
    {
      key: "ArrowDown",
      handler: () => {
        if (mode.value === "browsing") { moveSelection(1); return; }
        return false;
      },
    },
    {
      key: "ArrowUp",
      handler: () => {
        if (mode.value === "browsing") { moveSelection(-1); return; }
        return false;
      },
    },
    {
      key: "Enter",
      handler: (e: KeyboardEvent) => {
        if (mode.value !== "browsing") return false;
        // Ctrl+Shift+Enter → chain the result back into the query (calculator flow)
        if (e.ctrlKey && e.shiftKey) { chainSelection(); return; }
        confirmSelection();
      },
    },
    ...digitBindings,
  ],
  { priority: KEY_PRIORITY.PALETTE },
);

// Reset to a fresh browsing state whenever the palette is shown via the global
// hotkey — the Rust side hides without resetting, so a palette left in
// command-active mode would otherwise reopen without the search input. This
// guarantees CommandInput is mounted; CommandInput re-focuses on the same event.
let unlistenShown: (() => void) | null = null;
let disposed = false;
onMounted(async () => {
  const un = await onPaletteShown(() => reset());
  if (disposed) un();
  else unlistenShown = un;
});
onBeforeUnmount(() => {
  disposed = true;
  unlistenShown?.();
});
</script>

<template>
  <div class="palette">
    <template v-if="mode === 'browsing'">
      <button
        class="settings-btn"
        title="Open app data folder (%LOCALAPPDATA%\fnba-utils)"
        @click="openDataFolder"
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="14" height="14">
          <circle cx="12" cy="12" r="3" />
          <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z" />
        </svg>
      </button>
      <CommandInput
        :value="searchQuery"
        placeholder="Type a command..."
        @update="onSearchChange"
      />
      <div class="palette-divider" />
      <CommandList
        :commands="filteredCommands"
        :selected-index="selectedIndex"
        :show-hotkeys="searchQuery === ''"
        @select="(i) => { selectedIndex = i; confirmSelection(); }"
      />
      <StatusBar hint="↑↓ Navigate  ⏎ Select  1-9 Jump  ⎋ Close" show-version />
    </template>

    <template v-else-if="mode === 'command-active' && activeCommand && activeCommand.component">
      <div class="breadcrumb">
        <button class="breadcrumb-back" @click="back">
          <svg viewBox="0 0 20 20" fill="currentColor" width="14" height="14">
            <path fill-rule="evenodd" d="M12.79 5.23a.75.75 0 01-.02 1.06L8.832 10l3.938 3.71a.75.75 0 11-1.04 1.08l-4.5-4.25a.75.75 0 010-1.08l4.5-4.25a.75.75 0 011.06.02z" clip-rule="evenodd" />
          </svg>
        </button>
        <span class="breadcrumb-icon">{{ activeCommand.icon }}</span>
        <span class="breadcrumb-name">{{ activeCommand.name }}</span>
        <template v-if="activeCommand.breadcrumbs?.length">
          <span class="breadcrumb-sep breadcrumb-sep-name">/</span>
          <template v-for="(crumb, i) in activeCommand.breadcrumbs" :key="crumb.label">
            <span v-if="i > 0" class="breadcrumb-sep">/</span>
            <span
              class="breadcrumb-step"
              :class="{
                'breadcrumb-step-done': i < activeBreadcrumbIndex,
                'breadcrumb-step-active': i === activeBreadcrumbIndex,
                'breadcrumb-step-upcoming': i > activeBreadcrumbIndex,
              }"
            >{{ crumb.label }}</span>
          </template>
        </template>
      </div>
      <div class="palette-divider" />
      <component
        ref="commandRef"
        :is="activeCommand.component"
        @back="back"
        @dismiss="dismiss"
      />
    </template>
  </div>
</template>

<style scoped>
.palette {
  position: relative;
  width: 100%;
  max-width: 632px;
  background: var(--bg-primary);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: var(--radius-lg);
  box-shadow:
    0 0 0 1px rgba(255, 255, 255, 0.06),
    0 0 20px rgba(96, 165, 250, 0.08),
    0 25px 50px -12px rgba(0, 0, 0, 0.6);
  overflow: hidden;
  display: flex;
  flex-direction: column;
  animation: palette-in 0.15s ease-out;
}

.settings-btn {
  position: absolute;
  top: 10px;
  right: 10px;
  z-index: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border: none;
  background: transparent;
  color: var(--text-placeholder);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: background 0.1s ease, color 0.1s ease;
}

.settings-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

@keyframes palette-in {
  from {
    opacity: 0;
    transform: translateY(-8px) scale(0.98);
  }
  to {
    opacity: 1;
    transform: translateY(0) scale(1);
  }
}

.palette-divider {
  height: 1px;
  background: var(--border-subtle);
}

.breadcrumb {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 16px;
}

.breadcrumb-back {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border: none;
  background: var(--bg-hover);
  color: var(--text-secondary);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: background 0.1s ease, color 0.1s ease;
}

.breadcrumb-back:hover {
  background: var(--bg-selected);
  color: var(--text-primary);
}

.breadcrumb-icon {
  font-size: 14px;
}

.breadcrumb-name {
  font-size: 13px;
  font-weight: 500;
  color: var(--text-secondary);
}

.breadcrumb-sep {
  font-size: 11px;
  color: var(--text-placeholder);
}

.breadcrumb-sep-name {
  margin-left: 2px;
}

.breadcrumb-step {
  font-size: 12px;
  transition: color 0.15s ease;
}

.breadcrumb-step-done {
  color: var(--text-secondary);
}

.breadcrumb-step-active {
  color: var(--text-primary);
  font-weight: 600;
}

.breadcrumb-step-upcoming {
  color: var(--text-placeholder);
}
</style>
