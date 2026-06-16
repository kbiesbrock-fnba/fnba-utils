// Spawns Markdown Viewer windows. ALL viewers are equal dynamic windows labeled
// `markdown-viewer:<timestamp>-<seq>`. Win+Shift+M opens a switcher when any
// viewers are open, or spawns a fresh one when none exist. Mirrors the
// dynamic-window pattern used by `jsonViewerWindow.ts`.

const MARKDOWN_VIEWER_OPTIONS: Record<string, unknown> = {
  url: "index.html#markdown-viewer",
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
  title: "Markdown Viewer",
};

// Monotonic suffix so two spawns in the same millisecond can't collide.
let seq = 0;

/**
 * Create and focus a brand-new Markdown Viewer window with a unique label.
 *
 * `initialContent`, if given, is written to disk and pre-seeded into the
 * registry BEFORE the window is created — the new window hydrates from its
 * registry entry on mount (no pending-localStorage handoff needed).
 *
 * `filePath`, if given, binds the window to a real file on disk (Open / Save flow).
 */
export async function openNewMarkdownViewerWindow(initialContent?: string, filePath?: string): Promise<void> {
  const label = `markdown-viewer:${Date.now()}-${seq++}`;

  if ((initialContent != null && initialContent.trim() !== "") || filePath) {
    try {
      const { writeMarkdownDoc } = await import("./tauri");
      const { saveState, touchEntry } = await import("./markdownViewerRegistry");
      const docPath = await writeMarkdownDoc(label, initialContent ?? "");
      const seedMode = (initialContent && initialContent.trim()) ? "preview" : "edit";
      saveState(label, { docPath, mode: seedMode, filePath: filePath ?? null, dirty: false });
      touchEntry(label, (initialContent ?? "").replace(/\s+/g, " ").trim().slice(0, 60));
    } catch {
      // best-effort; window still opens empty
    }
  }

  const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
  const win = new WebviewWindow(label, MARKDOWN_VIEWER_OPTIONS);
  // visible:true shows it on creation; just pull focus once it's ready.
  win.once("tauri://created", () => {
    void win.setFocus().catch(() => {});
  });
}

/**
 * Reopen every Markdown Viewer window that has a registry entry but no live window.
 * Called once at app startup to restore windows killed by a recompile, quit, or crash.
 * Windows the user explicitly closed are not in the registry (removeEntry clears them)
 * so they are not restored.
 */
export async function restoreMarkdownViewerWindows(): Promise<void> {
  try {
    const { readRegistry } = await import("./markdownViewerRegistry");
    const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");

    const registry = readRegistry();
    const allWindows = await WebviewWindow.getAll();
    const liveLabels = new Set(allWindows.map((w) => w.label));

    for (const [label, entry] of Object.entries(registry)) {
      if (!label.startsWith("markdown-viewer:")) continue;
      if (liveLabels.has(label)) continue;

      // Build window options, overlaying saved geometry when available.
      const opts: Record<string, unknown> = {
        ...MARKDOWN_VIEWER_OPTIONS,
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
