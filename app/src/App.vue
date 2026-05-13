<script setup lang="ts">
import CommandPalette from "./components/CommandPalette.vue";
import MissionControlApp from "./components/mission-control/MissionControlApp.vue";
import SessionDetailApp from "./components/session-detail/SessionDetailApp.vue";
import SqlQueryApp from "./components/sql-query/SqlQueryApp.vue";
import { usePalette } from "./composables/usePalette";

const { dismiss } = usePalette();

const isMissionControl = window.location.hash.startsWith("#mission-control");
const isSessionDetail = window.location.hash.startsWith("#session-detail");
const isSqlQuery = window.location.hash.startsWith("#sql-query");

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
