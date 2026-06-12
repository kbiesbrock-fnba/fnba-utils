<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from "vue";
import { useJsonViewer, type CopyFormat, type ViewMode, type FormatStyle } from "../../composables/useJsonViewer";
import { copyText } from "../../lib/tauri";
import { openNewJsonViewerWindow } from "../../lib/jsonViewerWindow";
import { touchEntry, removeEntry, saveState, saveWin, readRegistry } from "../../lib/jsonViewerRegistry";
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

// All JSON Viewer windows are dynamic (`json-viewer:*`) — always close.
async function closeWindow() {
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const w = getCurrentWindow();
  removeEntry(w.label);
  await w.close();
}

// --- Title bar controls ---
const pinned = ref(false);
const isMaximized = ref(false);
let unlistenResize: (() => void) | null = null;
let unlistenMove: (() => void) | null = null;
let unlistenFocus: (() => void) | null = null;
let unlistenCloseRequested: (() => void) | null = null;

// --- Persistence helpers ---

// Debounce timer handles for state and geometry persistence.
let persistStateTimer: ReturnType<typeof setTimeout> | null = null;
let persistGeoTimer: ReturnType<typeof setTimeout> | null = null;

// Last known un-maximized geometry so that restoring after maximize lands
// on the previous footprint rather than the default size.
let lastUnmaximizedGeo: { x: number; y: number; width: number; height: number } | null = null;

/** Debounced: persist viewer state (input, mode, search, etc.) to localStorage. */
function schedulePersistState(label: string): void {
  if (persistStateTimer) clearTimeout(persistStateTimer);
  persistStateTimer = setTimeout(() => {
    persistStateTimer = null;
    saveState(label, {
      input: input.value,
      diffInput: diffInput.value,
      mode: mode.value,
      formatStyle: formatStyle.value,
      sortKeys: sortKeys.value,
      search: search.value,
    });
  }, 400);
}

/** Debounced: persist window geometry/pin/maximized state to localStorage. */
function schedulePersistGeo(label: string, win: Awaited<ReturnType<typeof import("@tauri-apps/api/window")["getCurrentWindow"]>>): void {
  if (persistGeoTimer) clearTimeout(persistGeoTimer);
  persistGeoTimer = setTimeout(() => {
    persistGeoTimer = null;
    void (async () => {
      try {
        const maximized = await win.isMaximized();
        if (!maximized) {
          // Only update geometry when not maximized — preserve the last
          // un-maximized footprint so restore-then-unmaximize lands correctly.
          const sf = await win.scaleFactor();
          const pos = (await win.outerPosition()).toLogical(sf);
          const size = (await win.innerSize()).toLogical(sf);
          lastUnmaximizedGeo = {
            x: Math.round(pos.x),
            y: Math.round(pos.y),
            width: Math.round(size.width),
            height: Math.round(size.height),
          };
        }
        saveWin(label, {
          x: lastUnmaximizedGeo?.x ?? 0,
          y: lastUnmaximizedGeo?.y ?? 0,
          width: lastUnmaximizedGeo?.width ?? 1000,
          height: lastUnmaximizedGeo?.height ?? 700,
          pinned: pinned.value,
          maximized,
        });
      } catch {
        // ignore — geometry persistence is best-effort
      }
    })();
  }, 400);
}

async function togglePin() {
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const w = getCurrentWindow();
  pinned.value = !pinned.value;
  await w.setAlwaysOnTop(pinned.value);
  // Persist pin state change immediately.
  schedulePersistGeo(w.label, w);
}

async function minimize() {
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().minimize();
}

async function toggleMaximize() {
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().toggleMaximize();
}

