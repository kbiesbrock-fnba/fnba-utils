<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from "vue";
import { renderMarkdown } from "../../lib/markdown";
import { touchEntry, removeEntry, saveState, saveWin, readRegistry } from "../../lib/markdownViewerRegistry";
import { readMarkdownDoc, writeMarkdownDoc, deleteMarkdownDoc } from "../../lib/tauri";
import { openNewMarkdownViewerWindow } from "../../lib/markdownViewerWindow";
import { openExternal } from "../../lib/external";
import SplitPane from "../common/SplitPane.vue";

const source = ref("");
// Default to edit: a brand-new/empty window should drop straight into typing.
// Hydration below flips to the saved mode (usually preview) when there's content.
const mode = ref<"preview" | "edit" | "split">("edit");
let currentDocPath: string | null = null;

const renderedHtml = computed(() => renderMarkdown(source.value));

// --- Window title ---
// Shows the first heading or first non-empty line, else "Markdown Viewer".
const windowTitle = computed(() => {
  const s = source.value.trim();
  if (!s) return "Markdown Viewer";
  const firstLine = s.split("\n").find((l) => l.trim().length > 0) ?? "";
  const stripped = firstLine.replace(/^#+\s*/, "").trim();
  const title = stripped.slice(0, 40);
  return title || "Markdown Viewer";
});

watch(windowTitle, async (title) => {
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().setTitle(title);
  } catch {
    // non-critical
  }
});

// --- Link interception ---
function onPreviewClick(e: MouseEvent) {
  const a = (e.target as HTMLElement).closest("a");
  if (a) {
    const href = a.getAttribute("href");
    if (href) {
      e.preventDefault();
      void openExternal(href);
    }
  }
}

// --- Persistence ---
let persistDocTimer: ReturnType<typeof setTimeout> | null = null;

function schedulePersistDoc(label: string): void {
  if (persistDocTimer) clearTimeout(persistDocTimer);
  persistDocTimer = setTimeout(() => {
    persistDocTimer = null;
    void (async () => {
      try {
        currentDocPath = await writeMarkdownDoc(label, source.value);
        saveState(label, { docPath: currentDocPath, mode: mode.value });
      } catch {
        // ignore — file I/O is best-effort
      }
    })();
  }, 400);
}

// Watch source: update preview, touch registry, debounce doc persist.
watch(source, (val) => {
  void (async () => {
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      const label = getCurrentWindow().label;
      const preview = val.replace(/\s+/g, " ").trim().slice(0, 60);
      touchEntry(label, preview);
      schedulePersistDoc(label);
    } catch {
      // ignore
    }
  })();
});

// Watch mode: persist immediately (cheap, no debounce needed).
watch(mode, () => {
  void (async () => {
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      const label = getCurrentWindow().label;
      saveState(label, { docPath: currentDocPath, mode: mode.value });
    } catch {
      // ignore
    }
  })();
});

