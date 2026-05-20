import { ref } from "vue";
import {
  startNewClaudeSession,
  pickDirectory,
  isTauri,
  type NewSessionInfo,
} from "@/lib/tauri";

const RECENTS_KEY = "fnba-utils:mc-recent-projects";
const RECENTS_LIMIT = 10;

export type LaunchStep = "form" | "launching" | "done" | "error";

function readRecents(): string[] {
  try {
    const raw = window.localStorage.getItem(RECENTS_KEY);
    if (!raw) return [];
    const arr = JSON.parse(raw);
    return Array.isArray(arr) ? arr.filter((s): s is string => typeof s === "string") : [];
  } catch {
    return [];
  }
}

function writeRecents(list: string[]) {
  try {
    window.localStorage.setItem(RECENTS_KEY, JSON.stringify(list.slice(0, RECENTS_LIMIT)));
  } catch {
    // ignore storage errors
  }
}

function bumpRecent(cwd: string) {
  const next = [cwd, ...readRecents().filter((p) => p !== cwd)].slice(0, RECENTS_LIMIT);
  writeRecents(next);
}

export function useNewClaudeSession() {
  const step = ref<LaunchStep>("form");
  const cwd = ref<string>("");
  const initialPrompt = ref<string>("");
  const worktree = ref<boolean>(false);
  const error = ref<string | null>(null);
  const result = ref<NewSessionInfo | null>(null);
  const recents = ref<string[]>(readRecents());

  function reset() {
    step.value = "form";
    cwd.value = "";
    initialPrompt.value = "";
    worktree.value = false;
    error.value = null;
    result.value = null;
    recents.value = readRecents();
  }

  async function browse() {
    try {
      const picked = await pickDirectory();
      if (picked) cwd.value = picked;
    } catch (e) {
      // Picker failures shouldn't block; surface inline only if persistent.
      console.warn("[new-session] pickDirectory failed", e);
    } finally {
      // The native picker steals focus on Windows, which can demote our
      // alwaysOnTop frameless palette window behind the picker (the palette
      // appears to "hide"). Force the palette back to front + focused after
      // the picker resolves or is cancelled.
      if (isTauri) {
        try {
          const { getCurrentWindow } = await import("@tauri-apps/api/window");
          const w = getCurrentWindow();
          await w.show();
          await w.setFocus();
        } catch (e) {
          console.warn("[new-session] palette refocus failed", e);
        }
      }
    }
  }

  async function launch() {
    const target = cwd.value.trim();
    if (!target) {
      error.value = "Choose a working directory first";
      return;
    }
    step.value = "launching";
    error.value = null;
    try {
      const info = await startNewClaudeSession(
        target,
        initialPrompt.value.trim() || null,
        worktree.value,
      );
      result.value = info;
      bumpRecent(target);
      recents.value = readRecents();
      step.value = "done";
    } catch (e) {
      error.value = String(e);
      step.value = "error";
    }
  }

  function selectRecent(path: string) {
    cwd.value = path;
  }

  return {
    step,
    cwd,
    initialPrompt,
    worktree,
    error,
    result,
    recents,
    reset,
    browse,
    launch,
    selectRecent,
  };
}
