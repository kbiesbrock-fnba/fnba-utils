import type { PaletteCommand } from "./types";
import ClipboardManagerLauncher from "../components/clipboard-manager/ClipboardManagerLauncher.vue";

export const clipboardManagerCommand: PaletteCommand = {
  id: "clipboard-manager",
  name: "Clipboard",
  description: "Browse clipboard history (Win+V)",
  icon: "\u{1F4CB}", // 📋
  keywords: [
    "clipboard",
    "history",
    "paste",
    "copy",
    "snippets",
    "buffer",
  ],
  component: ClipboardManagerLauncher,
};