// --- Window title ---
// Shows the first few root keys (object) or item count (array), else "JSON Viewer".
const windowTitle = computed(() => {
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

watch(windowTitle, async (title) => {
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().setTitle(title);
  } catch {
    // non-critical
  }
});

function onKeydown(e: KeyboardEvent) {
  // Don't steal shortcuts while the user is mid-edit in a text field.
  const target = e.target as HTMLElement | null;
  const inEditable =
    target &&
    (target.tagName === "TEXTAREA" ||
      target.tagName === "INPUT" ||
      target.isContentEditable);
  if (e.key === "Escape" && !inEditable) {
    e.preventDefault();
    void closeWindow();
  }
  if (e.key === "F11") {
    e.preventDefault();
    void (async () => {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      const w = getCurrentWindow();
      await w.setFullscreen(!(await w.isFullscreen()));
    })();
  }
}

onMounted(async () => {
  window.addEventListener("keydown", onKeydown);

  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const w = getCurrentWindow();
  const label = w.label;

  // --- Hydrate from saved state BEFORE the pending-blob handoff ---
  // (A restored window reuses its old label, so the registry entry is present.)
  try {
    const registry = readRegistry();
    const entry = registry[label];
    if (entry?.state) {
      const s = entry.state;
      input.value = s.input;
      diffInput.value = s.diffInput;
      mode.value = s.mode as ViewMode;
      formatStyle.value = s.formatStyle as FormatStyle;
      sortKeys.value = s.sortKeys;
      search.value = s.search;
      // Trigger parse so parsed/diffParsed are populated from restored input.
      parse();
      if (diffInput.value) parseDiffInput();
    }
    if (entry?.win?.pinned) {
      pinned.value = true;
      // alwaysOnTop was already set at window creation time, no need to re-apply.
    }
    if (entry?.win && !entry.win.maximized) {
      // Seed last-known un-maximized geo so a toggleMaximize→restore cycle works.
      lastUnmaximizedGeo = {
        x: entry.win.x,
        y: entry.win.y,
        width: entry.win.width,
        height: entry.win.height,
      };
    }
  } catch {
    // ignore — hydration is best-effort
  }

  // --- Pending-blob handoff (palette "Open in JSON Viewer" soft command) ---
  // This runs AFTER hydration. A brand-new window has a fresh label with no
  // registry state, so hydration is a no-op and the pending blob wins cleanly.
  // A restored window won't have a pending blob (they share a label, not the
  // pending key) so the two paths don't collide in practice.
  try {
    const pending = localStorage.getItem("fnba-utils:json-viewer-pending");
    if (pending != null) {
      localStorage.removeItem("fnba-utils:json-viewer-pending");
      input.value = pending;
    }
  } catch {
    // ignore
  }

  // Register in MRU registry on open.
  touchEntry(label);

  // Persist initial state once so an immediate recompile can restore it.
  schedulePersistState(label);

  // Sync maximized state and keep it in sync as the window resizes.
  isMaximized.value = await w.isMaximized();
  unlistenResize = await w.onResized(async () => {
    isMaximized.value = await w.isMaximized();
    schedulePersistGeo(label, w);
  });

  // Persist geometry on window move.
  unlistenMove = await w.onMoved(() => {
    schedulePersistGeo(label, w);
  });

  // OS-level focus (taskbar click, switcher, Alt-Tab) — the DOM "focus" event
  // doesn't fire for window activation in WebView2, so use Tauri's event.
  unlistenFocus = await w.onFocusChanged(({ payload: focused }) => {
    if (focused) touchEntry(label);
  });

  // Intentional close via Alt+F4 or taskbar close — remove entry so the window
  // is NOT restored on next launch. (closeWindow() also calls removeEntry for
  // the Esc / ✕ button paths; double-remove is harmless.)
  unlistenCloseRequested = await w.onCloseRequested(() => {
    removeEntry(label);
  });

  // Seed geometry after mount so a first-launch window persists position before
  // the user has moved or resized it.
  schedulePersistGeo(label, w);
});

onUnmounted(() => {
  window.removeEventListener("keydown", onKeydown);
  if (persistStateTimer) {
    clearTimeout(persistStateTimer);
    persistStateTimer = null;
  }
  if (persistGeoTimer) {
    clearTimeout(persistGeoTimer);
    persistGeoTimer = null;
  }
  if (unlistenResize) {
    unlistenResize();
    unlistenResize = null;
  }
  if (unlistenMove) {
    unlistenMove();
    unlistenMove = null;
  }
  if (unlistenFocus) {
    unlistenFocus();
    unlistenFocus = null;
  }
  if (unlistenCloseRequested) {
    unlistenCloseRequested();
    unlistenCloseRequested = null;
  }
  // NOTE: removeEntry is intentionally NOT called here. onUnmounted fires on
  // every webview reload (Vite HMR / F5) while the window stays open — calling
  // removeEntry here would wipe persisted state for a window the user never
  // closed. Process death never runs onUnmounted at all. Intentional closes are
  // handled by closeWindow() (Esc / ✕) and onCloseRequested (Alt+F4 / taskbar).
});

// Update registry preview as the user types (first 60 chars, single-line),
// and debounce full state persistence.
watch(input, (val) => {
  void (async () => {
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      const label = getCurrentWindow().label;
      const preview = val.replace(/\s+/g, " ").trim().slice(0, 60);
      touchEntry(label, preview);
      schedulePersistState(label);
    } catch {
      // ignore
    }
  })();
});

