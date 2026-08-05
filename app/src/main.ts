import { createApp } from "vue";
import App from "./App.vue";
import "./style.css";
import { initCommands } from "./commands";

// Fire-and-forget — don't block mount on detecting opt-in features.
void initCommands();

createApp(App).mount("#app");

// Win+Shift+J zero-windows case: Rust emits "json-viewer-new" to the always-alive
// main palette window when no json-viewer:/markdown-viewer: windows exist and the
// switcher would be empty. (Event name kept as-is — emitted by
// src-tauri/src/lib.rs, which is out of scope for this change.)
if (!window.location.hash) {
  import("@tauri-apps/api/event").then(({ listen }) => {
    void listen("json-viewer-new", () => {
      void import("./lib/fileViewerWindow").then((m) => m.openNewFileViewerWindow({ kind: "json" }));
    });
  });

  // Reopens any File Viewer windows (JSON or Markdown) killed by a recompile,
  // app quit, or crash. Windows the user explicitly closed are not in the
  // registry and stay closed. This also runs the one-time legacy-registry
  // migration sweep on its first call (see fileViewerWindow.ts) — the
  // viewer-doc cleanup sweep below is chained to run only AFTER this
  // resolves, so it never sees a not-yet-migrated legacy entry as "absent"
  // and deletes an unsaved doc that's about to be migrated forward.
  void import("./lib/fileViewerWindow").then(async (m) => {
    await m.restoreFileViewerWindows();

    // Sweep doc-cache files (both kinds) orphaned by a crash: keep only paths
    // still referenced by the registry.
    try {
      const { readRegistry } = await import("./lib/fileViewerRegistry");
      const { cleanupViewerDocs } = await import("./lib/tauri");
      const keep = Object.values(readRegistry())
        .map((e: any) => e?.state?.docPath)
        .filter((p): p is string => typeof p === "string" && p.length > 0);
      await cleanupViewerDocs(keep);
    } catch { /* best-effort */ }
  });
}
