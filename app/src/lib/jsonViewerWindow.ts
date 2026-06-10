// Spawns JSON Viewer windows. The static `json-viewer` window (defined in
// tauri.conf.json) is the Win+Shift+J quick-access primary; this helper creates
// *additional* dynamically-labeled windows so the user can have several open at
// once. Mirrors the dynamic-window pattern in `panels.ts`.

const JSON_VIEWER_OPTIONS: Record<string, unknown> = {
  url: "index.html#json-viewer",
  width: 1000,
  height: 700,
  minWidth: 600,
  minHeight: 400,
  resizable: true,
  decorations: false,
  shadow: false,
  transparent: true,
  backgroundColor: "#00000000",
  visible: true,
  alwaysOnTop: false,
  skipTaskbar: false,
  title: "JSON Viewer",
};

// Monotonic suffix so two spawns in the same millisecond can't collide.
let seq = 0;

/** Create and focus a brand-new JSON Viewer window with a unique label. */
export async function openNewJsonViewerWindow(): Promise<void> {
  const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
  const label = `json-viewer:${Date.now()}-${seq++}`;
  const win = new WebviewWindow(label, JSON_VIEWER_OPTIONS);
  // visible:true shows it on creation; just pull focus once it's ready.
  win.once("tauri://created", () => {
    void win.setFocus().catch(() => {});
  });
}
