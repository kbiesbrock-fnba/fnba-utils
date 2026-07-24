<script setup lang="ts">
import { ref, computed } from "vue";
import type { QueryResult } from "../../composables/useJsonViewer";

const props = defineProps<{
  results: QueryResult[];
  query: string;
  /** Header label — "JSONPath" for path queries, "Search" for plain text. */
  label?: string;
}>();

const expanded = ref<Set<number>>(new Set());

function toggleExpanded(index: number) {
  if (expanded.value.has(index)) {
    expanded.value.delete(index);
  } else {
    expanded.value.add(index);
  }
}

function pathStr(path: string[]): string {
  return "$" + path.map((seg) => (isNaN(Number(seg)) ? `.${seg}` : `[${seg}]`)).join("");
}

function getValuePreview(value: unknown): string {
  if (typeof value === "string") {
    return `"${value}"`;
  }
  if (Array.isArray(value)) {
    return `[${value.length}]`;
  }
  if (typeof value === "object" && value !== null) {
    return `{${Object.keys(value as Record<string, unknown>).length}}`;
  }
  return String(value);
}

function copyPath(index: number) {
  const result = props.results[index];
  navigator.clipboard.writeText(pathStr(result.path));
}

function copyValue(index: number) {
  const result = props.results[index];
  const text = typeof result.value === "string" ? result.value : JSON.stringify(result.value, null, 2);
  navigator.clipboard.writeText(text);
}
</script>

<template>
  <div class="query-results">
    <div class="query-info">
      <span class="query-label">{{ label ?? "JSONPath" }}:</span>
      <code class="query-text">{{ query }}</code>
      <span class="result-count">{{ results.length }} result{{ results.length !== 1 ? "s" : "" }}</span>
    </div>

    <div v-if="results.length === 0" class="no-results">No matches found</div>

    <div v-for="(result, index) in results" :key="index" class="result-item">
      <div class="result-header">
        <button
          class="expand-btn"
          @click="toggleExpanded(index)"
          :title="expanded.has(index) ? 'Hide context' : 'Show context'"
        >
          {{ expanded.has(index) ? "−" : "+" }}
        </button>
        <div class="result-main">
          <span class="path">{{ pathStr(result.path) }}</span>
          <span class="separator">→</span>
          <span class="value-preview">{{ getValuePreview(result.value) }}</span>
        </div>
        <div class="action-buttons">
          <button @click="copyPath(index)" title="Copy path" class="icon-btn">📋</button>
          <button @click="copyValue(index)" title="Copy value" class="icon-btn">✂️</button>
        </div>
      </div>

      <div v-if="expanded.has(index)" class="result-context">
        <pre class="context-json">{{ JSON.stringify(result.value, null, 2) }}</pre>
      </div>
    </div>
  </div>
</template>

<style scoped>
.query-results {
  display: flex;
  flex-direction: column;
  gap: 8px;
  overflow-y: auto;
  flex: 1;
}

.query-info {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
  padding: 8px;
  background: rgba(76, 175, 80, 0.1);
  border: 1px solid rgba(76, 175, 80, 0.3);
  border-radius: 4px;
  font-size: 12px;
}

.query-label {
  color: #aaa;
  font-weight: 500;
  text-transform: uppercase;
}

.query-text {
  color: #4CAF50;
  font-family: monospace;
  background: rgba(0, 0, 0, 0.2);
  padding: 2px 6px;
  border-radius: 2px;
  flex: 1;
}

.result-count {
  color: #999;
  margin-left: auto;
}

.no-results {
  color: #666;
  text-align: center;
  padding: 20px;
  font-size: 13px;
}

.result-item {
  border: 1px solid #404040;
  border-radius: 4px;
  background: #2d2d2d;
  overflow: hidden;
  /* overflow:hidden zeroes the flex min-height, letting a lone row collapse
     below its content — never shrink rows, let the container scroll. */
  flex-shrink: 0;
}

.result-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px;
  background: #262626;
  border-bottom: 1px solid #404040;
  cursor: pointer;
  transition: background 0.15s;
}

.result-header:hover {
  background: #2d2d2d;
}

.expand-btn {
  background: none;
  border: none;
  color: #4CAF50;
  cursor: pointer;
  padding: 0;
  width: 24px;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-weight: bold;
  font-size: 16px;
  flex-shrink: 0;
}

.expand-btn:hover {
  color: #66bb6a;
}

.result-main {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 1;
  min-width: 0;
}

.path {
  color: #bb86fc;
  font-family: monospace;
  font-size: 11px;
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.separator {
  color: #666;
  flex-shrink: 0;
}

.value-preview {
  color: #b0b0b0;
  font-family: monospace;
  font-size: 11px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex: 1;
}

.action-buttons {
  display: flex;
  gap: 4px;
  flex-shrink: 0;
}

.icon-btn {
  background: none;
  border: 1px solid #404040;
  color: #aaa;
  padding: 4px 8px;
  border-radius: 2px;
  cursor: pointer;
  font-size: 12px;
  transition: all 0.15s;
}

.icon-btn:hover {
  background: #404040;
  color: #ddd;
  border-color: #555;
}

.result-context {
  padding: 8px;
  background: #1e1e1e;
  border-top: 1px solid #404040;
}

.context-json {
  margin: 0;
  font-size: 11px;
  line-height: 1.4;
  color: #b0b0b0;
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 300px;
  overflow-y: auto;
}
</style>
