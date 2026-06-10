import { ref, computed } from "vue";
import { hideWindow } from "@/lib/tauri";
import { filterCommands } from "@/commands";
import { buildSoftCommands } from "@/lib/softCommands";
import type { PaletteCommand } from "@/commands/types";

export type PaletteMode = "browsing" | "command-active";

const mode = ref<PaletteMode>("browsing");
const searchQuery = ref("");
const selectedIndex = ref(0);
const activeCommand = ref<PaletteCommand | null>(null);
const previousCommand = ref<PaletteCommand | null>(null);
const returningToPrevious = ref(false);

const filteredCommands = computed(() => {
  const matches = filterCommands(searchQuery.value);
  // When nothing matches, offer contextual "soft commands" for the raw text
  // (URL, Jira key, JSON, math, …). Returns [] when no pattern matches, so the
  // normal empty state still shows for unrecognized text.
  if (matches.length > 0) return matches;
  return buildSoftCommands(searchQuery.value);
});

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

  /** Soft commands carry a one-shot `action` (run + dismiss); normal commands
   *  open their `component`. */
  function runOrSelect(cmd: PaletteCommand) {
    if (cmd.action) {
      Promise.resolve(cmd.action())
        .catch((e) => console.error("[soft-command]", e))
        .finally(() => dismiss());
      return;
    }
    selectCommand(cmd);
  }

  function confirmSelection() {
    const cmd = filteredCommands.value[selectedIndex.value];
    if (cmd) runOrSelect(cmd);
  }

  /** Activate the Nth visible command (0-indexed). Used by digit hotkeys
   *  1-9 in the palette so users can launch a command without arrow-keying. */
  function selectByIndex(index: number) {
    const cmd = filteredCommands.value[index];
    if (cmd) {
      selectedIndex.value = index;
      runOrSelect(cmd);
    }
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
    selectByIndex,
    onSearchChange,
  };
}
