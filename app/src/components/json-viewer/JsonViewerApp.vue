<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { useJsonViewer, type CopyFormat, type ViewMode } from "../../composables/useJsonViewer";
import { copyText } from "../../lib/tauri";
import JsonTreeNode from "./JsonTreeNode.vue";
import JsonQueryResults from "./JsonQueryResults.vue";

const {
  input,
  parsed,
  parseError,
  search,
  sortKeys,
  mode,
  formatStyle,
  diffInput,
  diffParsed,
  selectedPath,
  expanded,
  parse,
  toggleExpand,
  selectNode,
  flatten,
  generateSchema,
  copyAs,
  parseDiffInput,
  serializePath,
  isJsonPathQuery,
  evaluateJsonPath,
  formatJson,
} = useJsonViewer();

const justCopied = ref(false);
let copyTimer: ReturnType<typeof setTimeout> | null = null;

const rootPath: string[] = [];
const rootIsExpanded = computed(() => expanded.value.has(serializePath(rootPath)));

function toggleRootExpanded() {
  toggleExpand(rootPath);
}

async function doCopy(format: CopyFormat) {
  try {
    const text = copyAs(format);
    await copyText(text);
    justCopied.value = true;
    if (copyTimer) clearTimeout(copyTimer);
    copyTimer = setTimeout(() => {
      justCopied.value = false;
      copyTimer = null;
    }, 2000);
  } catch (e) {
    console.error("Copy failed:", e);
  }
}

watch(input, () => {
  parse();
});

watch(diffInput, () => {
  parseDiffInput();
});

const flattenedOutput = computed(() => {
  if (parsed.value === null) return "";
  return flatten(parsed.value).join("\n");
});

const schemaOutput = computed(() => {
  if (parsed.value === null) return "";
  const schema = generateSchema(parsed.value);
  return JSON.stringify(schema, null, 2);
});

const isQueryMode = computed(() => isJsonPathQuery(search.value));

const queryResults = computed(() => {
  if (!isQueryMode.value || !search.value) return [];
  return evaluateJsonPath(search.value);
});

function selectedPathStr(): string {
  const path = selectedPath.value;
  if (path.length === 0) return "$";
  let result = "$";
  for (const segment of path) {
    if (isNaN(Number(segment))) {
      result += `.${segment}`;
    } else {
      result += `[${segment}]`;
    }
  }
  return result;
}
</script>

