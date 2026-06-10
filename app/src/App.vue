<script setup lang="ts">
import { defineAsyncComponent } from "vue";
import { usePalette } from "./composables/usePalette";

// Each Tauri window loads the same index.html with a different hash, but only
// ever renders one of these components. defineAsyncComponent + dynamic import
// puts each in its own chunk so a given window only pays for the code it
// actually uses.
const CommandPalette = defineAsyncComponent(
  () => import("./components/CommandPalette.vue"),
);
const MissionControlApp = defineAsyncComponent(
  () => import("./components/mission-control/MissionControlApp.vue"),
);
const SessionDetailApp = defineAsyncComponent(
  () => import("./components/session-detail/SessionDetailApp.vue"),
);
const SqlQueryApp = defineAsyncComponent(
  () => import("./components/sql-query/SqlQueryApp.vue"),
);
const StandupPanelApp = defineAsyncComponent(
  () => import("./components/standup/StandupPanelApp.vue"),
);
const IssueDetailApp = defineAsyncComponent(
  () => import("./components/standup/IssueDetailApp.vue"),
);
const ClipboardManagerWindow = defineAsyncComponent(
  () => import("./components/clipboard-manager/ClipboardManagerWindow.vue"),
);
const JsonViewerApp = defineAsyncComponent(
  () => import("./components/json-viewer/JsonViewerApp.vue"),
);

const { dismiss } = usePalette();

const isMissionControl = window.location.hash.startsWith("#mission-control");
const isSessionDetail = window.location.hash.startsWith("#session-detail");
const isSqlQuery = window.location.hash.startsWith("#sql-query");
const isStandupPanel = window.location.hash.startsWith("#standup-panel");
const isIssueDetail = window.location.hash.startsWith("#issue-detail");
const isClipboardManager = window.location.hash.startsWith("#clipboard-manager");
const isJsonViewer = window.location.hash.startsWith("#json-viewer");

function onBackdropClick(e: MouseEvent) {
  if ((e.target as HTMLElement).classList.contains("backdrop")) {
    dismiss();
  }
}
</script>

<template>
  <SqlQueryApp v-if="isSqlQuery" />
  <SessionDetailApp v-else-if="isSessionDetail" />
  <MissionControlApp v-else-if="isMissionControl" />
  <StandupPanelApp v-else-if="isStandupPanel" />
  <IssueDetailApp v-else-if="isIssueDetail" />
  <ClipboardManagerWindow v-else-if="isClipboardManager" />
  <JsonViewerApp v-else-if="isJsonViewer" />
  <div v-else class="backdrop" @mousedown="onBackdropClick">
    <CommandPalette />
  </div>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.35);
  backdrop-filter: blur(8px);
  display: flex;
  align-items: flex-start;
  justify-content: center;
  padding-top: 15vh;
  animation: fade-in 0.15s ease-out;
}

@keyframes fade-in {
  from { opacity: 0; }
  to { opacity: 1; }
}
</style>
