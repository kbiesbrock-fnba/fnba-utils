<script setup lang="ts">
import { computed } from "vue";

const props = defineProps<{
  path: string[];
  value: unknown;
  expanded: boolean;
  isSelected: boolean;
  search: string;
  sortKeys: boolean;
}>();

const emit = defineEmits<{
  toggle: [path: string[]];
  select: [path: string[]];
}>();

const isContainer = computed(() => {
  return Array.isArray(props.value) || (typeof props.value === "object" && props.value !== null);
});

const isArray = computed(() => Array.isArray(props.value));

const summary = computed(() => {
  if (Array.isArray(props.value)) {
    return `[${props.value.length}]`;
  }
  if (typeof props.value === "object" && props.value !== null) {
    const keys = Object.keys(props.value);
    return `{${keys.length} keys}`;
  }
  if (props.value === null) return "null";
  if (typeof props.value === "string") return `"${props.value}"`;
  return String(props.value);
});

const childEntries = computed(() => {
  if (!props.value) return [];
  if (Array.isArray(props.value)) {
    return props.value.map((v, i) => ({ key: String(i), value: v }));
  }
  if (typeof props.value === "object" && props.value !== null) {
    const obj = props.value as Record<string, unknown>;
    let keys = Object.keys(obj);
    if (props.sortKeys) {
      keys = keys.sort();
    }
    return keys.map((k) => ({ key: k, value: obj[k] }));
  }
  return [];
});

function checkNodeMatches(): boolean {
  if (!props.search) return true;
  const query = props.search.toLowerCase();
  // Check path
  const pathStr = props.path.join(".").toLowerCase();
  if (pathStr.includes(query)) return true;
  // Check value
  const valueStr = JSON.stringify(props.value).toLowerCase();
  return valueStr.includes(query);
}

function checkAnyChildMatches(): boolean {
  if (!props.search) return true;
  // Recursively check if any child matches
  return childEntries.value.some((e) => {
    const childNodeMatches = checkChildMatches(e.value);
    return childNodeMatches;
  });
}

function checkChildMatches(value: unknown): boolean {
  if (!props.search) return true;
  const query = props.search.toLowerCase();
  // Check value
  const valueStr = JSON.stringify(value).toLowerCase();
  if (valueStr.includes(query)) return true;
  // If container, check children
  if (Array.isArray(value)) {
    return value.some((v) => checkChildMatches(v));
  }
  if (typeof value === "object" && value !== null) {
    return Object.values(value as Record<string, unknown>).some((v) => checkChildMatches(v));
  }
  return false;
}

const nodeMatches = computed(() => checkNodeMatches());
const anyChildMatches = computed(() => checkAnyChildMatches());
const shouldShow = computed(() => nodeMatches.value || anyChildMatches.value);
const opacity = computed(() => (shouldShow.value ? 1 : 0.3));
</script>

<template>
  <div class="tree-node" :style="{ opacity }">
    <div
      class="node-row"
      :class="{ selected: isSelected }"
      @click="emit('select', path)"
    >
      <div class="node-content">
        <button
          v-if="isContainer"
          class="toggle-btn"
          @click.stop="emit('toggle', path)"
        >
          {{ expanded ? "▼" : "▶" }}
        </button>
        <span v-else class="toggle-spacer"></span>

        <span class="key">{{ path[path.length - 1] ?? "root" }}</span>

        <span v-if="!isContainer || !expanded" class="summary">
          {{ summary }}
        </span>
      </div>
    </div>

    <div v-if="isContainer && expanded" class="children">
      <json-tree-node
        v-for="(child, index) in childEntries"
        :key="`${child.key}`"
        :path="[...path, child.key]"
        :value="child.value"
        :expanded="props.expanded && expanded"
        :is-selected="JSON.stringify([...path, child.key]) === JSON.stringify($props.path)"
        :search="search"
        :sort-keys="sortKeys"
        @toggle="(p) => emit('toggle', p)"
        @select="(p) => emit('select', p)"
      />
    </div>
  </div>
</template>

<style scoped>
.tree-node {
  transition: opacity 0.15s;
}

.node-row {
  display: flex;
  align-items: center;
  padding: 6px 8px;
  cursor: pointer;
  border-radius: 4px;
  transition: background 0.15s;
  user-select: none;
}

.node-row:hover {
  background: rgba(255, 255, 255, 0.08);
}

.node-row.selected {
  background: rgba(76, 175, 80, 0.2);
}

.node-content {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 1;
  min-width: 0;
}

.toggle-btn {
  background: none;
  border: none;
  color: inherit;
  cursor: pointer;
  padding: 0;
  width: 20px;
  height: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
}

.toggle-spacer {
  width: 20px;
}

.key {
  font-weight: 500;
  color: #bb86fc;
  min-width: 80px;
  flex-shrink: 0;
}

.summary {
  color: #999;
  font-size: 12px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex: 1;
}

.children {
  margin-left: 16px;
  border-left: 1px solid rgba(255, 255, 255, 0.1);
  padding-left: 0;
}
</style>
