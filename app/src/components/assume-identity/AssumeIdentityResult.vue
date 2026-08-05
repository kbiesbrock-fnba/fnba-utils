<script setup lang="ts">
import { ref } from "vue";
import type { AssumeIdentityResult } from "@/lib/tauri";

const props = withDefaults(
  defineProps<{
    result: AssumeIdentityResult;
    /** Hide the Server/Login header — used in the multi-connection result
     *  list, where that same info is already shown in the connection's own
     *  head row, so repeating it per-card just costs vertical space. */
    compact?: boolean;
  }>(),
  { compact: false },
);

const copiedPassword = ref(false);
function copyPassword(password: string) {
  navigator.clipboard.writeText(password).then(() => {
    copiedPassword.value = true;
    setTimeout(() => (copiedPassword.value = false), 2000);
  });
}
</script>

<template>
  <div class="result-view" :class="{ compact }">
    <!-- Header (skipped in compact mode: the multi-connection list already
         shows server/label in its own head row above this component). -->
    <template v-if="!compact">
      <div class="result-header">
        <span class="server-label">Server</span>
        <span class="server-value">{{ result.server }}</span>
      </div>
      <div class="result-meta">
        <span class="meta-label">Login</span>
        <span class="meta-value">{{ result.login }}</span>
      </div>
    </template>

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
      <div
        v-if="result.message"
        class="result-badge caption"
        :class="result.passwordChanged ? 'success' : 'warning'"
      >
        {{ result.message }}
      </div>

      <!-- A real grid (not two independent flex columns) so "Acting as" /
           "Password" / "Since" line up row-for-row between Success and
           Previously — each element below carries .primary or .secondary to
           place it in column 1 or 2; auto-placement handles the rows. -->
      <div class="state-grid">
        <div
          v-if="result.after"
          class="state-header primary"
          :class="result.passwordChanged ? 'now-success' : 'now-warning'"
        >
          Success
        </div>
        <div v-if="result.before" class="state-header secondary previously">Previously</div>

        <div v-if="result.after" class="state-row primary">
          <span class="state-label">Acting as</span>
          <span class="state-value">{{ result.after.actingAsLogin }}</span>
          <span class="state-meta">({{ result.after.actingAsName }})</span>
        </div>
        <div v-if="result.before" class="state-row secondary">
          <span class="state-label">Was</span>
          <span class="state-value">{{ result.before.actingAsLogin }}</span>
          <span class="state-meta">({{ result.before.actingAsName }})</span>
        </div>

        <div v-if="result.after" class="state-row primary">
          <span class="state-label">Password</span>
          <span class="state-value mono">{{ result.after.password }}</span>
          <button class="copy-btn" @click="copyPassword(result.after.password)">
            {{ copiedPassword ? 'Copied' : 'Copy' }}
          </button>
        </div>
        <div v-if="result.before" class="state-row secondary">
          <span class="state-label">Password</span>
          <span class="state-value mono">{{ result.before.password }}</span>
        </div>

        <div v-if="result.after" class="state-row primary">
          <span class="state-label">Since</span>
          <span class="state-value mono">{{ result.after.changedAt }}</span>
        </div>
        <div v-if="result.before" class="state-row secondary">
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
}

/* Compact (multi-connection list): no internal scroll region of its own —
   the list wrapping this component (.multi-result) is the single scroll
   owner, and less padding since this card is one of several stacked. */
.result-view.compact {
  padding: 10px 20px 12px;
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

/* Side-by-side "Success" / "Previously" grid for the normal-switch case.
   A genuine CSS grid (not two independent flex columns) — .primary/.secondary
   children are placed into column 1/2 and auto-placement lines them up row
   by row, so "Acting as"/"Was", "Password"/"Password", "Since"/"Since" align
   horizontally even though each side renders only when its data exists. */
.state-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  column-gap: 24px;
  row-gap: 2px;
  margin-bottom: 12px;
}

.state-header.primary,
.state-row.primary {
  grid-column: 1;
}

.state-header.secondary,
.state-row.secondary {
  grid-column: 2;
  opacity: 0.55;
}

.state-row.secondary .state-value {
  font-size: 13px;
}

.state-row {
  display: flex;
  align-items: baseline;
  flex-wrap: nowrap;
  gap: 8px;
  padding: 2px 0;
  min-width: 0;
}

.state-label {
  font-size: 12px;
  color: var(--text-secondary);
  width: 62px;
  flex-shrink: 0;
}

.state-value {
  font-size: 14px;
  color: var(--text-primary);
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.state-value.mono {
  font-family: var(--font-mono);
}

.state-meta {
  font-size: 12px;
  color: var(--text-secondary);
  flex: 0 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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
  /* Never the thing that gives up room — .state-value truncates first so the
     Copy button always stays on the password's row instead of wrapping. */
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

/* Caption variant: sits above the side-by-side row instead of between
   stacked sections, so it reads as a one-line summary at a glance. */
.result-badge.caption {
  margin-top: 0;
  margin-bottom: 10px;
  padding: 4px 10px;
  font-size: 12px;
  display: inline-block;
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
