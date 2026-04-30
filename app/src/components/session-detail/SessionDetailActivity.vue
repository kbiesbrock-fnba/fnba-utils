<script setup lang="ts">
import { computed } from "vue";
import type { ConversationMessage } from "@/lib/tauri";

const props = defineProps<{ messages: ConversationMessage[] }>();

function formatTime(iso: string): string {
  if (!iso) return "";
  try {
    const d = new Date(iso);
    return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  } catch {
    return "";
  }
}

const displayMessages = computed(() =>
  props.messages.map((m) => ({
    ...m,
    time: formatTime(m.timestamp),
    isUser: m.role === "user",
    isTool: !!m.toolName,
  })),
);
</script>

<template>
  <div class="sd-activity">
    <div v-if="messages.length === 0" class="sd-activity-empty">No messages yet</div>
    <div v-for="(msg, i) in displayMessages" :key="i" class="msg">
      <div class="msg-bar" :class="{ user: msg.isUser, assistant: !msg.isUser }" />
      <div class="msg-body">
        <div class="msg-meta">
          <span class="msg-role">{{ msg.isUser ? "You" : "Claude" }}</span>
          <span v-if="msg.isTool" class="msg-tool">{{ msg.toolName }}</span>
          <span class="msg-time">{{ msg.time }}</span>
        </div>
        <div class="msg-text">{{ msg.summary }}</div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.sd-activity {
  flex: 1;
  overflow-y: auto;
  overflow-x: hidden;
  min-height: 0;
  min-width: 0;
}

.sd-activity-empty {
  padding: 24px 16px;
  text-align: center;
  font-size: 11px;
  color: var(--text-secondary);
}

.msg {
  display: flex;
  gap: 8px;
  padding: 8px 14px;
  overflow: hidden;
  transition: background 0.1s ease;
}

.msg:hover {
  background: var(--bg-hover);
}

.msg-bar {
  width: 3px;
  border-radius: 2px;
  flex-shrink: 0;
  min-height: 20px;
}

.msg-bar.user {
  background: var(--accent-blue);
}

.msg-bar.assistant {
  background: var(--accent-green);
}

.msg-body {
  min-width: 0;
  flex: 1;
}

.msg-meta {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 2px;
}

.msg-role {
  font-size: 11px;
  font-weight: 600;
  color: var(--text-primary);
}

.msg-tool {
  font-size: 10px;
  padding: 0 5px;
  border-radius: 3px;
  background: rgba(52, 211, 153, 0.15);
  color: var(--accent-green);
  font-family: var(--font-mono);
}

.msg-time {
  font-size: 10px;
  color: var(--text-placeholder);
  margin-left: auto;
}

.msg-text {
  font-size: 12px;
  color: var(--text-secondary);
  line-height: 1.4;
  overflow: hidden;
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  word-break: break-word;
}
</style>