// Persist state on any viewer-state change beyond input (mode, formatStyle,
// sortKeys, search, diffInput). Debounced together with the input watcher above.
watch([diffInput, mode, formatStyle, sortKeys, search], () => {
  void (async () => {
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      schedulePersistState(getCurrentWindow().label);
    } catch {
      // ignore
    }
  })();
});
</script>

<template>
  <div class="json-viewer-app">
    <!-- Title bar -->
    <div class="title-bar" data-tauri-drag-region>
      <span class="tb-title" data-tauri-drag-region>{{ windowTitle }}</span>
      <div class="tb-buttons">
        <button class="tb-btn" :class="{ active: pinned }" @click="togglePin" title="Keep on top">📌</button>
        <button class="tb-btn" @click="minimize" title="Minimize">—</button>
        <button class="tb-btn" @click="toggleMaximize" :title="isMaximized ? 'Restore' : 'Maximize'">{{ isMaximized ? '🗗' : '🗖' }}</button>
        <button class="tb-btn close" @click="closeWindow" title="Close (Esc)">✕</button>
      </div>
    </div>

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
        <span class="toolbar-divider"></span>
        <button
          class="util-btn"
          @click="clearAll"
          title="Clear all input and reset"
        >
          🗑 Clear
        </button>
        <button
          class="util-btn"
          @click="() => openNewJsonViewerWindow()"
          title="Open a new JSON Viewer window"
        >
          ＋ New
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

/* --- Title bar --- */
.title-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 32px;
  padding: 0 8px 0 12px;
  background: #252525;
  border-bottom: 1px solid #404040;
  flex-shrink: 0;
  -webkit-app-region: drag;
  user-select: none;
}

.tb-title {
  font-size: 12px;
  color: #aaa;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
  min-width: 0;
  -webkit-app-region: drag;
}

.tb-buttons {
  display: flex;
  gap: 2px;
  flex-shrink: 0;
  -webkit-app-region: no-drag;
}

.tb-btn {
  width: 28px;
  height: 24px;
  padding: 0;
  background: transparent;
  border: none;
  color: #888;
  border-radius: 3px;
  cursor: pointer;
  font-size: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background 0.12s, color 0.12s;
  -webkit-app-region: no-drag;
}

.tb-btn:hover {
  background: #3a3a3a;
  color: #ddd;
}

.tb-btn.active {
  color: #4CAF50;
}

.tb-btn.active:hover {
  background: #3a3a3a;
  color: #66bb6a;
}

.tb-btn.close:hover {
  background: #c0392b;
  color: white;
}

/* --- Toolbar --- */
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

.toolbar-divider {
  width: 1px;
  align-self: stretch;
  background: #555;
  margin: 2px 4px;
}

.toolbar-buttons button.util-btn {
  background: #2d2d2d;
}

.toolbar-buttons button.util-btn:hover {
  background: #3a3a3a;
  color: #ddd;
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
