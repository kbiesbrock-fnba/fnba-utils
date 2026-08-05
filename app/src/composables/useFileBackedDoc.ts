// Shared file-backed document plumbing for File Viewer bodies (JSON,
// Markdown): disk-backed doc-cache persistence (docPath), dirty-tracking,
// external-change-on-disk detection, unsaved-changes close-prompt, and
// Open/Save/Save-As file I/O. Internals mirror Markdown Viewer's original,
// hand-rolled implementation exactly (just scoped inside a composable instead
// of living in the .vue file) — this is a lift-and-share, not a rewrite.
//
// `hydrate()` is explicitly awaited by the host's own `onMounted` rather than
// self-invoked internally: Vue doesn't serialize async `onMounted` hooks
// across composables, and a host step like Markdown's "empty doc → force
// edit mode" depends on hydration having already finished.
import { ref, watch, onMounted, onUnmounted, type Ref } from "vue";
import type { CloseRequestedEvent } from "@tauri-apps/api/window";
import {
  touchEntry,
  removeEntry,
  saveState,
  readRegistry,
  type ViewerKind,
} from "../lib/fileViewerRegistry";
import {
  writeViewerDoc,
  readViewerDoc,
  deleteViewerDoc,
  openViewerFile,
  saveViewerFileAs,
  saveViewerFile,
  statViewerFile,
  readViewerFile,
} from "../lib/tauri";
import { openNewFileViewerWindow } from "../lib/fileViewerWindow";
import { baseName } from "../lib/pathUtils";

export interface FileBackedDocOptions {
  kind: ViewerKind;
  /** The live buffer (source / input). */
  content: Ref<string>;
  /** Save-As default filename (incl. extension) for an UNBOUND doc — the
   *  composable itself handles the bound case (suggests the current
   *  filename) before ever calling this. */
  suggestedName: (content: string) => string;
  /** Kind-specific fields folded into the same saveState write. */
  extraState?: () => Record<string, unknown>;
  /** Mirror of extraState: restore those fields on hydrate. */
  hydrateExtra?: (state: Record<string, unknown>) => void;
}

type PersistedState = Record<string, unknown> & {
  docPath?: string | null;
  filePath?: string | null;
  dirty?: boolean;
  diskMtimeMs?: number | null;
  diskSize?: number | null;
};

