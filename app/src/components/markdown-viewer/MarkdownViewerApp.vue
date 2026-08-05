<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onUnmounted } from "vue";
import { renderMarkdown } from "../../lib/markdown";
import { openNewFileViewerWindow } from "../../lib/fileViewerWindow";
import { openExternal } from "../../lib/external";
import { useViewerWindowChrome } from "../../composables/useViewerWindowChrome";
import { useFileBackedDoc } from "../../composables/useFileBackedDoc";
import { useRevealSearch } from "../../composables/useRevealSearch";
import { baseName } from "../../lib/pathUtils";
import ViewerTitleBar from "../file-viewer/ViewerTitleBar.vue";
import ViewerToolbarRow1 from "../file-viewer/ViewerToolbarRow1.vue";
import ViewerSearchBar from "../file-viewer/ViewerSearchBar.vue";
import ExternalChangeBanner from "../file-viewer/ExternalChangeBanner.vue";
import SaveCloseModal from "../file-viewer/SaveCloseModal.vue";
import SplitPane from "../common/SplitPane.vue";
import StatusBar from "../StatusBar.vue";

const source = ref("");
// Default to edit: a brand-new/empty window should drop straight into typing.
// Hydration below flips to the saved mode (usually preview) when there's content.
const mode = ref<"preview" | "edit" | "split">("edit");

const splitEditorRef = ref<HTMLTextAreaElement | null>(null);
const splitPreviewRef = ref<HTMLElement | null>(null);
// Ref for the solo edit-mode textarea (distinct element from splitEditorRef).
const editRef = ref<HTMLTextAreaElement | null>(null);
// Source char-range captured from a preview selection; carried into the editor
// on the next mode switch to 'edit' or 'split'.
let previewSelRange: { start: number; end: number } | null = null;
// Guards the scroll-sync feedback loop (programmatic scroll re-fires @scroll).
let syncing = false;

const renderedHtml = computed(() => renderMarkdown(source.value));

