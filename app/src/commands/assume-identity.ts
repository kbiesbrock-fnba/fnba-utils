import type { PaletteCommand } from "./types";
import AssumeIdentityCommand from "../components/assume-identity/AssumeIdentityCommand.vue";

export const assumeIdentityCommand: PaletteCommand = {
  id: "assume-identity",
  name: "Assume Identity",
  description: "Switch SQL identity on a target server",
  icon: "\uD83C\uDFAD",
  keywords: ["sql", "identity", "login", "impersonate", "switch", "assume"],
  component: AssumeIdentityCommand,
  breadcrumbs: [
    { label: "Imposter", steps: ["imposter"] },
    { label: "User", steps: ["user"] },
    { label: "Connection", steps: ["connection"] },
    { label: "Confirm", steps: ["confirm", "executing", "result", "error"] },
  ],
};
