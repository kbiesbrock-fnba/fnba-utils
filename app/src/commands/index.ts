import { ref, computed } from "vue";
import type { PaletteCommand } from "./types";
import { assumeIdentityCommand } from "./assume-identity";
import { rightLookupCommand } from "./right-lookup";
import { buildStandupCommand, buildStandupDescription } from "./standup";
import { getAppConfig } from "@/lib/tauri";

// Always-on commands.
const baseCommands: PaletteCommand[] = [assumeIdentityCommand, rightLookupCommand];

/** Live list of palette commands. Mutated as opt-in features are detected. */
export const commandsRef = ref<PaletteCommand[]>([...baseCommands]);

/** Computed accessor for templates that just need the array. */
export const commands = computed(() => commandsRef.value);

export function filterCommands(query: string): PaletteCommand[] {
  if (!query) return commandsRef.value;
  const q = query.toLowerCase();
  return commandsRef.value.filter(
    (cmd) =>
      cmd.name.toLowerCase().includes(q) ||
      cmd.description.toLowerCase().includes(q) ||
      cmd.keywords.some((k) => k.includes(q)),
  );
}

function upsertCommand(cmd: PaletteCommand) {
  const idx = commandsRef.value.findIndex((c) => c.id === cmd.id);
  if (idx === -1) {
    commandsRef.value = [...commandsRef.value, cmd];
  } else {
    const next = [...commandsRef.value];
    next[idx] = cmd;
    commandsRef.value = next;
  }
}

function removeCommand(id: string) {
  commandsRef.value = commandsRef.value.filter((c) => c.id !== id);
}

let standupRefreshing = false;

/**
 * Refresh the Standup command's "Last run: ..." subtitle from the backend.
 * Safe to call repeatedly; coalesces concurrent calls.
 */
export async function refreshStandupCommand() {
  if (standupRefreshing) return;
  standupRefreshing = true;
  try {
    const desc = await buildStandupDescription();
    upsertCommand(buildStandupCommand(desc));
  } finally {
    standupRefreshing = false;
  }
}

let initialized = false;

/** Detect opt-in features and add their palette commands. Idempotent. */
export async function initCommands() {
  if (initialized) return;
  initialized = true;
  try {
    const cfg = await getAppConfig();
    if (cfg.standup.enabled) {
      await refreshStandupCommand();
    } else {
      removeCommand("standup");
    }
  } catch (e) {
    console.warn("initCommands: failed to load app config", e);
  }
}
