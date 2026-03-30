<script setup lang="ts">
import { ref, computed } from "vue";
import { usePalette } from "../composables/usePalette";
import { useKeyLayer, KEY_PRIORITY } from "../composables/useKeyLayer";
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
  onSearchChange,
} = usePalette();

const commandRef = ref<{ step: string } | null>(null);

const activeBreadcrumbIndex = computed(() => {
  const crumbs = activeCommand.value?.breadcrumbs;
  const step = commandRef.value?.step;
  if (!crumbs || !step) return -1;
  return crumbs.findIndex((b) => b.steps.includes(step));
});

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
      handler: () => {
        if (mode.value === "browsing") { confirmSelection(); return; }
        return false;
      },
    },
  ],
  { priority: KEY_PRIORITY.PALETTE },
);
</script>

<template>
  <div class="palette">
    <template v-if="mode === 'browsing'">
      <CommandInput
        :value="searchQuery"
        placeholder="Type a command..."
        @update="onSearchChange"
      />
      <div class="palette-divider" />
      <CommandList
        :commands="filteredCommands"
        :selected-index="selectedIndex"
        @select="(i) => { selectedIndex = i; confirmSelection(); }"
      />
      <StatusBar hint="↑↓ Navigate  ⏎ Select  ⎋ Close" />
    </template>

    <template v-else-if="mode === 'command-active' && activeCommand">
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
