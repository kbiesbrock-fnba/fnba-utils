<script setup lang="ts">
import { computed } from "vue";
import type { ConnectionStatus } from "@/lib/tauri";

const props = defineProps<{ status: ConnectionStatus }>();

const dotClass = computed(() => {
  if (props.status.error) return "dot-error";
  if (props.status.isSelf) return "dot-self";
  return "dot-assuming";
});

const shortServer = computed(() => {
  // Drop the domain — "dsqlaleroy.fnba-dev.network" → "dsqlaleroy"
  return props.status.server.split(".")[0];
});

const displayIdentity = computed(() => {
  if (props.status.error) return "error";
  if (props.status.isSelf) return "self";
  return props.status.actingAsName || props.status.actingAsLogin || "unknown";
});

const tooltip = computed(() => {
  if (props.status.error) return `${props.status.server}\n${props.status.error}`;
  return `${props.status.server} — ${displayIdentity.value}`;
});
</script>

<template>
  <div class="conn-row" :class="{ clickable: !status.error }" :title="tooltip">
    <span class="conn-dot" :class="dotClass" />
    <span class="conn-server">{{ shortServer }}</span>
    <span class="conn-badge">{{ status.label }}</span>
    <span class="conn-identity" :class="{ 'conn-error': !!status.error }">{{ displayIdentity }}</span>
  </div>
</template>

<style scoped>
.conn-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 14px;
  font-size: 12px;
  transition: background 0.1s ease;
}

.conn-row.clickable {
  cursor: pointer;
}

.conn-row:hover {
  background: var(--bg-hover);
}

.conn-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
}

.dot-self {
  background: var(--accent-green);
  box-shadow: 0 0 4px rgba(74, 222, 128, 0.4);
}

.dot-assuming {
  background: var(--accent-yellow);
  box-shadow: 0 0 4px rgba(250, 204, 21, 0.4);
}

.dot-error {
  background: var(--accent-red);
  box-shadow: 0 0 4px rgba(248, 113, 113, 0.4);
}

.conn-server {
  font-weight: 500;
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  min-width: 0;
}

.conn-badge {
  font-size: 10px;
  padding: 0 5px;
  border-radius: 3px;
  font-weight: 500;
  background: rgba(255, 255, 255, 0.06);
  color: var(--text-secondary);
  white-space: nowrap;
  flex-shrink: 0;
  line-height: 16px;
}

.conn-identity {
  color: var(--text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
  margin-left: auto;
}

.conn-error {
  color: var(--accent-red);
  opacity: 0.8;
  font-size: 11px;
}
</style>
