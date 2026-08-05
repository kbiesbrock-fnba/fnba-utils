<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onUnmounted } from "vue";
import type { CloseRequestedEvent } from "@tauri-apps/api/window";
import { renderMarkdown } from "../../lib/markdown";
import { touchEntry, removeEntry, saveState, readRegistry, type MarkdownViewerState } from "../../lib/fileViewerRegistry";
import { readMarkdownDoc, writeMarkdownDoc, deleteMarkdownDoc, openMarkdownFile, saveMarkdownAs, saveMarkdownFile, statMarkdownFile, readMarkdownFile } from "../../lib/tauri";
import { openNewFileViewerWindow } from "../../lib/fileViewerWindow";
import { openExternal } from "../../lib/external";
import { useViewerWindowChrome } from "../../composables/useViewerWindowChrome";
import ViewerTitleBar from "../file-viewer/ViewerTitleBar.vue";
import SplitPane from "../common/SplitPane.vue";
import StatusBar from "../StatusBar.vue";

const source = ref("");
// Default to edit: a brand-new/empty window should drop straight into typing.
// Hydration below flips to the saved mode (usually preview) when there's content.
const mode = ref<"preview" | "edit" | "split">("edit");
let currentDocPath: string | null = null;

// --- File-backed editor state ---
const filePath = ref<string | null>(null);
const dirty = ref(false);
const showCloseModal = ref(false);
let forceClose = false; // set right before a programmatic close so onNativeCloseRequest allows it

// --- External-change detection ---
let diskBaseline: { mtimeMs: number; size: number } | null = null;
const externalChange = ref<null | "changed" | "deleted">(null);

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

// --- Window title ---
// For bound files: show filename + dirty indicator.
// For unbound (scratch): shows the first heading or first non-empty line, else "Markdown Viewer".
function baseName(p: string): string {
  return p.split(/[\\/]/).pop() || p;
}

