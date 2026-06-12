import type { PaletteCommand } from "./types";
import { openNewJsonViewerWindow } from "../lib/jsonViewerWindow";

export const jsonViewerCommand: PaletteCommand = {
  id: "json-viewer",
  name: "JSON Viewer",
  description: "View, search, and transform JSON with tree view and multiple formats",
  icon: "🔍",
  keywords: ["json", "tree", "flatten", "schema", "diff", "format", "transform"],
  action: () => openNewJsonViewerWindow(),
};
