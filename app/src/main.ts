import { createApp } from "vue";
import App from "./App.vue";
import "./style.css";
import { initCommands } from "./commands";

// Fire-and-forget — don't block mount on detecting opt-in features.
void initCommands();

createApp(App).mount("#app");

// Win+Shift+J zero-windows case: Rust emits "json-viewer-new" to the always-alive
// main palette window when no json-viewer: windows exist and the switcher would be empty.
if (!window.location.hash) {
  import("@tauri-apps/api/event").then(({ listen }) => {
    void listen("json-viewer-new", () => {
      void import("./lib/jsonViewerWindow").then((m) => m.openNewJsonViewerWindow());
    });
  });

  // Reopens any JSON Viewer windows killed by a recompile, app quit, or crash.
  // Windows the user explicitly closed are not in the registry and stay closed.
  void import("./lib/jsonViewerWindow").then((m) => m.restoreJsonViewerWindows());

  // Reopens any Markdown Viewer windows killed by a recompile, app quit, or crash.
  void import("./lib/markdownViewerWindow").then((m) => m.restoreMarkdownViewerWindows());

  // Sweep markdown-doc files orphaned by a crash: keep only paths still in the registry.
  void (async () => {
    try {
      const { readRegistry } = await import("./lib/markdownViewerRegistry");
      const { cleanupMarkdownDocs } = await import("./lib/tauri");
      const keep = Object.values(readRegistry())
        .map((e: any) => e?.state?.docPath)
        .filter((p): p is string => typeof p === "string" && p.length > 0);
      await cleanupMarkdownDocs(keep);
    } catch { /* best-effort */ }
  })();
}