<template>
  <div class="json-viewer-app">
    <!-- Toolbar -->
    <div class="toolbar">
      <div class="search-bar">
        <span class="search-icon">🔍</span>
        <input
          v-model="search"
          type="text"
          placeholder="Search by path or value..."
          class="search-input"
        />
      </div>
      <div class="toolbar-buttons">
        <button
          :class="{ active: mode === 'format' }"
          @click="mode = 'format'"
          title="Format and display"
        >
          ✨ Format
        </button>
        <button
          :class="{ active: mode === 'tree' }"
          @click="mode = 'tree'"
          title="Tree view"
        >
          🌳 Tree
        </button>
        <button
          :class="{ active: mode === 'flatten' }"
          @click="mode = 'flatten'"
          title="Flattened dot-notation"
        >
          → Flatten
        </button>
        <button
          :class="{ active: mode === 'schema' }"
          @click="mode = 'schema'"
          title="JSON Schema"
        >
          📋 Schema
        </button>
        <button
          :class="{ active: mode === 'diff' }"
          @click="mode = 'diff'"
          title="Compare two JSON blobs"
        >
          📊 Diff
        </button>
        <button
          :class="{ active: sortKeys }"
          @click="sortKeys = !sortKeys"
          title="Sort object keys alphabetically"
        >
          A-Z
        </button>
      </div>
    </div>

    <!-- Main content -->
    <div class="content">
      <!-- Input pane (left side for tree, side-by-side for diff) -->
      <div v-if="mode === 'diff'" class="input-panes">
        <div class="input-pane">
          <label>JSON 1</label>
          <textarea v-model="input" placeholder="Paste first JSON..." />
          <div v-if="parseError" class="error">{{ parseError }}</div>
        </div>
        <div class="input-pane">
          <label>JSON 2</label>
          <textarea v-model="diffInput" placeholder="Paste second JSON..." />
        </div>
      </div>
      <div v-else class="input-pane single">
        <textarea v-model="input" placeholder="Paste JSON here..." />
        <div v-if="parseError" class="error">{{ parseError }}</div>
      </div>

      <!-- Output pane (right side) -->
      <div v-if="mode === 'tree'" class="output-pane">
        <!-- JSONPath query results -->
        <json-query-results
          v-if="isQueryMode && parsed !== null"
          :results="queryResults"
          :query="search"
        />
        <!-- Regular tree view -->
        <json-tree-node
          v-else-if="parsed !== null"
          :path="rootPath"
          :value="parsed"
          :expanded="rootIsExpanded"
          :is-selected="selectedPath.length === 0"
          :search="search"
          :sort-keys="sortKeys"
          @toggle="toggleExpand"
          @select="selectNode"
        />
        <div v-else class="placeholder">Paste JSON to view tree</div>
      </div>
      <div v-else-if="mode === 'flatten'" class="output-pane">
        <pre v-if="flattenedOutput" class="output-text">{{ flattenedOutput }}</pre>
        <div v-else class="placeholder">Paste JSON to flatten</div>
      </div>
      <div v-else-if="mode === 'schema'" class="output-pane">
        <pre v-if="schemaOutput" class="output-text">{{ schemaOutput }}</pre>
        <div v-else class="placeholder">Paste JSON to generate schema</div>
      </div>
      <div v-else-if="mode === 'format'" class="output-pane">
        <div v-if="parsed !== null" class="format-controls">
          <label>Format style:</label>
          <select v-model="formatStyle" class="format-select">
            <option value="pretty2">Pretty (2-space indent)</option>
            <option value="pretty4">Pretty (4-space indent)</option>
            <option value="minified">Minified</option>
            <option value="compact">Compact (one item per line)</option>
          </select>
        </div>
        <pre v-if="parsed !== null" class="output-text format-output">{{ formatJson(formatStyle) }}</pre>
        <div v-else class="placeholder">Paste JSON to format</div>
      </div>
      <div v-else-if="mode === 'diff'" class="output-pane">
        <div v-if="parsed && diffParsed" class="diff-view">
          <div>Diff view (simplified)</div>
          <pre class="output-text">{{ JSON.stringify({ json1: parsed, json2: diffParsed }, null, 2) }}</pre>
        </div>
        <div v-else class="placeholder">Paste both JSON blobs to compare</div>
      </div>
    </div>

    <!-- Status bar -->
    <div class="status-bar">
      <div class="path-display">{{ selectedPathStr() }}</div>
      <div class="copy-controls">
        <div class="copy-dropdown">
          <button
            class="copy-btn"
            :class="{ copied: justCopied }"
            @click="doCopy('pretty')"
            title="Copy selected branch or whole JSON as prettified"
          >
            {{ justCopied ? "✓ Copied" : "📋 Copy" }}
          </button>
          <div class="dropdown-menu">
            <button @click="doCopy('pretty')">Prettified (2-space)</button>
            <button @click="doCopy('minified')">Minified</button>
            <button @click="doCopy('jsonpath')">JSONPath</button>
            <button @click="doCopy('jsonpath-wildcard')">JSONPath (wildcards)</button>
            <button @click="doCopy('value')">Value only</button>
            <button @click="doCopy('branch')">Branch</button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.json-viewer-app {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: #1e1e1e;
  color: #e0e0e0;
  font-family: "Monaco", "Menlo", "Ubuntu Mono", monospace;
}

.toolbar {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 12px 16px;
  background: #2d2d2d;
  border-bottom: 1px solid #404040;
  flex-shrink: 0;
}

.search-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 1;
  background: #333;
  border-radius: 4px;
  padding: 6px 10px;
}

.search-icon {
  font-size: 14px;
}

