import { ref, computed } from "vue";
import { hideWindow } from "../lib/tauri";
import { commands, filterCommands } from "../commands";
import type { PaletteCommand } from "../commands/types";

export type PaletteMode = "browsing" | "command-active";

const mode = ref<PaletteMode>("browsing");
const searchQuery = ref("");
const selectedIndex = ref(0);
const activeCommand = ref<PaletteCommand | null>(null);

const filteredCommands = computed(() => filterCommands(searchQuery.value));

export function usePalette() {
  function reset() {
    mode.value = "browsing";
    searchQuery.value = "";
    selectedIndex.value = 0;
    activeCommand.value = null;
  }

  function dismiss() {
    reset();
    hideWindow();
  }

  function selectCommand(cmd: PaletteCommand) {
    activeCommand.value = cmd;
    mode.value = "command-active";
    searchQuery.value = "";
    selectedIndex.value = 0;
  }

  function back() {
    if (mode.value === "command-active") {
      mode.value = "browsing";
      activeCommand.value = null;
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
    reset,
    dismiss,
    selectCommand,
    back,
    moveSelection,
    confirmSelection,
    onSearchChange,
  };
}
