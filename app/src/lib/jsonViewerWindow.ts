// Spawns JSON Viewer windows. ALL viewers are equal dynamic windows labeled
// `json-viewer:<timestamp>-<seq>`. Win+Shift+J opens a switcher when any
// viewers are open, or spawns a fresh one when none exist. Mirrors the
// dynamic-window pattern in `panels.ts`.

const JSON_VIEWER_OPTIONS: Record<string, unknown> = {
  url: "index.html#json-viewer",
  width: 1000,
  height: 700,
  minWidth: 600,
  minHeight: 400,
  resizable: true,
  decorations: false,
  shadow: true,
  transparent: false,
  backgroundColor: "#1e1e1e",
  visible: true,
  alwaysOnTop: false,
  skipTaskbar: false,
  title: "JSON Viewer",
};

// Monotonic suffix so two spawns in the same millisecond can't collide.
let seq = 0;

/**
 * Create and focus a brand-new JSON Viewer window with a unique label.
 *
 * `initialContent`, if given, is stashed in localStorage and picked up by the
 * new window on mount (see JsonViewerApp) — used by the palette's "Open in
 * JSON Viewer" soft command to seed the window with a pasted blob.
 */
export async function openNewJsonViewerWindow(initialContent?: string): Promise<void> {
  if (initialContent != null) {
    try {
      localStorage.setItem("fnba-utils:json-viewer-pending", initialContent);
    } catch {
      // ignore
    }
  }
  const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
  const label = `json-viewer:${Date.now()}-${seq++}`;
  const win = new WebviewWindow(label, JSON_VIEWER_OPTIONS);
  // visible:true shows it on creation; just pull focus once it's ready.
  win.once("tauri://created", () => {
    void win.setFocus().catch(() => {});
  });
}

/**
 * Reopen every JSON Viewer window that has a registry entry but no live window.
 * Called once at app startup to restore windows killed by a recompile, quit, or crash.
 * Windows the user explicitly closed are not in the registry (removeEntry clears them)
 * so they are not restored.
 */
export async function restoreJsonViewerWindows(): Promise<void> {
  try {
    const { readRegistry } = await import("./jsonViewerRegistry");
    const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");

    const registry = readRegistry();
    const allWindows = await WebviewWindow.getAll();
    const liveLabels = new Set(allWindows.map((w) => w.label));

    for (const [label, entry] of Object.entries(registry)) {
      if (!label.startsWith("json-viewer:")) continue;
      if (liveLabels.has(label)) continue;

      // Build window options, overlaying saved geometry when available.
      const opts: Record<string, unknown> = {
        ...JSON_VIEWER_OPTIONS,
        ...(entry.win
          ? {
              x: entry.win.x,
              y: entry.win.y,
              width: entry.win.width,
              height: entry.win.height,
              alwaysOnTop: entry.win.pinned,
            }
          : {}),
      };

      // Reuse the saved label so the window hydrates itself from the same
      // registry entry on mount (label is the key into localStorage registry).
      const win = new WebviewWindow(label, opts);

      if (entry.win?.maximized) {
        win.once("tauri://created", () => {
          void win.maximize().catch(() => {});
          // Do NOT setFocus — don't steal focus from whatever the user was doing.
        });
      }
      // No setFocus for restored windows.
    }
  } catch {
    // Restoration is best-effort; never crash the app over it.
  }
}
