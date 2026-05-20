<script setup lang="ts">
import { useSessionDetail } from "@/composables/useSessionDetail";
import SessionDetailHeader from "./SessionDetailHeader.vue";
import SessionDetailStats from "./SessionDetailStats.vue";
import SessionDetailActions from "./SessionDetailActions.vue";
import ChatPane from "./ChatPane.vue";

const { detail, loading, error, pinned, togglePin, onChatClosed, onChatError } = useSessionDetail();
</script>

<template>
  <div class="sd-app">
    <div v-if="!detail && !loading" class="sd-empty">
      Select a session in Mission Control
    </div>
    <div v-else-if="loading && !detail" class="sd-empty">Loading...</div>
    <div v-else-if="error && !detail" class="sd-empty sd-error">{{ error }}</div>
    <template v-else-if="detail">
      <SessionDetailHeader :detail="detail" :pinned="pinned" @toggle-pin="togglePin" />
      <div class="sd-divider" />
      <SessionDetailStats :stats="detail.stats" :subagent-count="detail.subagents.length" />
      <div class="sd-divider" />
      <ChatPane
        v-if="detail.isAlive"
        :session-id="detail.sessionId"
        :cwd="detail.cwd"
        @closed="onChatClosed"
        @error="onChatError"
      />
      <div v-else class="sd-dead">Session has ended.</div>
      <div v-if="error" class="sd-inline-error">{{ error }}</div>
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

.sd-inline-error {
  padding: 6px 14px;
  font-size: 11px;
  color: var(--accent-red);
  background: rgba(248, 113, 113, 0.08);
}

.sd-dead {
  padding: 20px 16px;
  text-align: center;
  font-size: 12px;
  color: var(--text-secondary);
}
</style>
