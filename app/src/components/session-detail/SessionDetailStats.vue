<script setup lang="ts">
import { computed } from "vue";
import type { SessionStats } from "@/lib/tauri";
import { formatTokens } from "@/lib/format";
import { estimateCost, formatUSD, DEFAULT_PRICING } from "@/lib/pricing";

const props = defineProps<{ stats: SessionStats; subagentCount: number }>();

const inputTokens = computed(() => formatTokens(props.stats.totalInputTokens));
const outputTokens = computed(() => formatTokens(props.stats.totalOutputTokens));
// Cost estimate uses default (Sonnet) rates since we don't capture the model
// into state today. Pricing table at lib/pricing.ts — update by hand when
// rates change. Tooltip explains the estimate basis.
const costEstimate = computed(() =>
  formatUSD(estimateCost(props.stats.totalInputTokens, props.stats.totalOutputTokens)),
);
const costTooltip = computed(
  () =>
    `Est. cost at Sonnet rates ($${DEFAULT_PRICING.inputPerMTok}/MT in, $${DEFAULT_PRICING.outputPerMTok}/MT out). Adjust pricing in lib/pricing.ts.`,
);
</script>

<template>
  <div class="sd-stats">
    <div class="stat">
      <span class="stat-value">{{ stats.messageCount }}</span>
      <span class="stat-label">messages</span>
    </div>
    <div class="stat-sep" />
    <div class="stat">
      <span class="stat-value">{{ inputTokens }}</span>
      <span class="stat-label">tokens in</span>
    </div>
    <div class="stat-sep" />
    <div class="stat">
      <span class="stat-value">{{ outputTokens }}</span>
      <span class="stat-label">tokens out</span>
    </div>
    <div class="stat-sep" />
    <div class="stat" :title="costTooltip">
      <span class="stat-value cost">{{ costEstimate }}</span>
      <span class="stat-label">est. cost</span>
    </div>
    <div v-if="subagentCount > 0" class="stat-sep" />
    <div v-if="subagentCount > 0" class="stat">
      <span class="stat-value agents">{{ subagentCount }}</span>
      <span class="stat-label">agent{{ subagentCount !== 1 ? "s" : "" }}</span>
    </div>
  </div>
</template>

<style scoped>
.sd-stats {
  display: flex;
  align-items: center;
  padding: 10px 14px;
  gap: 0;
}

.stat {
  display: flex;
  flex-direction: column;
  align-items: center;
  flex: 1;
  gap: 2px;
}

.stat-value {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
  font-family: var(--font-mono);
}

.stat-value.agents {
  color: var(--accent-green);
}

.stat-value.cost {
  color: var(--accent-blue);
}

.stat-label {
  font-size: 10px;
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.3px;
}

.stat-sep {
  width: 1px;
  height: 28px;
  background: var(--border-subtle);
  flex-shrink: 0;
}
</style>
