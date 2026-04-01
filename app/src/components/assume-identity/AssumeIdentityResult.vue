<script setup lang="ts">
import { ref } from "vue";
import type { AssumeIdentityResult } from "../../lib/tauri";

const props = defineProps<{
  result: AssumeIdentityResult;
}>();

const copiedPassword = ref(false);
function copyPassword(password: string) {
  navigator.clipboard.writeText(password).then(() => {
    copiedPassword.value = true;
    setTimeout(() => (copiedPassword.value = false), 2000);
  });
}
</script>

<template>
  <div class="result-view">
    <!-- Header -->
    <div class="result-header">
      <span class="server-label">Server</span>
      <span class="server-value">{{ result.server }}</span>
    </div>
    <div class="result-meta">
      <span class="meta-label">Login</span>
      <span class="meta-value">{{ result.login }}</span>
    </div>

    <!-- Already assuming -->
    <template v-if="result.alreadyAssuming && result.after">
      <div class="result-badge warning">Already acting as this identity</div>
      <div class="state-section">
        <div class="state-header current">Current</div>
        <div class="state-row">
          <span class="state-label">Acting as</span>
          <span class="state-value">{{ result.after.actingAsLogin }}</span>
          <span class="state-meta">({{ result.after.actingAsName }})</span>
        </div>
        <div class="state-row">
          <span class="state-label">Password</span>
          <span class="state-value mono">{{ result.after.password }}</span>
          <button class="copy-btn" @click="copyPassword(result.after.password)">
            {{ copiedPassword ? 'Copied' : 'Copy' }}
          </button>
        </div>
        <div class="state-row">
          <span class="state-label">Since</span>
          <span class="state-value mono">{{ result.after.changedAt }}</span>
        </div>
      </div>
    </template>

    <!-- Normal switch -->
    <template v-else>
      <div v-if="result.after" class="state-section primary">
        <div class="state-header" :class="result.passwordChanged ? 'now-success' : 'now-warning'">
          Success
        </div>
        <div class="state-row">
          <span class="state-label">Acting as</span>
          <span class="state-value">{{ result.after.actingAsLogin }}</span>
          <span class="state-meta">({{ result.after.actingAsName }})</span>
        </div>
        <div class="state-row">
          <span class="state-label">Password</span>
          <span class="state-value mono">{{ result.after.password }}</span>
          <button class="copy-btn" @click="copyPassword(result.after.password)">
            {{ copiedPassword ? 'Copied' : 'Copy' }}
          </button>
        </div>
        <div class="state-row">
          <span class="state-label">Since</span>
          <span class="state-value mono">{{ result.after.changedAt }}</span>
        </div>
      </div>

      <div
        v-if="result.message"
        class="result-badge"
        :class="result.passwordChanged ? 'success' : 'warning'"
      >
        {{ result.message }}
      </div>

      <div v-if="result.before" class="state-section secondary">
        <div class="state-header previously">Previously</div>
        <div class="state-row">
          <span class="state-label">Was</span>
          <span class="state-value">{{ result.before.actingAsLogin }}</span>
          <span class="state-meta">({{ result.before.actingAsName }})</span>
        </div>
        <div class="state-row">
          <span class="state-label">Password</span>
          <span class="state-value mono">{{ result.before.password }}</span>
        </div>
        <div class="state-row">
          <span class="state-label">Since</span>
          <span class="state-value mono">{{ result.before.changedAt }}</span>
        </div>
      </div>
    </template>
  </div>
</template>

<style scoped>
.result-view {
  padding: 16px 20px;
  overflow-y: auto;
  max-height: 380px;
}

.result-header {
  display: flex;
  align-items: baseline;
  gap: 8px;
  margin-bottom: 2px;
}

.server-label,
.meta-label {
  font-size: 12px;
  color: var(--text-secondary);
  width: 50px;
}

.server-value,
.meta-value {
  font-size: 13px;
  font-family: var(--font-mono);
  color: var(--text-secondary);
}

.result-meta {
  display: flex;
  align-items: baseline;
  gap: 8px;
  margin-bottom: 16px;
}

.state-section {
  margin-bottom: 12px;
}

.state-header {
  font-size: 12px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  margin-bottom: 6px;
  padding-bottom: 4px;
  border-bottom: 1px solid var(--border-subtle);
}

.state-header.now-success {
  color: var(--accent-green);
}

.state-header.now-warning {
  color: var(--accent-red);
}

.state-header.current {
  color: var(--accent-green);
}

.state-header.previously {
  color: var(--text-secondary);
  font-weight: 500;
}

.state-section.secondary {
  opacity: 0.55;
  margin-top: 4px;
}

.state-section.secondary .state-value {
  font-size: 13px;
}

.state-row {
  display: flex;
  align-items: baseline;
  gap: 8px;
  padding: 3px 0;
}

.state-label {
  font-size: 12px;
  color: var(--text-secondary);
  width: 70px;
  flex-shrink: 0;
}

.state-value {
  font-size: 14px;
  color: var(--text-primary);
}

.state-value.mono {
  font-family: var(--font-mono);
}

.state-meta {
  font-size: 12px;
  color: var(--text-secondary);
}

.copy-btn {
  background: transparent;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  font-size: 11px;
  padding: 1px 8px;
  cursor: pointer;
  flex-shrink: 0;
  transition: color 0.15s, border-color 0.15s;
}

.copy-btn:hover {
  color: var(--text-primary);
  border-color: var(--text-secondary);
}

.result-badge {
  margin-top: 12px;
  padding: 8px 12px;
  border-radius: var(--radius-sm);
  font-size: 13px;
  font-weight: 500;
}

.result-badge.success {
  background: rgba(52, 211, 153, 0.1);
  color: var(--accent-green);
}

.result-badge.warning {
  background: rgba(251, 191, 36, 0.1);
  color: var(--accent-yellow);
}
</style>
