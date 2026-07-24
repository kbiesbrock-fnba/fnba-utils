import type { PaletteCommand } from "./types";

export const dockerWidgetCommand: PaletteCommand = {
  id: "docker-widget",
  name: "Docker Widget",
  description: "Show the Docker container gadget",
  icon: "\u{1F433}", // 🐳
  keywords: ["docker", "containers", "compose", "engine", "widget"],
  action: async () => {
    const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
    const w = await WebviewWindow.getByLabel("docker-widget");
    if (w) {
      await w.show();
      await w.setAlwaysOnTop(true);
      // NO setFocus — keep non-activating
    }
  },
};