export function useFileBackedDoc(options: FileBackedDocOptions) {
  const { kind, content } = options;

  let currentDocPath: string | null = null;

  const filePath = ref<string | null>(null);
  const dirty = ref(false);
  const showCloseModal = ref(false);
  let forceClose = false; // set right before a programmatic close so onNativeCloseRequest allows it

  // --- External-change detection ---
  let diskBaseline: { mtimeMs: number; size: number } | null = null;
  const externalChange = ref<null | "changed" | "deleted">(null);

  // --- Persistence ---
  let persistDocTimer: ReturnType<typeof setTimeout> | null = null;

  /** Write the full viewer state (docPath + kind-specific extras + filePath + dirty + disk baseline) immediately. */
  function persistState(label: string): void {
    saveState(label, {
      ...(options.extraState?.() ?? {}),
      docPath: currentDocPath,
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

  function schedulePersistDocForLabel(label: string): void {
    if (persistDocTimer) clearTimeout(persistDocTimer);
    persistDocTimer = setTimeout(() => {
      persistDocTimer = null;
      void (async () => {
        try {
          currentDocPath = await writeViewerDoc(label, kind, content.value);
          persistState(label);
        } catch {
          // ignore — file I/O is best-effort
        }
      })();
    }, 400);
  }

  /** Debounced (400ms): re-write the doc-cache file and persist state. */
  function schedulePersist(): void {
    void (async () => {
      try {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        schedulePersistDocForLabel(getCurrentWindow().label);
      } catch {
        // ignore
      }
    })();
  }

  // Watch content: update registry preview, touch entry, debounce doc
  // persist. Mark dirty for bound docs when content changes.
  watch(content, (val) => {
    void (async () => {
      try {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        const label = getCurrentWindow().label;
        const preview = val.replace(/\s+/g, " ").trim().slice(0, 60);
        touchEntry(label, preview);
        schedulePersistDocForLabel(label);
        if (filePath.value) dirty.value = true;
      } catch {
        // ignore
      }
    })();
  });

  // --- External-change detection helpers ---

  /** Snapshot the current disk fingerprint so our own writes never self-trigger the banner. */
  async function refreshBaseline(): Promise<void> {
    if (!filePath.value) { diskBaseline = null; return; }
    try {
      const s = await statViewerFile(filePath.value);
      diskBaseline = s.exists ? { mtimeMs: s.mtimeMs, size: s.size } : null;
      await persistCurrent();
    } catch { /* best-effort */ }
  }

  /** Check whether the on-disk file differs from our baseline. Sets externalChange. */
  async function checkExternalChange(): Promise<void> {
    if (!filePath.value) { externalChange.value = null; return; }
    try {
      const s = await statViewerFile(filePath.value);
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
      const f = await readViewerFile(filePath.value);
      content.value = f.content;
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
      const f = await readViewerFile(filePath.value);
      await openNewFileViewerWindow({ kind, content: f.content });
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

  async function saveAs(): Promise<boolean> {
    const name = filePath.value ? baseName(filePath.value) : options.suggestedName(content.value);
    const p = await saveViewerFileAs(kind, content.value, name);
    if (!p) return false;
    filePath.value = p;
    dirty.value = false;
    await refreshBaseline();
    return true;
  }

  async function save(): Promise<boolean> {
    if (!filePath.value) return saveAs();
    try {
      await saveViewerFile(filePath.value, content.value);
      dirty.value = false;
      await refreshBaseline();
      return true;
    } catch (e) {
      console.error("save failed", e);
      return false;
    }
  }

  async function openFile(): Promise<void> {
    const f = await openViewerFile(kind);
    if (f) await openNewFileViewerWindow({ kind, content: f.content, filePath: f.path });
  }

  // --- Close ---

  function needsSavePrompt(): boolean {
    if (filePath.value) return dirty.value;
    return content.value.trim() !== "";
  }

  async function doClose(): Promise<void> {
    forceClose = true;
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const w = getCurrentWindow();
    try {
      if (currentDocPath) await deleteViewerDoc(currentDocPath);
    } catch {
      // ignore
    }
    removeEntry(w.label);
    await w.close();
  }

  function closeWindow(): void {
    if (needsSavePrompt()) { showCloseModal.value = true; return; }
    void doClose();
  }

  async function onModalSave(): Promise<void> {
    const ok = await save();
    showCloseModal.value = false;
    if (ok) await doClose();
    // if saveAs was cancelled (ok=false), stay open
  }

  function onModalDiscard(): void { showCloseModal.value = false; void doClose(); }
  function onModalCancel(): void { showCloseModal.value = false; }

  // --- OS-level close (Alt+F4 / taskbar ✕) ---
  async function onNativeCloseRequest(event: CloseRequestedEvent, label: string): Promise<void> {
    if (forceClose) return; // our own doClose() — allow
    if (needsSavePrompt()) {
      event.preventDefault();
      showCloseModal.value = true;
    } else {
      if (currentDocPath) void deleteViewerDoc(currentDocPath).catch(() => {});
      removeEntry(label);
    }
  }

  // --- Hydration ---

  /**
   * Restore docPath/content, filePath, dirty, disk baseline, and (via
   * `hydrateExtra`) kind-specific fields from this window's registry entry.
   * A registry entry with no `docPath` (a brand-new, never-persisted window)
   * is a no-op — the caller's own `content` ref keeps its initial value.
   * Explicitly awaited by the host — NOT self-invoked here.
   */
  async function hydrate(): Promise<void> {
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      const label = getCurrentWindow().label;
      const registry = readRegistry();
      const entry = registry[label];
      const state = entry?.state as PersistedState | undefined;
      if (state?.docPath) {
        currentDocPath = state.docPath;
        content.value = await readViewerDoc(currentDocPath);
        filePath.value = state.filePath ?? null;
        dirty.value = state.dirty ?? false;
        const dm = state.diskMtimeMs, ds = state.diskSize;
        diskBaseline = (typeof dm === "number" && typeof ds === "number") ? { mtimeMs: dm, size: ds } : null;
        options.hydrateExtra?.(state);
        // If bound, check immediately for changes that happened while the app was closed.
        if (filePath.value) void checkExternalChange();
      }
    } catch {
      // ignore — hydration is best-effort
    }

    // For a new bound window (opened via openFile()) where no baseline was
    // persisted yet, establish the baseline now.
    if (filePath.value && diskBaseline === null) void refreshBaseline();
  }

  // --- Ctrl+S / Ctrl+Shift+S / Ctrl+O — fires regardless of focus (no
  // editable-field guard), copied verbatim from Markdown's original
  // behavior. Net-new capability for JSON, zero behavior change for Markdown.
  function onKeydown(e: KeyboardEvent): void {
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

  onMounted(() => {
    window.addEventListener("keydown", onKeydown);
  });

  onUnmounted(() => {
    window.removeEventListener("keydown", onKeydown);
    if (persistDocTimer) {
      clearTimeout(persistDocTimer);
      persistDocTimer = null;
    }
    // NOTE: deleteViewerDoc / removeEntry are intentionally NOT called here.
    // onUnmounted fires on every Vite HMR reload while the window stays open —
    // calling them here would destroy the doc for a window the user never closed.
    // Process death never runs onUnmounted at all. Intentional closes are handled
    // by closeWindow() (Esc / ✕) and onNativeCloseRequest (Alt+F4 / taskbar).
  });

  return {
    filePath,
    dirty,
    showCloseModal,
    externalChange,
    hydrate,
    persistCurrent,
    schedulePersist,
    checkExternalChange,
    reloadFromDisk,
    keepMine,
    openDiskCopy,
    saveOverDeleted,
    dismissExternal,
    save,
    saveAs,
    openFile,
    needsSavePrompt,
    doClose,
    closeWindow,
    onModalSave,
    onModalDiscard,
    onModalCancel,
    onNativeCloseRequest,
  };
}
