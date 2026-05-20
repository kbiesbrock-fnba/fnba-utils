import type { PaletteCommand } from "./types";
import { assumeIdentityCommand } from "./assume-identity";
import { rightLookupCommand } from "./right-lookup";
import { newClaudeSessionCommand } from "./new-claude-session";

export const commands: PaletteCommand[] = [
  assumeIdentityCommand,
  rightLookupCommand,
  newClaudeSessionCommand,
];

export function filterCommands(query: string): PaletteCommand[] {
  if (!query) return commands;
  const q = query.toLowerCase();
  return commands.filter(
    (cmd) =>
      cmd.name.toLowerCase().includes(q) ||
      cmd.description.toLowerCase().includes(q) ||
      cmd.keywords.some((k) => k.includes(q)),
  );
}
