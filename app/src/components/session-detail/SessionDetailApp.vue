<script setup lang="ts">
import { useSessionDetail } from "@/composables/useSessionDetail";
import SessionDetailHeader from "./SessionDetailHeader.vue";
import SessionDetailStats from "./SessionDetailStats.vue";
import SessionDetailActivity from "./SessionDetailActivity.vue";
import SessionDetailActions from "./SessionDetailActions.vue";

const { detail, loading, error, pinned, togglePin } = useSessionDetail();
</script>

<template>
  <div class="sd-app">
    <div v-if="!detail && !loading" class="sd-empty">
      Select a session in Mission Control
    </div>
    <div v-else-if="loading && !detail" class="sd-empty">Loading...</div>
    <div v-else-if="error" class="sd-empty sd-error">{{ error }}</div>
    <template v-else-if="detail">
      <SessionDetailHeader :detail="detail" :pinned="pinned" @toggle-pin="togglePin" />
      <div class="sd-divider" />
      <SessionDetailStats :stats="detail.stats" :subagent-count="detail.subagents.length" />
      <div class="sd-divider" />
      <SessionDetailActivity :messages="detail.recentMessages" />
      <div class="sd-divider" />
      <SessionDetailActions />
    </template>
  </div>
</template>

<style scoped>
.sd-app {
  width: 100%;
  height: 100vh;
  background: var(--bg-primary);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: var(--radius-lg);
  box-shadow:
    0 0 0 1px rgba(255, 255, 255, 0.06),
    0 0 20px rgba(96, 165, 250, 0.08),
    0 25px 50px -12px rgba(0, 0, 0, 0.6);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.sd-divider {
  height: 1px;
  background: var(--border-subtle);
  flex-shrink: 0;
}

.sd-empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  color: var(--text-secondary);
  padding: 32px 16px;
  text-align: center;
}

.sd-error {
  color: var(--accent-red);
}
</style>