// --- Close ---
async function closeWindow() {
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const w = getCurrentWindow();
  try {
    if (currentDocPath) await deleteMarkdownDoc(currentDocPath);
  } catch {
    // ignore
  }
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

// --- Geometry persistence ---
let persistGeoTimer: ReturnType<typeof setTimeout> | null = null;
let lastUnmaximizedGeo: { x: number; y: number; width: number; height: number } | null = null;

function schedulePersistGeo(label: string, win: Awaited<ReturnType<typeof import("@tauri-apps/api/window")["getCurrentWindow"]>>): void {
  if (persistGeoTimer) clearTimeout(persistGeoTimer);
  persistGeoTimer = setTimeout(() => {
    persistGeoTimer = null;
    void (async () => {
      try {
        const maximized = await win.isMaximized();
        if (!maximized) {
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

function onKeydown(e: KeyboardEvent) {
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

  // --- Hydrate from saved state ---
  // (A restored window reuses its old label, so the registry entry is present.
  //  A brand-new window seeded via openNewMarkdownViewerWindow also has a
  //  pre-seeded registry entry — no pending-localStorage handoff needed.)
  try {
    const registry = readRegistry();
    const entry = registry[label];
    if (entry?.state?.docPath) {
      currentDocPath = entry.state.docPath;
      source.value = await readMarkdownDoc(currentDocPath);
      mode.value = (entry.state.mode as "preview" | "edit" | "split") ?? "preview";
    }
    if (entry?.win?.pinned) {
      pinned.value = true;
    }
    if (entry?.win && !entry.win.maximized) {
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

  // No content (new window, or a restored/empty doc) → start in edit mode so
  // the user can type immediately rather than facing an empty preview.
  if (!source.value.trim()) {
    mode.value = "edit";
  }

  // Register in MRU registry on open.
  touchEntry(label);

  // Sync maximized state and keep it in sync as the window resizes.
  isMaximized.value = await w.isMaximized();
  unlistenResize = await w.onResized(async () => {
    isMaximized.value = await w.isMaximized();
    schedulePersistGeo(label, w);
  });

  unlistenMove = await w.onMoved(() => {
    schedulePersistGeo(label, w);
  });

  unlistenFocus = await w.onFocusChanged(({ payload: focused }) => {
    if (focused) touchEntry(label);
  });

  // Intentional close via Alt+F4 or taskbar close — delete doc and remove entry.
  unlistenCloseRequested = await w.onCloseRequested(async () => {
    try {
      if (currentDocPath) await deleteMarkdownDoc(currentDocPath);
    } catch {
      // ignore
    }
    removeEntry(label);
  });

  // Seed geometry after mount.
  schedulePersistGeo(label, w);
});

onUnmounted(() => {
  window.removeEventListener("keydown", onKeydown);
  if (persistDocTimer) {
    clearTimeout(persistDocTimer);
    persistDocTimer = null;
  }
  if (persistGeoTimer) {
    clearTimeout(persistGeoTimer);
    persistGeoTimer = null;
  }
  if (unlistenResize) { unlistenResize(); unlistenResize = null; }
  if (unlistenMove) { unlistenMove(); unlistenMove = null; }
  if (unlistenFocus) { unlistenFocus(); unlistenFocus = null; }
  if (unlistenCloseRequested) { unlistenCloseRequested(); unlistenCloseRequested = null; }
  // NOTE: deleteMarkdownDoc / removeEntry are intentionally NOT called here.
  // onUnmounted fires on every Vite HMR reload while the window stays open —
  // calling them here would destroy the doc for a window the user never closed.
  // Process death never runs onUnmounted at all. Intentional closes are handled
  // by closeWindow() (Esc / ✕) and onCloseRequested (Alt+F4 / taskbar).
});
</script>

<template>
  <div class="md-viewer-app">
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
      <div class="toolbar-buttons">
        <button
          :class="{ active: mode === 'preview' }"
          @click="mode = 'preview'"
          title="Rendered preview"
        >
          👁 Preview
        </button>
        <button
          :class="{ active: mode === 'edit' }"
          @click="mode = 'edit'"
          title="Edit source"
        >
          ✏ Edit
        </button>
        <button
          :class="{ active: mode === 'split' }"
          @click="mode = 'split'"
          title="Edit and preview side by side"
        >
          ⇆ Split
        </button>
        <span class="toolbar-divider"></span>
        <button
          class="util-btn"
          @click="source = ''; mode = 'edit'"
          title="Clear content"
        >
          🗑 Clear
        </button>
        <button
          class="util-btn"
          @click="() => openNewMarkdownViewerWindow()"
          title="Open a new Markdown Viewer window"
        >
          ＋ New
        </button>
      </div>
    </div>

    <!-- Content area -->
    <div class="content">
      <div v-if="mode === 'preview'" class="preview-pane">
        <div
          v-if="source.trim()"
          class="md-body"
          v-html="renderedHtml"
          @click="onPreviewClick"
        />
        <div v-else class="placeholder">
          Nothing to preview — switch to Edit and start typing
        </div>
      </div>
      <div v-else-if="mode === 'edit'" class="edit-pane">
        <textarea
          v-model="source"
          placeholder="Type or paste Markdown…"
          class="md-editor"
        />
      </div>
      <SplitPane v-else storageKey="fnba-utils:md-split" :default-ratio="0.5">
        <template #left>
          <div class="edit-pane">
            <textarea v-model="source" placeholder="Type or paste Markdown…" class="md-editor" />
          </div>
        </template>
        <template #right>
          <div class="preview-pane">
            <div v-if="source.trim()" class="md-body" v-html="renderedHtml" @click="onPreviewClick" />
            <div v-else class="placeholder">Start typing on the left…</div>
          </div>
        </template>
      </SplitPane>
    </div>
  </div>
</template>

<style scoped>
.md-viewer-app {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: #1e1e1e;
  color: #e0e0e0;
  font-family: "Segoe UI", "Inter", sans-serif;
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
  padding: 10px 16px;
  background: #2d2d2d;
  border-bottom: 1px solid #404040;
  flex-shrink: 0;
}

.toolbar-buttons {
  display: flex;
  gap: 8px;
  align-items: center;
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
  height: 22px;
  background: #555;
  margin: 0 4px;
}

.toolbar-buttons button.util-btn {
  background: #2d2d2d;
}

.toolbar-buttons button.util-btn:hover {
  background: #3a3a3a;
  color: #ddd;
}

/* --- Content --- */
.content {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.preview-pane {
  flex: 1;
  overflow-y: auto;
  padding: 24px 32px;
  width: 100%;
}

.edit-pane {
  flex: 1;
  display: flex;
  flex-direction: column;
  padding: 12px;
  width: 100%;
}

.md-editor {
  flex: 1;
  background: #1e1e1e;
  color: #e0e0e0;
  border: 1px solid #404040;
  border-radius: 4px;
  padding: 12px;
  font-family: "Monaco", "Menlo", "Ubuntu Mono", monospace;
  font-size: 13px;
  resize: none;
  outline: none;
}

.md-editor:focus {
  border-color: #4CAF50;
}

.placeholder {
  color: #666;
  font-size: 14px;
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
}

/* --- Rendered Markdown body --- */
.md-body {
  max-width: 800px;
  line-height: 1.7;
  font-size: 14px;
  color: #e0e0e0;
}

.md-body :deep(h1),
.md-body :deep(h2),
.md-body :deep(h3),
.md-body :deep(h4),
.md-body :deep(h5),
.md-body :deep(h6) {
  color: #f0f0f0;
  font-weight: 600;
  margin: 1.2em 0 0.5em;
  line-height: 1.3;
}

.md-body :deep(h1) { font-size: 1.8em; border-bottom: 1px solid #404040; padding-bottom: 0.3em; }
.md-body :deep(h2) { font-size: 1.4em; border-bottom: 1px solid #333; padding-bottom: 0.2em; }
.md-body :deep(h3) { font-size: 1.15em; }
.md-body :deep(h4) { font-size: 1em; }
.md-body :deep(h5) { font-size: 0.9em; }
.md-body :deep(h6) { font-size: 0.85em; color: #aaa; }

.md-body :deep(p) {
  margin: 0.6em 0;
}

.md-body :deep(a) {
  color: #bb86fc;
  text-decoration: none;
}

.md-body :deep(a:hover) {
  color: #ce9ffc;
  text-decoration: underline;
}

.md-body :deep(code) {
  font-family: "Monaco", "Menlo", "Ubuntu Mono", monospace;
  font-size: 0.88em;
  background: #2d2d2d;
  color: #ce9ffc;
  padding: 0.15em 0.4em;
  border-radius: 3px;
}

.md-body :deep(pre) {
  background: #252525;
  border: 1px solid #404040;
  border-radius: 4px;
  padding: 12px 16px;
  overflow-x: auto;
  margin: 1em 0;
}

.md-body :deep(pre code) {
  background: none;
  color: #e0e0e0;
  padding: 0;
  font-size: 0.88em;
  border-radius: 0;
}

.md-body :deep(blockquote) {
  border-left: 3px solid #4CAF50;
  margin: 1em 0;
  padding: 4px 16px;
  color: #aaa;
  background: #242424;
}

.md-body :deep(ul),
.md-body :deep(ol) {
  margin: 0.6em 0;
  padding-left: 2em;
}

.md-body :deep(li) {
  margin: 0.25em 0;
}

.md-body :deep(table) {
  border-collapse: collapse;
  width: 100%;
  margin: 1em 0;
  font-size: 13px;
}

.md-body :deep(th),
.md-body :deep(td) {
  border: 1px solid #404040;
  padding: 6px 12px;
  text-align: left;
}

.md-body :deep(th) {
  background: #2d2d2d;
  color: #f0f0f0;
  font-weight: 600;
}

.md-body :deep(tr:nth-child(even)) {
  background: #242424;
}

.md-body :deep(hr) {
  border: none;
  border-top: 1px solid #404040;
  margin: 1.5em 0;
}

.md-body :deep(strong) {
  color: #f0f0f0;
  font-weight: 600;
}

.md-body :deep(em) {
  color: #d0d0d0;
  font-style: italic;
}

.md-body :deep(img) {
  max-width: 100%;
  border-radius: 4px;
}
</style>