.search-input {
  background: none;
  border: none;
  color: inherit;
  outline: none;
  flex: 1;
  font-size: 13px;
}

.toolbar-buttons {
  display: flex;
  gap: 8px;
  flex-shrink: 0;
}

.toolbar-buttons button {
  padding: 6px 12px;
  background: #404040;
  border: 1px solid #555;
  color: #aaa;
  border-radius: 4px;
  cursor: pointer;
  font-size: 12px;
  transition: all 0.15s;
}

.toolbar-buttons button:hover {
  background: #505050;
  color: #ddd;
}

.toolbar-buttons button.active {
  background: #4CAF50;
  border-color: #45a049;
  color: white;
}

.content {
  display: flex;
  gap: 16px;
  flex: 1;
  padding: 12px;
  overflow: hidden;
}

.input-panes {
  display: flex;
  gap: 12px;
  flex: 0.5;
  overflow: hidden;
}

.input-pane {
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex: 1;
  background: #2d2d2d;
  border: 1px solid #404040;
  border-radius: 4px;
  padding: 8px;
  overflow: hidden;
}

.input-pane.single {
  flex: 0.4;
}

.input-pane label {
  font-size: 11px;
  color: #aaa;
  font-weight: 500;
  text-transform: uppercase;
}

textarea {
  flex: 1;
  background: #1e1e1e;
  color: #e0e0e0;
  border: none;
  border-radius: 2px;
  padding: 8px;
  font-family: inherit;
  font-size: 12px;
  resize: none;
  overflow-y: auto;
}

textarea:focus {
  outline: 1px solid #4CAF50;
}

.error {
  color: #ff6b6b;
  font-size: 12px;
  padding: 6px;
  background: rgba(255, 107, 107, 0.1);
  border-radius: 2px;
}

.output-pane {
  flex: 0.6;
  background: #2d2d2d;
  border: 1px solid #404040;
  border-radius: 4px;
  padding: 8px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
}

.output-text {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
  font-size: 12px;
  line-height: 1.4;
  color: #b0b0b0;
}

.placeholder {
  color: #666;
  font-size: 13px;
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
}

.diff-view {
  overflow-y: auto;
  flex: 1;
}

.status-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 16px;
  background: #2d2d2d;
  border-top: 1px solid #404040;
  flex-shrink: 0;
  font-size: 12px;
}

.path-display {
  color: #bb86fc;
  font-family: inherit;
  flex: 1;
}

.copy-controls {
  display: flex;
  gap: 8px;
}

.copy-dropdown {
  position: relative;
}

.copy-btn {
  padding: 6px 12px;
  background: #4CAF50;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-size: 12px;
  transition: background 0.15s;
}

.copy-btn:hover {
  background: #45a049;
}

.copy-btn.copied {
  background: #66bb6a;
}

.dropdown-menu {
  display: none;
  position: absolute;
  bottom: 100%;
  right: 0;
  background: #333;
  border: 1px solid #555;
  border-radius: 4px;
  margin-bottom: 4px;
  min-width: 160px;
  z-index: 10;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
}

.copy-dropdown:hover .dropdown-menu {
  display: flex;
  flex-direction: column;
}

.dropdown-menu button {
  background: none;
  border: none;
  color: #e0e0e0;
  padding: 8px 12px;
  text-align: left;
  cursor: pointer;
  font-size: 12px;
  transition: background 0.15s;
}

.dropdown-menu button:hover {
  background: #404040;
}

.format-controls {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px;
  background: #262626;
  border-bottom: 1px solid #404040;
  font-size: 12px;
}

.format-controls label {
  color: #aaa;
  font-weight: 500;
  white-space: nowrap;
}

.format-select {
  background: #333;
  border: 1px solid #555;
  color: #e0e0e0;
  padding: 4px 8px;
  border-radius: 2px;
  font-size: 12px;
  cursor: pointer;
}

.format-select:hover {
  background: #3a3a3a;
  border-color: #666;
}

.format-select:focus {
  outline: 1px solid #4CAF50;
}

.format-output {
  max-height: 100%;
  overflow-y: auto;
}
</style>
