<script setup lang="ts">
import { ref, computed, watch, onMounted } from "vue";
import { useJsonViewer, type CopyFormat, type ViewMode, type FormatStyle } from "../../composables/useJsonViewer";
import { copyText, writeViewerDoc } from "../../lib/tauri";
import { openNewFileViewerWindow } from "../../lib/fileViewerWindow";
import { saveState, readRegistry, type JsonViewerState } from "../../lib/fileViewerRegistry";
import { useViewerWindowChrome } from "../../composables/useViewerWindowChrome";
import { useFileBackedDoc } from "../../composables/useFileBackedDoc";
import { useRevealSearch } from "../../composables/useRevealSearch";
import { baseName } from "../../lib/pathUtils";
import ViewerTitleBar from "../file-viewer/ViewerTitleBar.vue";
import ViewerToolbarRow1 from "../file-viewer/ViewerToolbarRow1.vue";
import ViewerSearchBar from "../file-viewer/ViewerSearchBar.vue";
import ExternalChangeBanner from "../file-viewer/ExternalChangeBanner.vue";
import SaveCloseModal from "../file-viewer/SaveCloseModal.vue";
import JsonTreeNode from "./JsonTreeNode.vue";
import JsonQueryResults from "./JsonQueryResults.vue";
import SplitPane from "../common/SplitPane.vue";

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
  clearAll,
  toggleExpand,
  selectNode,
  flatten,
  generateSchema,
  copyAs,
  parseDiffInput,
  serializePath,
  isJsonPathQuery,
  evaluateJsonPath,
  searchResults,
  formatJson,
} = useJsonViewer();

// Preview/Edit/Split layout axis — a DIFFERENT concept than `mode` above
// (useJsonViewer's tree/flatten/schema/diff/format/query view-type axis).
// Kept as a separate local ref per the naming-collision note in the File
// Viewer parity plan; useJsonViewer.ts's `mode` and its exports are untouched.
const layoutMode = ref<"preview" | "edit" | "split">("split");

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

// Any non-empty search swaps the output pane to the results panel, whatever
// the active view mode — JSONPath queries and plain-text searches alike.
const searchActive = computed(
  () => search.value.trim().length > 0 && parsed.value !== null,
);

