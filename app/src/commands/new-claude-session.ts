import type { PaletteCommand } from "./types";
import NewSessionCommand from "../components/new-claude-session/NewSessionCommand.vue";

export const newClaudeSessionCommand: PaletteCommand = {
  id: "new-claude-session",
  name: "New Claude Session",
  description: "Launch a Claude Code session in a chosen project",
  icon: "✨",
  keywords: ["claude", "new", "session", "launch", "code", "start"],
  component: NewSessionCommand,
  breadcrumbs: [
    { label: "Configure", steps: ["form"] },
    { label: "Launching", steps: ["launching"] },
    { label: "Error", steps: ["error"] },
  ],
};