const windowTitle = computed(() => {
  if (filePath.value) {
    return baseName(filePath.value) + (dirty.value ? " ●" : "");
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

// --- Persistence ---
let persistDocTimer: ReturnType<typeof setTimeout> | null = null;

/** Write the full viewer state (docPath + mode + filePath + dirty + disk baseline) immediately. */
function persistState(label: string): void {
  saveState(label, {
    docPath: currentDocPath,
    mode: mode.value,
    filePath: filePath.value,
    dirty: dirty.value,
    diskMtimeMs: diskBaseline?.mtimeMs ?? null,
    diskSize: diskBaseline?.size ?? null,
  });
}

/** Persist state immediately using the current window label. */
async function persistCurrent(): Promise<void> {
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    persistState(getCurrentWindow().label);
  } catch {
    // ignore
  }
}

function schedulePersistDoc(label: string): void {
  if (persistDocTimer) clearTimeout(persistDocTimer);
  persistDocTimer = setTimeout(() => {
    persistDocTimer = null;
    void (async () => {
      try {
        currentDocPath = await writeMarkdownDoc(label, source.value);
        persistState(label);
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
      // Mark dirty for bound docs when content changes.
      if (filePath.value) dirty.value = true;
    } catch {
      // ignore
    }
  })();
});

// Watch mode: persist immediately, and carry any preview selection into the editor.
watch(mode, async (newMode) => {
  void persistCurrent();
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

// --- External-change detection helpers ---

/** Snapshot the current disk fingerprint so our own writes never self-trigger the banner. */
async function refreshBaseline(): Promise<void> {
  if (!filePath.value) { diskBaseline = null; return; }
  try {
    const s = await statMarkdownFile(filePath.value);
    diskBaseline = s.exists ? { mtimeMs: s.mtimeMs, size: s.size } : null;
    await persistCurrent();
  } catch { /* best-effort */ }
}

/** Check whether the on-disk file differs from our baseline. Sets externalChange. */
async function checkExternalChange(): Promise<void> {
  if (!filePath.value) { externalChange.value = null; return; }
  try {
    const s = await statMarkdownFile(filePath.value);
    if (!s.exists) {
      externalChange.value = "deleted";
      return;
    }
    if (diskBaseline && (s.mtimeMs !== diskBaseline.mtimeMs || s.size !== diskBaseline.size)) {
      externalChange.value = "changed";
    } else {
      externalChange.value = null;
    }
  } catch { /* ignore */ }
}

/** Banner action: discard local edits and reload from disk. */
async function reloadFromDisk(): Promise<void> {
  if (!filePath.value) return;
  try {
    const f = await readMarkdownFile(filePath.value);
    source.value = f.content;
    dirty.value = false;
    diskBaseline = { mtimeMs: f.mtimeMs, size: f.size };
    externalChange.value = null;
    await persistCurrent();
  } catch (e) { console.error("reload failed", e); }
}

/** Banner action: dismiss; re-baseline to stop nagging about THIS change. */
async function keepMine(): Promise<void> {
  externalChange.value = null;
  await refreshBaseline();
}

/** Banner action: open the on-disk version in a separate unbound scratch window. */
async function openDiskCopy(): Promise<void> {
  if (!filePath.value) return;
  try {
    const f = await readMarkdownFile(filePath.value);
    await openNewFileViewerWindow({ kind: "markdown", content: f.content });
  } catch (e) { console.error(e); }
  await keepMine();
}

/** Banner action: write the current content back to a file that was deleted on disk. */
async function saveOverDeleted(): Promise<void> {
  const ok = await save();
  if (ok) { externalChange.value = null; }
}

/** Banner action: dismiss without re-baselining (deleted-file case). */
function dismissExternal(): void { externalChange.value = null; }

// --- File I/O ---

function suggestedName(): string {
  if (filePath.value) return baseName(filePath.value);
  const first = source.value.split("\n").find((l) => l.trim()) ?? "";
  const slug = first
    .replace(/^#+\s*/, "")
    .trim()
    .slice(0, 40)
    .replace(/[^\w.-]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return (slug || "untitled") + ".md";
}

async function saveAs(): Promise<boolean> {
  const p = await saveMarkdownAs(source.value, suggestedName());
  if (!p) return false;
  filePath.value = p;
  dirty.value = false;
  await refreshBaseline();
  return true;
}

async function save(): Promise<boolean> {
  if (!filePath.value) return saveAs();
  try {
    await saveMarkdownFile(filePath.value, source.value);
    dirty.value = false;
    await refreshBaseline();
    return true;
  } catch (e) {
    console.error("save failed", e);
    return false;
  }
}

async function openFile() {
  const f = await openMarkdownFile();
  if (f) await openNewFileViewerWindow({ kind: "markdown", content: f.content, filePath: f.path });
}

// --- Close ---

function needsSavePrompt(): boolean {
  if (filePath.value) return dirty.value;
  return source.value.trim() !== "";
}

async function doClose() {
  forceClose = true;
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

function closeWindow() {
  if (needsSavePrompt()) { showCloseModal.value = true; return; }
  void doClose();
}

async function onModalSave() {
  const ok = await save();
  showCloseModal.value = false;
  if (ok) await doClose();
  // if saveAs was cancelled (ok=false), stay open
}

function onModalDiscard() { showCloseModal.value = false; void doClose(); }
function onModalCancel() { showCloseModal.value = false; }

// --- OS-level close (Alt+F4 / taskbar ✕) ---
// Copied verbatim from the pre-unification onCloseRequested body — this is the
// one place JSON and Markdown genuinely differ (see useViewerWindowChrome.ts),
// so it's supplied as an override rather than folded into shared chrome.
// `label` is threaded in as a parameter (the composable owns the mount-time
// closure that used to carry it) rather than changed in substance.
async function onNativeCloseRequest(event: CloseRequestedEvent, label: string) {
  if (forceClose) return; // our own doClose() — allow
  if (needsSavePrompt()) {
    event.preventDefault();
    showCloseModal.value = true;
  } else {
    if (currentDocPath) void deleteMarkdownDoc(currentDocPath).catch(() => {});
    removeEntry(label);
  }
}

// --- Title bar controls ---
// Shared window chrome (pin/minimize/maximize, geometry persistence,
// Escape/F11, title-watch, focus/close Tauri-event wiring). Markdown overrides
// the close request (dirty-check + modal) and adds an external-change recheck
// on focus gain.
const { pinned, isMaximized, togglePin, minimize, toggleMaximize } = useViewerWindowChrome({
  title: windowTitle,
  onEscapeClose: closeWindow,
  onNativeCloseRequest,
  onFocusGained: () => void checkExternalChange(),
});

function onKeydown(e: KeyboardEvent) {
  // Save / Save As / Open — handled globally regardless of focus.
  if (e.ctrlKey && (e.key === "s" || e.key === "S")) {
    e.preventDefault();
    if (e.shiftKey) void saveAs(); else void save();
    return;
  }
  if (e.ctrlKey && (e.key === "o" || e.key === "O")) {
    e.preventDefault();
    void openFile();
    return;
  }
}

onMounted(async () => {
  window.addEventListener("keydown", onKeydown);

  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const label = getCurrentWindow().label;

  // --- Hydrate from saved state ---
  // (A restored window reuses its old label, so the registry entry is present.
  //  A brand-new window seeded via openNewFileViewerWindow also has a
  //  pre-seeded registry entry — no pending-localStorage handoff needed.)
  try {
    const registry = readRegistry();
    const entry = registry[label];
    const state = entry?.state as MarkdownViewerState | undefined;
    if (state?.docPath) {
      currentDocPath = state.docPath;
      source.value = await readMarkdownDoc(currentDocPath);
      mode.value = (state.mode as "preview" | "edit" | "split") ?? "preview";
      filePath.value = state.filePath ?? null;
      dirty.value = state.dirty ?? false;
      // Hydrate disk baseline so external-change detection works across restarts.
      const dm = state.diskMtimeMs, ds = state.diskSize;
      diskBaseline = (typeof dm === "number" && typeof ds === "number") ? { mtimeMs: dm, size: ds } : null;
      // If bound, check immediately for changes that happened while the app was closed.
      if (filePath.value) void checkExternalChange();
    }
  } catch {
    // ignore — hydration is best-effort
  }

  // For a new bound window (opened via openFile()) where no baseline was
  // persisted yet, establish the baseline now.
  if (filePath.value && diskBaseline === null) void refreshBaseline();

  // No content (new window, or a restored/empty doc) → start in edit mode so
  // the user can type immediately rather than facing an empty preview.
  if (!source.value.trim()) {
    mode.value = "edit";
  }
});

onUnmounted(() => {
  window.removeEventListener("keydown", onKeydown);
  if (persistDocTimer) {
    clearTimeout(persistDocTimer);
    persistDocTimer = null;
  }
  if (mirrorEl) { mirrorEl.remove(); mirrorEl = null; cachedTops = null; }
  // NOTE: deleteMarkdownDoc / removeEntry are intentionally NOT called here.
  // onUnmounted fires on every Vite HMR reload while the window stays open —
  // calling them here would destroy the doc for a window the user never closed.
  // Process death never runs onUnmounted at all. Intentional closes are handled
  // by closeWindow() (Esc / ✕) and onNativeCloseRequest (Alt+F4 / taskbar).
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
      @close="closeWindow"
    />

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
          @click="openFile"
          title="Open a Markdown file (Ctrl+O)"
        >
          📂 Open
        </button>
        <button
          class="util-btn"
          @click="save"
          title="Save (Ctrl+S)"
        >
          💾 Save
        </button>
        <button
          class="util-btn"
          @click="saveAs"
          title="Save As… (Ctrl+Shift+S)"
        >
          Save As…
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
          @click="() => openNewFileViewerWindow({ kind: 'markdown' })"
          title="Open a new Markdown Viewer window"
        >
          ＋ New
        </button>
      </div>
    </div>

    <!-- External-change banners (between toolbar and content; push content down) -->
    <div v-if="externalChange === 'changed'" class="ext-banner">
      <span class="ext-msg">⚠ This file changed on disk{{ dirty ? " — you have unsaved edits" : "" }}.</span>
      <span class="ext-actions">
        <button class="ext-btn ext-primary" @click="reloadFromDisk">{{ dirty ? "Reload (discard mine)" : "Reload" }}</button>
        <button v-if="dirty" class="ext-btn" @click="openDiskCopy">Open disk copy ↗</button>
        <button class="ext-btn" @click="keepMine">Keep mine</button>
      </span>
    </div>
    <div v-else-if="externalChange === 'deleted'" class="ext-banner ext-deleted">
      <span class="ext-msg">⚠ This file was deleted on disk.</span>
      <span class="ext-actions">
        <button class="ext-btn ext-primary" @click="saveOverDeleted">Save again</button>
        <button class="ext-btn" @click="dismissExternal">Dismiss</button>
      </span>
    </div>

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
    <div v-if="showCloseModal" class="close-modal-backdrop">
      <div class="close-modal">
        <div class="cm-title">Save changes?</div>
        <div class="cm-msg">{{ filePath ? baseName(filePath) : "This document" }} has unsaved changes.</div>
        <div class="cm-actions">
          <button class="cm-btn cm-primary" @click="onModalSave">Save</button>
          <button class="cm-btn" @click="onModalDiscard">Don't save</button>
          <button class="cm-btn" @click="onModalCancel">Cancel</button>
        </div>
      </div>
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

/* --- Save-on-close modal --- */
.close-modal-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.close-modal {
  background: #2d2d2d;
  border: 1px solid #555;
  border-radius: 8px;
  padding: 24px;
  width: 320px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.cm-title {
  font-size: 15px;
  font-weight: 600;
  color: #f0f0f0;
}

.cm-msg {
  font-size: 13px;
  color: #aaa;
  word-break: break-all;
}

.cm-actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
  margin-top: 4px;
}

.cm-btn {
  padding: 6px 14px;
  background: #404040;
  border: 1px solid #555;
  color: #ddd;
  border-radius: 4px;
  cursor: pointer;
  font-size: 13px;
  transition: background 0.12s;
}

.cm-btn:hover {
  background: #505050;
}

.cm-btn.cm-primary {
  background: #4CAF50;
  border-color: #45a049;
  color: white;
}

.cm-btn.cm-primary:hover {
  background: #45a049;
}

/* --- External-change banner --- */
.ext-banner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 14px;
  background: #3a2f1a;
  border-bottom: 1px solid #5c4a1f;
  color: #e0c66a;
  font-size: 12px;
  flex-shrink: 0;
  gap: 12px;
}

.ext-banner.ext-deleted {
  background: #3a1e1e;
  border-bottom-color: #5c2a2a;
  color: #e08080;
}

.ext-msg {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ext-actions {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
}

.ext-btn {
  padding: 4px 10px;
  background: #4a3a20;
  border: 1px solid #7a6030;
  color: #e0c66a;
  border-radius: 4px;
  cursor: pointer;
  font-size: 11px;
  transition: background 0.12s, color 0.12s;
  white-space: nowrap;
}

.ext-btn:hover {
  background: #5a4a28;
  color: #f0d87a;
}

.ext-banner.ext-deleted .ext-btn {
  background: #4a2020;
  border-color: #7a3030;
  color: #e08080;
}

.ext-banner.ext-deleted .ext-btn:hover {
  background: #5a2828;
  color: #f09090;
}

.ext-btn.ext-primary {
  background: #4CAF50;
  border-color: #45a049;
  color: white;
}

.ext-btn.ext-primary:hover {
  background: #45a049;
  color: white;
}
</style>