const queryResults = computed(() => {
  if (!searchActive.value) return [];
  return isQueryMode.value
    ? evaluateJsonPath(search.value)
    : searchResults(search.value);
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

// --- File-backed doc: docPath persistence, dirty-tracking, external-change
// detection, unsaved-changes close-prompt, Open/Save/Save-As, Ctrl+S/O. ---
const fileDoc = useFileBackedDoc({
  kind: "json",
  content: input,
  suggestedName: (val) => {
    const first = val.split("\n").find((l) => l.trim()) ?? "";
    const slug = first
      .trim()
      .slice(0, 40)
      .replace(/[^\w.-]+/g, "-")
      .replace(/^-+|-+$/g, "");
    return (slug || "untitled") + ".json";
  },
  extraState: () => ({
    diffInput: diffInput.value,
    mode: mode.value,
    formatStyle: formatStyle.value,
    sortKeys: sortKeys.value,
    layoutMode: layoutMode.value,
  }),
  hydrateExtra: (s) => {
    if (typeof s.diffInput === "string") diffInput.value = s.diffInput;
    if (typeof s.mode === "string") mode.value = s.mode as ViewMode;
    if (typeof s.formatStyle === "string") formatStyle.value = s.formatStyle as FormatStyle;
    if (typeof s.sortKeys === "boolean") sortKeys.value = s.sortKeys;
    if (typeof s.layoutMode === "string") layoutMode.value = s.layoutMode as "preview" | "edit" | "split";
  },
});

// --- Window title ---
// Bound files show filename + dirty indicator. Otherwise shows the first few
// root keys (object) or item count (array), else "JSON Viewer".
const windowTitle = computed(() => {
  if (fileDoc.filePath.value) {
    return baseName(fileDoc.filePath.value) + (fileDoc.dirty.value ? " ●" : "");
  }
  const p = parsed.value;
  if (p === null || p === undefined) return "JSON Viewer";
  if (Array.isArray(p)) return `JSON · [${p.length} item${p.length === 1 ? "" : "s"}]`;
  if (typeof p === "object") {
    const keys = Object.keys(p as object);
    if (keys.length === 0) return "JSON · {}";
    const preview = keys.slice(0, 4).join(", ");
    const truncated = preview.length > 36 ? preview.slice(0, 36) + "…" : preview;
    return `JSON · {${truncated}}`;
  }
  return "JSON Viewer";
});

// --- Ctrl+F reveal search ---
const { isOpen: searchOpen, close: closeSearchRaw, toggle: toggleSearch } = useRevealSearch();
function closeSearch() {
  search.value = "";
  closeSearchRaw();
}
watch(search, (q) => {
  if (q.trim() && layoutMode.value === "edit") layoutMode.value = "split";
});

// --- Title bar controls ---
// Shared window chrome (pin/minimize/maximize, geometry persistence,
// Escape/F11/Ctrl+F, title-watch, focus/close Tauri-event wiring). JSON now
// gets the same unsaved-changes-close-prompt + external-change recheck
// Markdown already had.
const { pinned, isMaximized, togglePin, minimize, toggleMaximize } = useViewerWindowChrome({
  title: windowTitle,
  onEscapeClose: fileDoc.closeWindow,
  onNativeCloseRequest: fileDoc.onNativeCloseRequest,
  onFocusGained: () => void fileDoc.checkExternalChange(),
  onToggleSearch: toggleSearch,
});

// Persist state on any viewer-state change beyond input (mode, formatStyle,
// sortKeys, layoutMode, diffInput). `search` is intentionally excluded — it's
// ephemeral (never persisted; resets to empty on every fresh window).
watch([diffInput, mode, formatStyle, sortKeys, layoutMode], () => fileDoc.schedulePersist());

onMounted(async () => {
  // --- One-time legacy-shape migration ---
  // Existing installs have JSON registry entries shaped `{ input: "...", ... }`
  // with no docPath. Upgrade them to the doc-cache before hydrating.
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const label = getCurrentWindow().label;
    const legacy = readRegistry()[label]?.state as (JsonViewerState & { input?: string }) | undefined;
    if (legacy && !legacy.docPath && typeof legacy.input === "string" && legacy.input) {
      const docPath = await writeViewerDoc(label, "json", legacy.input);
      saveState(label, { ...legacy, docPath, filePath: null, dirty: false });
    }
  } catch {
    // ignore — migration is best-effort
  }

  await fileDoc.hydrate();
  parse();
  if (diffInput.value) parseDiffInput();
});
</script>

<template>
  <div class="json-viewer-app">
    <!-- Title bar -->
    <ViewerTitleBar
      :title="windowTitle"
      :pinned="pinned"
      :is-maximized="isMaximized"
      @pin="togglePin"
      @minimize="minimize"
      @maximize="toggleMaximize"
      @close="fileDoc.closeWindow"
    />

    <!-- Toolbar row 1 (Preview/Edit/Split + file actions + utility actions) -->
    <ViewerToolbarRow1 v-model:layout-mode="layoutMode">
      <template #file-actions>
        <button class="util-btn" @click="fileDoc.openFile" title="Open a JSON file (Ctrl+O)">📂 Open</button>
        <button class="util-btn" @click="fileDoc.save" title="Save (Ctrl+S)">💾 Save</button>
        <button class="util-btn" @click="fileDoc.saveAs" title="Save As… (Ctrl+Shift+S)">Save As…</button>
      </template>
      <template #utility-actions>
        <button class="util-btn" :class="{ active: searchOpen }" @click="toggleSearch" title="Find (Ctrl+F)">🔍 Find</button>
        <button
          class="util-btn"
          @click="clearAll"
          title="Clear all input and reset"
        >
          🗑 Clear
        </button>
        <button
          class="util-btn"
          @click="() => openNewFileViewerWindow({ kind: 'json' })"
          title="Open a new JSON Viewer window"
        >
          ＋ New
        </button>
      </template>
    </ViewerToolbarRow1>

    <!-- Toolbar row 2 (JSON-only: view-type buttons) -->
    <div class="toolbar-row2">
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

    <!-- Find search bar (Ctrl+F reveal) -->
    <ViewerSearchBar
      v-if="searchOpen"
      v-model="search"
      placeholder="Search by path or value..."
      :match-count="searchActive ? queryResults.length : null"
      @close="closeSearch"
    />

    <!-- External-change banner (between toolbar and content; pushes content down) -->
    <ExternalChangeBanner
      :state="fileDoc.externalChange.value"
      :dirty="fileDoc.dirty.value"
      @reload="fileDoc.reloadFromDisk"
      @open-disk-copy="fileDoc.openDiskCopy"
      @keep-mine="fileDoc.keepMine"
      @save-again="fileDoc.saveOverDeleted"
      @dismiss="fileDoc.dismissExternal"
    />

    <!-- Main content -->
    <div class="content">
      <template v-if="layoutMode === 'preview'">
        <!-- Output pane (right side) -->
        <!-- An active search takes over the output pane in every mode -->
        <div v-if="searchActive" class="output-pane">
          <json-query-results
            :results="queryResults"
            :query="search"
            :label="isQueryMode ? 'JSONPath' : 'Search'"
          />
        </div>
        <div v-else-if="mode === 'tree'" class="output-pane">
          <json-tree-node
            v-if="parsed !== null"
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
      </template>
      <template v-else-if="layoutMode === 'edit'">
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
      </template>
      <SplitPane v-else storageKey="fnba-utils:json-split" :default-ratio="0.4">
        <template #left>
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
        </template>
        <template #right>
          <!-- Output pane (right side) -->
          <!-- An active search takes over the output pane in every mode -->
          <div v-if="searchActive" class="output-pane">
            <json-query-results
              :results="queryResults"
              :query="search"
              :label="isQueryMode ? 'JSONPath' : 'Search'"
            />
          </div>
          <div v-else-if="mode === 'tree'" class="output-pane">
            <json-tree-node
              v-if="parsed !== null"
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
        </template>
      </SplitPane>
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

    <!-- Save-on-close modal -->
    <SaveCloseModal
      :show="fileDoc.showCloseModal.value"
      :label="fileDoc.filePath.value ? baseName(fileDoc.filePath.value) : 'This document'"
      @save="fileDoc.onModalSave"
      @discard="fileDoc.onModalDiscard"
      @cancel="fileDoc.onModalCancel"
    />
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

/* --- Toolbar row 2 (JSON-only view-type buttons) --- */
.toolbar-row2 {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 16px;
  background: #262626;
  border-bottom: 1px solid #404040;
  flex-shrink: 0;
}

.toolbar-row2 button {
  padding: 6px 12px;
  background: #404040;
  border: 1px solid #555;
  color: #aaa;
  border-radius: 4px;
  cursor: pointer;
  font-size: 12px;
  transition: all 0.15s;
}

.toolbar-row2 button:hover {
  background: #505050;
  color: #ddd;
}

.toolbar-row2 button.active {
  background: #4CAF50;
  border-color: #45a049;
  color: white;
}

.content {
  display: flex;
  flex: 1;
  padding: 12px;
  overflow: hidden;
}

.input-panes {
  display: flex;
  gap: 12px;
  flex: 1;
  width: 100%;
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
  flex: 1;
  width: 100%;
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
  flex: 1;
  width: 100%;
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
