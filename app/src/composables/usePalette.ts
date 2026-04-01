import { ref, computed } from "vue";
import { hideWindow } from "@/lib/tauri";
import { filterCommands } from "@/commands";
import type { PaletteCommand } from "@/commands/types";

export type PaletteMode = "browsing" | "command-active";

const mode = ref<PaletteMode>("browsing");
const searchQuery = ref("");
const selectedIndex = ref(0);
const activeCommand = ref<PaletteCommand | null>(null);
const previousCommand = ref<PaletteCommand | null>(null);
const returningToPrevious = ref(false);

const filteredCommands = computed(() => filterCommands(searchQuery.value));

export function usePalette() {
  function reset() {
    mode.value = "browsing";
    searchQuery.value = "";
    selectedIndex.value = 0;
    activeCommand.value = null;
    previousCommand.value = null;
    returningToPrevious.value = false;
  }

  function dismiss() {
    reset();
    hideWindow();
  }

  function selectCommand(cmd: PaletteCommand) {
    if (mode.value === "command-active" && activeCommand.value) {
      previousCommand.value = activeCommand.value;
    }
    activeCommand.value = cmd;
    mode.value = "command-active";
    searchQuery.value = "";
    selectedIndex.value = 0;
  }

  function back() {
    if (mode.value === "command-active") {
      if (previousCommand.value) {
        activeCommand.value = previousCommand.value;
        previousCommand.value = null;
        returningToPrevious.value = true;
      } else {
        mode.value = "browsing";
        activeCommand.value = null;
      }
      searchQuery.value = "";
      selectedIndex.value = 0;
    } else {
      dismiss();
    }
  }

  function moveSelection(delta: number) {
    const len = filteredCommands.value.length;
    if (len === 0) return;
    selectedIndex.value = (selectedIndex.value + delta + len) % len;
  }

  function confirmSelection() {
    const cmd = filteredCommands.value[selectedIndex.value];
    if (cmd) selectCommand(cmd);
  }

  function onSearchChange(query: string) {
    searchQuery.value = query;
    selectedIndex.value = 0;
  }

  return {
    mode,
    searchQuery,
    selectedIndex,
    activeCommand,
    filteredCommands,
    returningToPrevious,
    reset,
    dismiss,
    selectCommand,
    back,
    moveSelection,
    confirmSelection,
    onSearchChange,
  };
}
