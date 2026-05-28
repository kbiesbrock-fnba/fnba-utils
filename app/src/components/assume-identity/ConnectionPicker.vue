<script setup lang="ts">
import { ref, computed } from "vue";
import { useListNavigation } from "@/composables/useListNavigation";
import type { IdentityConnection } from "@/lib/tauri";
import CommandInput from "../CommandInput.vue";
import LabelPrompt from "./LabelPrompt.vue";

const props = defineProps<{
  connections: IdentityConnection[];
}>();

const emit = defineEmits<{
  select: [connection: IdentityConnection];
  deleteCustom: [server: string];
}>();

const query = ref("");
const listRef = ref<HTMLElement | null>(null);
const labelMode = ref<{ server: string } | null>(null);

// Stable per-environment quick-select digit. Shared remotes get fixed numbers
// (caster=1, meleagris=2, dsqlaleroy=3); any other/local connection gets 4+ in
// list order. Displayed in descending-digit order (locals on top, caster last).
function envDigit(server: string): number | null {
  const s = server.toLowerCase();
  if (s.includes("caster")) return 1;
  if (s.includes("meleagris")) return 2;
  if (s.includes("dsqlaleroy")) return 3;
  return null;
}

interface DisplayConn {
  conn: IdentityConnection;
  digit: number;
}

const displayConns = computed<DisplayConn[]>(() => {
  const q = query.value.toLowerCase();
  const matching = q
    ? props.connections.filter(
        (c) => c.label.toLowerCase().includes(q) || c.server.toLowerCase().includes(q),
      )
    : props.connections;

  let other = 4;
  const withDigit = matching.map((conn) => ({
    conn,
    digit: envDigit(conn.server) ?? other++,
  }));
  withDigit.sort((a, b) => b.digit - a.digit);
  return withDigit;
});

// digit -> connection, for keyboard quick-select (only when not typing).
const digitMap = computed(() => {
  const m = new Map<number, IdentityConnection>();
  for (const d of displayConns.value) {
    if (d.digit <= 9) m.set(d.digit, d.conn);
  }
  return m;
});

const { selectedIndex, resetIndex } = useListNavigation({
  itemCount: () => (labelMode.value ? 0 : displayConns.value.length),
  onSelect: (i) => {
    const row = displayConns.value[i];
    if (row) emit("select", row.conn);
  },
  onEnterEmpty: () => {
    if (query.value.trim()) {
      labelMode.value = { server: query.value.trim() };
    }
  },
  extraKeys: [
    {
      key: "Delete",
      handler: () => {
        if (labelMode.value) return false;
        const row = displayConns.value[selectedIndex.value];
        if (row?.conn.isCustom) {
          emit("deleteCustom", row.conn.server);
          return;
        }
        return false;
      },
    },
    // Digit quick-select by environment number — only when the search box is
    // empty, so a numeric server name can still be typed when adding a custom.
    ...["1", "2", "3", "4", "5", "6", "7", "8", "9"].map((d) => ({
      key: d,
      preventDefault: false,
      handler: (e: KeyboardEvent) => {
        if (labelMode.value || query.value.trim()) return false;
        e.preventDefault();
        const conn = digitMap.value.get(parseInt(d, 10));
        if (conn) emit("select", conn);
      },
    })),
  ],
  listRef,
  scrollStrategy: "data-index",
});

function onUpdate(value: string) {
  query.value = value;
  resetIndex();
}

function onLabelConfirm(label: string) {
  if (!labelMode.value) return;
  emit("select", { label, server: labelMode.value.server });
  labelMode.value = null;
}

function onLabelCancel() {
  labelMode.value = null;
}
</script>

<template>
  <LabelPrompt
    v-if="labelMode"
    :value="labelMode.server"
    default-label="Local"
    @confirm="onLabelConfirm"
    @cancel="onLabelCancel"
  />
  <template v-else>
    <CommandInput
      key="query"
      :value="query"
      placeholder="Select connection (press its number)…"
      @update="onUpdate"
    />
    <div class="picker-divider" />
    <div ref="listRef" class="picker-list">
      <div v-if="displayConns.length === 0 && query.trim()" class="empty use-custom">
        Press Enter to use <strong>{{ query.trim() }}</strong>
      </div>
      <div v-else-if="displayConns.length === 0" class="empty">No matching connections</div>
      <div
        v-for="(row, i) in displayConns"
        :key="row.conn.server"
        class="picker-item"
        :class="{ selected: i === selectedIndex }"
        :data-index="i"
        @click="emit('select', row.conn)"
        @mouseenter="selectedIndex = i"
      >
        <span v-if="row.digit <= 9" class="kbd">{{ row.digit }}</span>
        <span class="picker-name">{{ row.conn.server }}</span>
        <span class="picker-labels">{{ row.conn.label }}</span>
        <span v-if="row.conn.isCustom" class="custom-badge">custom</span>
        <button
          v-if="row.conn.isCustom"
          class="remove-btn"
          title="Delete custom entry (Del)"
          @click.stop="emit('deleteCustom', row.conn.server)"
        >
          <svg viewBox="0 0 16 16" fill="currentColor" width="12" height="12">
            <path d="M3.72 3.72a.75.75 0 0 1 1.06 0L8 6.94l3.22-3.22a.75.75 0 1 1 1.06 1.06L9.06 8l3.22 3.22a.75.75 0 1 1-1.06 1.06L8 9.06l-3.22 3.22a.75.75 0 0 1-1.06-1.06L6.94 8 3.72 4.78a.75.75 0 0 1 0-1.06Z" />
          </svg>
        </button>
      </div>
    </div>
  </template>
</template>

<style src="./picker-shared.css" scoped></style>
<style scoped>
.kbd {
  flex-shrink: 0;
  min-width: 16px;
  height: 16px;
  padding: 0 3px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--border-subtle);
  border-radius: 3px;
  font-size: 10px;
  font-family: var(--font-mono);
  color: var(--text-secondary);
}

.picker-labels {
  margin-left: auto;
}
</style>