// --- File-backed doc: docPath persistence, dirty-tracking, external-change
// detection, unsaved-changes close-prompt, Open/Save/Save-As, Ctrl+S/O. ---
const fileDoc = useFileBackedDoc({
  kind: "markdown",
  content: source,
  suggestedName: (val) => {
    const first = val.split("\n").find((l) => l.trim()) ?? "";
    const slug = first
      .replace(/^#+\s*/, "")
      .trim()
      .slice(0, 40)
      .replace(/[^\w.-]+/g, "-")
      .replace(/^-+|-+$/g, "");
    return (slug || "untitled") + ".md";
  },
  extraState: () => ({ mode: mode.value }),
  hydrateExtra: (s) => {
    mode.value = (s.mode as "preview" | "edit" | "split") ?? "preview";
  },
});

// --- Window title ---
// For bound files: show filename + dirty indicator.
// For unbound (scratch): shows the first heading or first non-empty line, else "Markdown Viewer".
const windowTitle = computed(() => {
  if (fileDoc.filePath.value) {
    return baseName(fileDoc.filePath.value) + (fileDoc.dirty.value ? " ●" : "");
  }
  const s = source.value.trim();
  if (!s) return "Markdown Viewer";
  const firstLine = s.split("\n").find((l) => l.trim().length > 0) ?? "";
  const stripped = firstLine.replace(/^#+\s*/, "").trim();
  const title = stripped.slice(0, 40);
  return title || "Markdown Viewer";
});

// --- Scroll sync helpers ---

function editorLineHeight(el: HTMLTextAreaElement): number {
  const cs = getComputedStyle(el);
  const lh = parseFloat(cs.lineHeight);
  if (Number.isFinite(lh)) return lh;
  return (parseFloat(cs.fontSize) || 13) * 1.5;
}

// --- Source-line ↔ editor-pixel mapping (wrap-aware) ---
// The textarea soft-wraps long lines, so a source line's pixel offset is NOT
// `line * lineHeight`. We measure it with a hidden mirror <div> that replicates
// the textarea's width + typography, one <span> per source line; each span's
// offsetTop is that line's true top in the textarea's scroll coordinates.
// Results are cached until the text or editor width changes.
const MIRROR_PROPS = [
  "boxSizing", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft",
  "fontFamily", "fontSize", "fontWeight", "fontStyle", "fontVariant",
  "letterSpacing", "lineHeight", "textTransform", "wordSpacing", "tabSize",
  "overflowWrap", "wordBreak", "wordWrap",
];
let mirrorEl: HTMLDivElement | null = null;
let cachedTops: number[] | null = null;
let cachedText = "";
let cachedWidth = -1;

function lineTops(editor: HTMLTextAreaElement): number[] {
  const text = source.value;
  const width = editor.clientWidth;
  if (cachedTops && cachedText === text && cachedWidth === width) return cachedTops;

  if (!mirrorEl) {
    mirrorEl = document.createElement("div");
    mirrorEl.setAttribute("aria-hidden", "true");
    Object.assign(mirrorEl.style, {
      position: "absolute", top: "0", left: "-9999px",
      visibility: "hidden", height: "auto", overflow: "hidden", pointerEvents: "none",
    });
    document.body.appendChild(mirrorEl);
  }
  const cs = getComputedStyle(editor);
  for (const p of MIRROR_PROPS) {
    (mirrorEl.style as unknown as Record<string, string>)[p] =
      (cs as unknown as Record<string, string>)[p];
  }
  mirrorEl.style.width = `${width}px`;
  mirrorEl.style.whiteSpace = "pre-wrap";

  // Build with safe DOM methods (textContent) — no innerHTML. One span per
  // source line, joined by literal newlines so pre-wrap breaks between them.
  const lines = text.split("\n");
  mirrorEl.replaceChildren();
  const spans: HTMLElement[] = [];
  lines.forEach((l, i) => {
    if (i > 0) mirrorEl!.appendChild(document.createTextNode("\n"));
    const span = document.createElement("span");
    span.textContent = l.length ? l : "​"; // ZWSP keeps blank lines tall
    mirrorEl!.appendChild(span);
    spans.push(span);
  });
  const tops: number[] = spans.map((s) => s.offsetTop);

  cachedTops = tops;
  cachedText = text;
  cachedWidth = width;
  return tops;
}

/** Fractional source line shown at editor scroll offset `scrollTop`. */
function lineAtScroll(editor: HTMLTextAreaElement, scrollTop: number): number {
  const tops = lineTops(editor);
  if (tops.length === 0) return 0;
  let i = 0;
  while (i + 1 < tops.length && tops[i + 1] <= scrollTop) i++;
  const top = tops[i];
  const next = i + 1 < tops.length ? tops[i + 1] : top + editorLineHeight(editor);
  const frac = next > top ? (scrollTop - top) / (next - top) : 0;
  return i + Math.min(1, Math.max(0, frac));
}

/** Editor scroll offset that puts (fractional) source `line` at the top. */
function scrollForLine(editor: HTMLTextAreaElement, line: number): number {
  const tops = lineTops(editor);
  if (tops.length === 0) return 0;
  const i = Math.max(0, Math.min(Math.floor(line), tops.length - 1));
  const top = tops[i];
  const next = i + 1 < tops.length ? tops[i + 1] : top + editorLineHeight(editor);
  return top + (line - i) * (next - top);
}

interface ScrollAnchor { line: number; top: number; }
function previewAnchors(preview: HTMLElement): ScrollAnchor[] {
  const base = preview.getBoundingClientRect().top;
  const out: ScrollAnchor[] = [];
  preview.querySelectorAll<HTMLElement>("[data-source-line]").forEach((el) => {
    const line = Number(el.dataset.sourceLine);
    if (!Number.isFinite(line)) return;
    out.push({ line, top: el.getBoundingClientRect().top - base + preview.scrollTop });
  });
  return out; // DOM order ⇒ ascending by line
}

// Editor scrolled → align the preview to the same source line.
// Note: textarea line→pixel is approximate when long lines wrap; anchoring the
// preview by source line keeps it well aligned for typical Markdown documents.
function onEditorScroll() {
  if (syncing) return;
  const editor = splitEditorRef.value;
  const preview = splitPreviewRef.value;
  if (!editor || !preview) return;
  const topLine = lineAtScroll(editor, editor.scrollTop);
  const anchors = previewAnchors(preview);
  let targetTop: number;
  if (anchors.length < 2) {
    const er = editor.scrollHeight - editor.clientHeight;
    const pr = preview.scrollHeight - preview.clientHeight;
    targetTop = er > 0 ? (editor.scrollTop / er) * pr : 0;
  } else {
    let lo = anchors[0];
    let hi = anchors[anchors.length - 1];
    for (let i = 0; i < anchors.length; i++) {
      if (anchors[i].line <= topLine) lo = anchors[i];
      if (anchors[i].line >= topLine) { hi = anchors[i]; break; }
    }
    targetTop = hi.line === lo.line
      ? lo.top
      : lo.top + ((topLine - lo.line) / (hi.line - lo.line)) * (hi.top - lo.top);
  }
  syncing = true;
  preview.scrollTop = targetTop;
  requestAnimationFrame(() => { syncing = false; });
}

// Preview scrolled → align the editor to the topmost visible source line.
function onPreviewScroll() {
  if (syncing) return;
  const editor = splitEditorRef.value;
  const preview = splitPreviewRef.value;
  if (!editor || !preview) return;
  const anchors = previewAnchors(preview);
  let targetTop: number;
  if (anchors.length < 2) {
    const pr = preview.scrollHeight - preview.clientHeight;
    const er = editor.scrollHeight - editor.clientHeight;
    targetTop = pr > 0 ? (preview.scrollTop / pr) * er : 0;
  } else {
    let chosen = anchors[0];
    for (const a of anchors) {
      if (a.top <= preview.scrollTop + 1) chosen = a; else break;
    }
    const idx = anchors.indexOf(chosen);
    const next = anchors[idx + 1];
    let line = chosen.line;
    if (next && next.top > chosen.top) {
      line = chosen.line + ((preview.scrollTop - chosen.top) / (next.top - chosen.top)) * (next.line - chosen.line);
    }
    targetTop = scrollForLine(editor, line);
  }
  syncing = true;
  editor.scrollTop = targetTop;
  requestAnimationFrame(() => { syncing = false; });
}

// --- Status bar hint ---
const statusHint = computed(() => {
  const base = "Ctrl+S Save · Ctrl+O Open · F11 Fullscreen · ⎋ Close";
  return mode.value === "split"
    ? `Ctrl+click to sync panes · ${base}`
    : base;
});

// Ctrl+click on the editor pane → scroll the preview to the clicked source line.
function onEditorClick(e: MouseEvent) {
  if (!e.ctrlKey) return;
  const editor = splitEditorRef.value;
  const preview = splitPreviewRef.value;
  if (!editor || !preview) return;

  const rect = editor.getBoundingClientRect();
  const relY = e.clientY - rect.top + editor.scrollTop;
  const line = lineAtScroll(editor, relY);

  const anchors = previewAnchors(preview);
  let targetTop: number;
  if (anchors.length < 2) {
    const pr = preview.scrollHeight - preview.clientHeight;
    targetTop = editor.scrollHeight > 0
      ? Math.min((relY / editor.scrollHeight) * preview.scrollHeight, pr)
      : 0;
  } else {
    let lo = anchors[0];
    let hi = anchors[anchors.length - 1];
    for (let i = 0; i < anchors.length; i++) {
      if (anchors[i].line <= line) lo = anchors[i];
      if (anchors[i].line >= line) { hi = anchors[i]; break; }
    }
    targetTop = hi.line === lo.line
      ? lo.top
      : lo.top + ((line - lo.line) / (hi.line - lo.line)) * (hi.top - lo.top);
  }
  syncing = true;
  preview.scrollTop = Math.max(0, targetTop - preview.clientHeight / 3);
  requestAnimationFrame(() => { syncing = false; });
}

// --- Preview → editor selection sync ---

/** Walk up from a DOM node to the nearest [data-source-line] ancestor. */
function closestSourceLine(node: Node | null): number | null {
  let el: Element | null = node instanceof Element ? node : node?.parentElement ?? null;
  while (el) {
    const v = (el as HTMLElement).dataset?.sourceLine;
    if (v !== undefined) return Number(v);
    el = el.parentElement;
  }
  return null;
}

/** Map two source-line indices to a start/end char offset in source.value. */
function sourceRangeForLines(a: number, b: number): { start: number; end: number } {
  const lines = source.value.split("\n");
  const from = Math.max(0, Math.min(Math.min(a, b), lines.length - 1));
  const to   = Math.max(0, Math.min(Math.max(a, b), lines.length - 1));
  let start = 0;
  for (let i = 0; i < from; i++) start += lines[i].length + 1;
  let end = start;
  for (let i = from; i <= to; i++) end += lines[i].length + (i < to ? 1 : 0);
  return { start, end };
}

/**
 * Fired on mouseup inside a preview pane. Captures the browser selection and
 * maps it back to a source-line range, then:
 *  - In split mode: immediately mirrors the selection into the editor textarea.
 *  - Any mode: stores the range so the mode-switch watcher can apply it when
 *    the user clicks Edit or Split.
 */
function onPreviewMouseUp() {
  const sel = window.getSelection();
  if (!sel || sel.isCollapsed || sel.rangeCount === 0) return;
  const selRange = sel.getRangeAt(0);

  const startLine = closestSourceLine(selRange.startContainer);
  const endLine   = closestSourceLine(selRange.endContainer);
  if (startLine === null || endLine === null) return;

  const { start, end } = sourceRangeForLines(startLine, endLine);
  previewSelRange = { start, end };

  if (mode.value === "split" && splitEditorRef.value) {
    const editor = splitEditorRef.value;
    editor.setSelectionRange(start, end);
    editor.scrollTop = Math.max(
      0,
      scrollForLine(editor, Math.min(startLine, endLine)) - editor.clientHeight / 3,
    );
  }
}

// --- Link interception + click-to-locate ---
function onPreviewClick(e: MouseEvent) {
  const targetEl = e.target as HTMLElement;
  const a = targetEl.closest("a");
  if (a) {
    const href = a.getAttribute("href");
    if (href) {
      e.preventDefault();
      void openExternal(href);
    }
    return;
  }
  // Locate only in split mode, and only on Ctrl+click — plain click selects text.
  if (mode.value !== "split") return;
  if (!e.ctrlKey) return;
  const sel = window.getSelection();
  if (sel && !sel.isCollapsed) return;
  const el = targetEl.closest<HTMLElement>("[data-source-line]");
  if (!el) return;
  const line = Number(el.dataset.sourceLine);
  if (!Number.isFinite(line)) return;
  const editor = splitEditorRef.value;
  if (!editor) return;
  const lines = source.value.split("\n");
  const clamped = Math.max(0, Math.min(line, lines.length - 1));
  let start = 0;
  for (let i = 0; i < clamped; i++) start += lines[i].length + 1; // +1 for \n
  const end = start + lines[clamped].length;
  editor.focus();
  editor.setSelectionRange(start, end);
  editor.scrollTop = Math.max(0, scrollForLine(editor, clamped) - editor.clientHeight / 3);
}

// Watch mode: persist immediately, and carry any preview selection into the editor.
watch(mode, async (newMode) => {
  void fileDoc.persistCurrent();
  if (previewSelRange && (newMode === "edit" || newMode === "split")) {
    const saved = previewSelRange;
    previewSelRange = null;
    await nextTick();
    const editor = newMode === "edit" ? editRef.value : splitEditorRef.value;
    if (!editor) return;
    editor.focus();
    editor.setSelectionRange(saved.start, saved.end);
    // Scroll so the start of the selection is visible.
    const lines = source.value.split("\n");
    let chars = 0;
    let topLine = 0;
    for (let i = 0; i < lines.length; i++) {
      if (chars >= saved.start) { topLine = i; break; }
      chars += lines[i].length + 1;
    }
    editor.scrollTop = Math.max(0, scrollForLine(editor, topLine) - editor.clientHeight / 3);
  }
});

// --- Find in document (Ctrl+F) ---
const mdSearchQuery = ref("");
const mdMatchIndex = ref(0);

function findMatches(text: string, query: string): number[] {
  const positions: number[] = [];
  if (!query) return positions;
  const lowerText = text.toLowerCase();
  const lowerQuery = query.toLowerCase();
  let idx = lowerText.indexOf(lowerQuery);
  while (idx !== -1) {
    positions.push(idx);
    idx = lowerText.indexOf(lowerQuery, idx + 1);
  }
  return positions;
}

const mdMatches = computed(() => findMatches(source.value, mdSearchQuery.value));

async function jumpToMatch(idx: number) {
  if (mode.value === "preview") mode.value = "split"; // keeps rendered context rather than bare "edit"
  await nextTick();
  const editor = mode.value === "edit" ? editRef.value : splitEditorRef.value;
  const m = mdMatches.value[idx];
  if (!editor || m === undefined) return;
  editor.focus();
  editor.setSelectionRange(m, m + mdSearchQuery.value.length);
  const line = source.value.slice(0, m).split("\n").length - 1;
  editor.scrollTop = Math.max(0, scrollForLine(editor, line) - editor.clientHeight / 3);
  // scrollTop change fires the existing @scroll="onEditorScroll" listener (split mode) —
  // preview pane follows via the already-existing scroll-sync machinery, no new code.
}

function nextMatch() {
  if (!mdMatches.value.length) return;
  mdMatchIndex.value = (mdMatchIndex.value + 1) % mdMatches.value.length;
  void jumpToMatch(mdMatchIndex.value);
}

function prevMatch() {
  if (!mdMatches.value.length) return;
  mdMatchIndex.value = (mdMatchIndex.value - 1 + mdMatches.value.length) % mdMatches.value.length;
  void jumpToMatch(mdMatchIndex.value);
}

watch(mdSearchQuery, (q) => {
  mdMatchIndex.value = 0;
  if (q && mdMatches.value.length) void jumpToMatch(0);
});

const { isOpen: searchOpen, close: closeSearchRaw, toggle: toggleSearch } = useRevealSearch();
function closeSearch() {
  mdSearchQuery.value = "";
  closeSearchRaw();
}

// --- Title bar controls ---
// Shared window chrome (pin/minimize/maximize, geometry persistence,
// Escape/F11/Ctrl+F, title-watch, focus/close Tauri-event wiring). Markdown
// overrides the close request (dirty-check + modal, via fileDoc) and adds an
// external-change recheck on focus gain.
const { pinned, isMaximized, togglePin, minimize, toggleMaximize } = useViewerWindowChrome({
  title: windowTitle,
  onEscapeClose: fileDoc.closeWindow,
  onNativeCloseRequest: fileDoc.onNativeCloseRequest,
  onFocusGained: () => void fileDoc.checkExternalChange(),
  onToggleSearch: toggleSearch,
});

onMounted(async () => {
  await fileDoc.hydrate();

  // No content (new window, or a restored/empty doc) → start in edit mode so
  // the user can type immediately rather than facing an empty preview.
  if (!source.value.trim()) {
    mode.value = "edit";
  }
});

onUnmounted(() => {
  if (mirrorEl) { mirrorEl.remove(); mirrorEl = null; cachedTops = null; }
});
</script>

<template>
  <div class="md-viewer-app">
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
    <ViewerToolbarRow1 v-model:layout-mode="mode">
      <template #file-actions>
        <button class="util-btn" @click="fileDoc.openFile" title="Open a Markdown file (Ctrl+O)">📂 Open</button>
        <button class="util-btn" @click="fileDoc.save" title="Save (Ctrl+S)">💾 Save</button>
        <button class="util-btn" @click="fileDoc.saveAs" title="Save As… (Ctrl+Shift+S)">Save As…</button>
      </template>
      <template #utility-actions>
        <button class="util-btn" :class="{ active: searchOpen }" @click="toggleSearch" title="Find in document (Ctrl+F)">🔍 Find</button>
        <button class="util-btn" @click="source = ''; mode = 'edit'" title="Clear content">🗑 Clear</button>
        <button class="util-btn" @click="() => openNewFileViewerWindow({ kind: 'markdown' })" title="Open a new Markdown Viewer window">＋ New</button>
      </template>
    </ViewerToolbarRow1>

    <!-- Find-in-document search bar (Ctrl+F reveal) -->
    <ViewerSearchBar
      v-if="searchOpen"
      v-model="mdSearchQuery"
      placeholder="Find in document..."
      :match-count="mdMatches.length"
      :match-index="mdMatches.length ? mdMatchIndex + 1 : 0"
      @close="closeSearch"
      @next="nextMatch"
      @prev="prevMatch"
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

    <!-- Content area -->
    <div class="content">
      <div v-if="mode === 'preview'" class="preview-pane" @mouseup="onPreviewMouseUp">
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
          ref="editRef"
          v-model="source"
          placeholder="Type or paste Markdown…"
          class="md-editor"
        />
      </div>
      <SplitPane v-else storageKey="fnba-utils:md-split" :default-ratio="0.5">
        <template #left>
          <div class="edit-pane">
            <textarea
              ref="splitEditorRef"
              v-model="source"
              placeholder="Type or paste Markdown…"
              class="md-editor"
              @scroll="onEditorScroll"
              @click="onEditorClick"
            />
          </div>
        </template>
        <template #right>
          <div ref="splitPreviewRef" class="preview-pane" @scroll="onPreviewScroll" @mouseup="onPreviewMouseUp">
            <div v-if="source.trim()" class="md-body" v-html="renderedHtml" @click="onPreviewClick" />
            <div v-else class="placeholder">Start typing on the left…</div>
          </div>
        </template>
      </SplitPane>
    </div>

    <!-- Status bar -->
    <StatusBar :hint="statusHint" />

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
.md-viewer-app {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: #1e1e1e;
  color: #e0e0e0;
  font-family: "Segoe UI", "Inter", sans-serif;
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
  user-select: text;
  -webkit-user-select: text;
  cursor: text;
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
