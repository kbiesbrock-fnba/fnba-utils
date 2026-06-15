import type { PaletteCommand } from "./types";
import { openNewMarkdownViewerWindow } from "../lib/markdownViewerWindow";

export const markdownViewerCommand: PaletteCommand = {
  id: "markdown-viewer",
  name: "Markdown Viewer",
  description: "Render and edit Markdown — paste or type, preview, toggle to source",
  icon: "📝",
  keywords: ["markdown", "md", "preview", "render", "readme", "viewer", "docs"],
  action: () => openNewMarkdownViewerWindow(),
};
