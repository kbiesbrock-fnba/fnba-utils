<script setup lang="ts">
import { ref } from "vue";

const props = defineProps<{ status: string; sending: boolean }>();
const emit = defineEmits<{ send: [text: string] }>();

const text = ref("");
const canSend = () => text.value.trim() && props.status === "idle" && !props.sending;

function handleSend() {
  if (!canSend()) return;
  emit("send", text.value.trim());
  text.value = "";
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    handleSend();
  }
}
</script>

<template>
  <div class="sd-prompt">
    <div class="sd-prompt-row">
      <textarea
        v-model="text"
        class="sd-prompt-input"
        :placeholder="status === 'idle' ? 'Send a prompt...' : 'Session is busy...'"
        :disabled="status !== 'idle' || sending"
        rows="2"
        @keydown="handleKeydown"
      />
      <button
        class="sd-prompt-send"
        :disabled="!canSend()"
        :title="sending ? 'Sending...' : 'Send prompt (Enter)'"
        @click="handleSend"
      >
        <svg v-if="!sending" viewBox="0 0 16 16" fill="currentColor" width="14" height="14">
          <path d="M.989 8 .064 2.68a1.342 1.342 0 0 1 1.85-1.462l13.402 5.744a1.13 1.13 0 0 1 0 2.076L1.913 14.782a1.343 1.343 0 0 1-1.85-1.463L.99 8Zm.603-4.867L2.11 7.25h5.378a.75.75 0 0 1 0 1.5H2.11l-.519 4.117L13.929 8 1.592 3.133Z" />
        </svg>
        <svg v-else class="spin" viewBox="0 0 16 16" fill="currentColor" width="14" height="14">
          <path d="M8 0a8 8 0 1 0 8 8h-1.5A6.5 6.5 0 1 1 8 1.5V0Z" />
        </svg>
      </button>
    </div>
  </div>
</template>

<style scoped>
.sd-prompt {
  padding: 8px 14px;
}

.sd-prompt-row {
  display: flex;
  gap: 6px;
  align-items: flex-end;
}

.sd-prompt-input {
  flex: 1;
  background: var(--bg-secondary);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  font-size: 12px;
  font-family: inherit;
  padding: 6px 8px;
  resize: none;
  outline: none;
  line-height: 1.4;
  transition: border-color 0.15s ease;
}

.sd-prompt-input::placeholder {
  color: var(--text-placeholder);
}

.sd-prompt-input:focus {
  border-color: var(--accent-blue);
}

.sd-prompt-input:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.sd-prompt-send {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  flex-shrink: 0;
  border: 1px solid var(--border-subtle);
  background: transparent;
  color: var(--accent-blue);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: all 0.1s ease;
}

.sd-prompt-send:hover:not(:disabled) {
  background: rgba(96, 165, 250, 0.12);
  border-color: rgba(96, 165, 250, 0.3);
}

.sd-prompt-send:disabled {
  opacity: 0.3;
  cursor: not-allowed;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.spin {
  animation: spin 0.8s linear infinite;
}
</style>
