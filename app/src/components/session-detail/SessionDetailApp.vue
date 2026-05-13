<script setup lang="ts">
import { useSessionDetail } from "@/composables/useSessionDetail";
import SessionDetailHeader from "./SessionDetailHeader.vue";
import SessionDetailStats from "./SessionDetailStats.vue";
import SessionDetailActivity from "./SessionDetailActivity.vue";
import SessionDetailActions from "./SessionDetailActions.vue";
import ChatPane from "./ChatPane.vue";

const { detail, loading, error, pinned, chatActive, togglePin, openChat, closeChat, onChatClosed, onChatError } = useSessionDetail();
</script>

<template>
  <div class="sd-app">
    <div v-if="!detail && !loading" class="sd-empty">
      Select a session in Mission Control
    </div>
    <div v-else-if="loading && !detail" class="sd-empty">Loading...</div>
    <div v-else-if="error && !detail" class="sd-empty sd-error">{{ error }}</div>
    <template v-else-if="detail">
      <!-- Chat mode: compact header + chat pane -->
      <template v-if="chatActive">
        <div class="sd-chat-header" data-tauri-drag-region>
          <span class="sd-chat-name">{{ detail.name ?? detail.sessionId }}</span>
          <button class="sd-release-btn" @click="closeChat">Close</button>
        </div>
        <ChatPane
          :session-id="detail.sessionId"
          :cwd="detail.cwd"
          @closed="onChatClosed"
          @error="onChatError"
        />
      </template>

      <!-- Info mode: existing layout + Open Chat button -->
      <template v-else>
        <SessionDetailHeader :detail="detail" :pinned="pinned" @toggle-pin="togglePin" />
        <div class="sd-divider" />
        <SessionDetailStats :stats="detail.stats" :subagent-count="detail.subagents.length" />
        <div class="sd-divider" />
        <SessionDetailActivity :messages="detail.recentMessages" />
        <div v-if="error" class="sd-inline-error">{{ error }}</div>
        <div class="sd-divider" />
        <div class="sd-pickup-section" v-if="detail.isAlive">
          <button class="sd-pickup-btn" @click="openChat">Open Chat</button>
        </div>
        <div class="sd-divider" />
        <SessionDetailActions />
      </template>
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

/* Chat mode header */
.sd-chat-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 14px;
  flex-shrink: 0;
  -webkit-app-region: drag;
}

.sd-chat-name {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
}

.sd-release-btn {
  flex-shrink: 0;
  font-size: 11px;
  padding: 4px 12px;
  border-radius: var(--radius-sm);
  border: 1px solid rgba(96, 165, 250, 0.3);
  background: rgba(96, 165, 250, 0.1);
  color: var(--accent-blue);
  cursor: pointer;
  font-weight: 500;
  transition: all 0.1s ease;
  -webkit-app-region: no-drag;
}

.sd-release-btn:hover {
  background: rgba(96, 165, 250, 0.2);
  border-color: rgba(96, 165, 250, 0.5);
}

/* Pick Up button */
.sd-pickup-section {
  padding: 10px 14px;
}

.sd-pickup-btn {
  width: 100%;
  padding: 8px 16px;
  font-size: 12px;
  font-weight: 600;
  border-radius: var(--radius-sm);
  border: 1px solid rgba(52, 211, 153, 0.3);
  background: rgba(52, 211, 153, 0.1);
  color: var(--accent-green);
  cursor: pointer;
  transition: all 0.15s ease;
}

.sd-pickup-btn:hover {
  background: rgba(52, 211, 153, 0.2);
  border-color: rgba(52, 211, 153, 0.5);
}
</style>
