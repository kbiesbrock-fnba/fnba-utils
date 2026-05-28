import type { PaletteCommand } from "./types";
import AssumeIdentityCommand from "../components/assume-identity/AssumeIdentityCommand.vue";

export const assumeIdentityCommand: PaletteCommand = {
  id: "assume-identity",
  name: "Assume Identity (Search by Rights)",
  description: "Become a user \u2014 search by name or by a right's holders",
  icon: "\uD83C\uDFAD",
  keywords: [
    "sql",
    "identity",
    "login",
    "impersonate",
    "switch",
    "assume",
    "right",
    "rights",
    "permission",
    "access",
    "associate",
    "lookup",
    "who has",
  ],
  component: AssumeIdentityCommand,
  breadcrumbs: [
    { label: "Imposter", steps: ["imposter"] },
    { label: "User", steps: ["user", "userRights"] },
    { label: "Connection", steps: ["connection"] },
    { label: "Confirm", steps: ["confirm", "executing", "result", "error"] },
  ],
};
