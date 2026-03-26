<script setup lang="ts">
import type { AssumeIdentityResult } from "../../lib/tauri";

const props = defineProps<{
  result: AssumeIdentityResult;
}>();
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
          <span class="state-value">{{ result.after.acting_as_login }}</span>
          <span class="state-meta">({{ result.after.acting_as_name }})</span>
        </div>
        <div class="state-row">
          <span class="state-label">Password</span>
          <span class="state-value mono">{{ result.after.password }}</span>
        </div>
        <div class="state-row">
          <span class="state-label">Since</span>
          <span class="state-value mono">{{ result.after.changed_at }}</span>
        </div>
      </div>
    </template>

    <!-- Normal before/after -->
    <template v-else>
      <div v-if="result.before" class="state-section">
        <div class="state-header before">Before</div>
        <div class="state-row">
          <span class="state-label">Acting as</span>
          <span class="state-value">{{ result.before.acting_as_login }}</span>
          <span class="state-meta">({{ result.before.acting_as_name }})</span>
        </div>
        <div class="state-row">
          <span class="state-label">Password</span>
          <span class="state-value mono">{{ result.before.password }}</span>
        </div>
        <div class="state-row">
          <span class="state-label">Since</span>
          <span class="state-value mono">{{ result.before.changed_at }}</span>
        </div>
      </div>

      <div v-if="result.after" class="state-section">
        <div class="state-header" :class="result.passwordChanged ? 'after-success' : 'after-warning'">
          After
        </div>
        <div class="state-row">
          <span class="state-label">Acting as</span>
          <span class="state-value">{{ result.after.acting_as_login }}</span>
          <span class="state-meta">({{ result.after.acting_as_name }})</span>
        </div>
        <div class="state-row">
          <span class="state-label">Password</span>
          <span class="state-value mono">{{ result.after.password }}</span>
        </div>
        <div class="state-row">
          <span class="state-label">Since</span>
          <span class="state-value mono">{{ result.after.changed_at }}</span>
        </div>
      </div>

      <div
        v-if="result.message"
        class="result-badge"
        :class="result.passwordChanged ? 'success' : 'warning'"
      >
        {{ result.message }}
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

.state-header.before {
  color: var(--accent-yellow);
}

.state-header.after-success {
  color: var(--accent-green);
}

.state-header.after-warning {
  color: var(--accent-red);
}

.state-header.current {
  color: var(--accent-green);
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
