import type { PaletteCommand } from "./types";
import { openNewMarkdownViewerWindow } from "../lib/markdownViewerWindow";
import { openMarkdownFile } from "../lib/tauri";

export const markdownViewerCommand: PaletteCommand = {
  id: "markdown-viewer",
  name: "Markdown Viewer",
  description: "Render and edit Markdown — paste or type, preview, toggle to source",
  icon: "📝",
  keywords: ["markdown", "md", "preview", "render", "readme", "viewer", "docs"],
  action: () => openNewMarkdownViewerWindow(),
};

export const openMarkdownFileCommand: PaletteCommand = {
  id: "markdown-open-file",
  name: "Open Markdown File…",
  description: "Open a .md file from disk in the Markdown Viewer",
  icon: "📂",
  keywords: ["markdown", "md", "open", "file", "readme"],
  action: async () => {
    const f = await openMarkdownFile();
    if (f) await openNewMarkdownViewerWindow(f.content, f.path);
  },
};
