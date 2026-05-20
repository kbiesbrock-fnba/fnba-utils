<script setup lang="ts">
import { ref } from "vue";
import { useTerminal } from "@/composables/useTerminal";

const props = defineProps<{ sessionId: string; cwd: string }>();
const emit = defineEmits<{ closed: []; error: [msg: string] }>();

const termContainer = ref<HTMLDivElement | null>(null);

const { trustWarning, startupState, interrupt, retryStart } = useTerminal({
  sessionId: props.sessionId,
  cwd: props.cwd,
  container: termContainer,
  onClosed: () => emit("closed"),
  onError: (msg) => emit("error", msg),
});
</script>

<template>
  <div class="chat-pane">
    <div v-if="trustWarning" class="chat-banner chat-banner-warn">
      <span>{{ trustWarning }}</span>
      <button class="chat-banner-close" @click="trustWarning = null">×</button>
    </div>
    <div v-if="startupState === 'stalled'" class="chat-banner chat-banner-info">
      <span>No output yet. Claude may still be starting.</span>
      <button class="chat-banner-action" @click="retryStart">Retry</button>
    </div>
    <div v-if="startupState === 'connecting'" class="chat-banner chat-banner-info chat-banner-quiet">
      <span>Starting Claude…</span>
    </div>

    <div ref="termContainer" class="chat-term" />

    <div class="chat-toolbar">
      <button class="chat-stop" title="Interrupt the current turn (Ctrl-C)" @click="interrupt">
        Stop
      </button>
    </div>
  </div>
</template>

<style scoped>
.chat-pane {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
  background: #0b1116;
}

.chat-term {
  flex: 1;
  min-height: 0;
  padding: 4px 6px;
  overflow: hidden;
}

.chat-term :deep(.xterm) {
  height: 100%;
}

.chat-term :deep(.xterm-viewport) {
  background: transparent !important;
}

.chat-toolbar {
  display: flex;
  justify-content: flex-end;
  gap: 6px;
  padding: 6px 10px;
  border-top: 1px solid var(--border-subtle);
  flex-shrink: 0;
  background: var(--bg-primary, #0b1116);
}

.chat-stop {
  padding: 4px 12px;
  font-size: 11px;
  font-weight: 600;
  border-radius: var(--radius-sm);
  border: 1px solid rgba(248, 113, 113, 0.4);
  background: rgba(248, 113, 113, 0.12);
  color: var(--accent-red, #f87171);
  cursor: pointer;
}

.chat-stop:hover {
  background: rgba(248, 113, 113, 0.2);
}

.chat-banner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 6px 12px;
  font-size: 11px;
  line-height: 1.4;
  flex-shrink: 0;
}

.chat-banner-warn {
  background: rgba(251, 191, 36, 0.1);
  border-bottom: 1px solid rgba(251, 191, 36, 0.3);
  color: var(--accent-yellow, #fbbf24);
}

.chat-banner-info {
  background: rgba(96, 165, 250, 0.08);
  border-bottom: 1px solid rgba(96, 165, 250, 0.3);
  color: var(--accent-blue, #60a5fa);
}

.chat-banner-quiet {
  background: transparent;
  border-bottom: 1px solid var(--border-subtle);
  color: var(--text-secondary);
}

.chat-banner-close,
.chat-banner-action {
  flex-shrink: 0;
  padding: 2px 8px;
  border: 1px solid currentColor;
  border-radius: var(--radius-sm);
  background: transparent;
  color: inherit;
  font-size: 10px;
  cursor: pointer;
}

.chat-banner-close {
  padding: 0 6px;
  font-size: 14px;
  line-height: 1;
  border: 0;
}
</style>
