// Shared window-chrome plumbing for File Viewer windows: pin/minimize/
// maximize, geometry persistence, Escape/F11 handling, the title-watch →
// setTitle wiring, and the focus/close Tauri-event wiring. Extracted from the
// byte-identical boilerplate that used to live separately in
// JsonViewerApp.vue and MarkdownViewerApp.vue.
//
// Close semantics are NOT identical between the two viewers — JSON's close is
// unconditional, Markdown's is gated behind an unsaved-changes check that can
// pop a save-prompt modal and block the OS-level close via
// `event.preventDefault()`. So close behavior is a caller-supplied override
// (`onNativeCloseRequest`) rather than something this composable decides on
// its own; the default (no override) is JSON's current unconditional
// behavior. Likewise `onEscapeClose` is always caller-supplied — each body
// keeps its own `closeWindow()`.

import { ref, watch, onMounted, onUnmounted, type Ref, type ComputedRef } from "vue";
import type { CloseRequestedEvent } from "@tauri-apps/api/window";
import { touchEntry, saveWin, removeEntry, readRegistry } from "../lib/fileViewerRegistry";

export interface ViewerWindowChromeOptions {
  /** Reactive window title; the composable keeps the OS title bar in sync with it. */
  title: Ref<string> | ComputedRef<string>;
  /** Esc key (outside an editable field) — each body supplies its own close logic. */
  onEscapeClose: () => void;
  /**
   * OS-level close request (Alt+F4 / taskbar ✕). Receives the raw Tauri event
   * (call `event.preventDefault()` to block the close) and the window's
   * label. Default (no override): unconditional `removeEntry(label)` —
   * today's JSON Viewer behavior. Markdown overrides this with its
   * dirty-check + modal, copied verbatim from its current onCloseRequested body.
   */
  onNativeCloseRequest?: (event: CloseRequestedEvent, label: string) => void | Promise<void>;
  /** Extra work to run after touchEntry(label) on OS-level focus gain. */
  onFocusGained?: () => void;
}

export function useViewerWindowChrome(options: ViewerWindowChromeOptions) {
  const pinned = ref(false);
  const isMaximized = ref(false);

  let unlistenResize: (() => void) | null = null;
  let unlistenMove: (() => void) | null = null;
  let unlistenFocus: (() => void) | null = null;
  let unlistenCloseRequested: (() => void) | null = null;

  // Debounce timer for geometry persistence.
  let persistGeoTimer: ReturnType<typeof setTimeout> | null = null;

  // Last known un-maximized geometry so that restoring after maximize lands
  // on the previous footprint rather than the default size.
  let lastUnmaximizedGeo: { x: number; y: number; width: number; height: number } | null = null;

  /** Debounced: persist window geometry/pin/maximized state to localStorage. */
  function schedulePersistGeo(
    label: string,
    win: Awaited<ReturnType<typeof import("@tauri-apps/api/window")["getCurrentWindow"]>>,
  ): void {
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
      options.onEscapeClose();
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

  watch(options.title, async (title) => {
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().setTitle(title);
    } catch {
      // non-critical
    }
  });

  onMounted(async () => {
    window.addEventListener("keydown", onKeydown);

    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const w = getCurrentWindow();
    const label = w.label;

    // --- Hydrate pin state + last-known un-maximized geometry from the registry ---
    try {
      const entry = readRegistry()[label];
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

    // Register in MRU registry on open.
    touchEntry(label);

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
      if (focused) {
        touchEntry(label);
        options.onFocusGained?.();
      }
    });

    // Intentional close via Alt+F4 or taskbar close.
    unlistenCloseRequested = await w.onCloseRequested(async (event) => {
      if (options.onNativeCloseRequest) {
        await options.onNativeCloseRequest(event, label);
      } else {
        // Default (JSON Viewer today): unconditional — always allow the close.
        removeEntry(label);
      }
    });

    // Seed geometry after mount so a first-launch window persists position before
    // the user has moved or resized it.
    schedulePersistGeo(label, w);
  });

  onUnmounted(() => {
    window.removeEventListener("keydown", onKeydown);
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
  });

  return {
    pinned,
    isMaximized,
    togglePin,
    minimize,
    toggleMaximize,
  };
}
