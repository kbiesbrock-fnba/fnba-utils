<script setup lang="ts">
// Thin dispatcher for the unified `#file-viewer` route. Reads this window's
// own registry entry (seeded via `seedEntry` at spawn time, before the
// WebviewWindow itself was even constructed) to decide which body to render.
// No shared visual shell beyond each body's own title bar — see
// useViewerWindowChrome.ts / ViewerTitleBar.vue for the plumbing they share.
import { defineAsyncComponent } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { readRegistry } from "../../lib/fileViewerRegistry";

const JsonViewerApp = defineAsyncComponent(
  () => import("../json-viewer/JsonViewerApp.vue"),
);
const MarkdownViewerApp = defineAsyncComponent(
  () => import("../markdown-viewer/MarkdownViewerApp.vue"),
);

// Defaults to "json" defensively if the entry is somehow missing — should
// never happen in practice since seedEntry always runs before the window is
// created (both for fresh spawns and for the restore/migration paths).
const kind = ((): "json" | "markdown" => {
  try {
    const label = getCurrentWindow().label;
    return readRegistry()[label]?.kind ?? "json";
  } catch {
    return "json";
  }
})();
</script>

<template>
  <MarkdownViewerApp v-if="kind === 'markdown'" />
  <JsonViewerApp v-else />
</template>
