import { ref } from "vue";
import {
  startNewClaudeSession,
  pickDirectory,
  type NewSessionInfo,
} from "@/lib/tauri";
import { useProjects } from "@/composables/useProjects";

export type LaunchStep = "form" | "launching" | "done" | "error";

export function useNewClaudeSession() {
  const step = ref<LaunchStep>("form");
  const cwd = ref<string>("");
  const initialPrompt = ref<string>("");
  const worktree = ref<boolean>(false);
  const error = ref<string | null>(null);
  const result = ref<NewSessionInfo | null>(null);

  // Source of truth: the persistent project registry. `start_new_claude_session`
  // calls `record_project_used` server-side on every successful spawn, so we
  // only need to refresh after launch to see the freshened ordering.
  const { projects, refresh: refreshProjects, pin } = useProjects();

  function reset() {
    step.value = "form";
    cwd.value = "";
    initialPrompt.value = "";
    worktree.value = false;
    error.value = null;
    result.value = null;
    refreshProjects();
  }

  async function browse() {
    try {
      const picked = await pickDirectory();
      if (picked) cwd.value = picked;
    } catch (e) {
      console.warn("[new-session] pickDirectory failed", e);
    } finally {
      // The native picker steals focus on Windows, which can demote our
      // alwaysOnTop frameless palette window behind the picker. Force the
      // palette back to front + focused after the picker resolves.
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
      step.value = "done";
      refreshProjects();
    } catch (e) {
      error.value = String(e);
      step.value = "error";
    }
  }

  function selectRecent(path: string) {
    cwd.value = path;
  }

  async function togglePin(path: string, nextPinned: boolean) {
    await pin(path, nextPinned);
  }

  return {
    step,
    cwd,
    initialPrompt,
    worktree,
    error,
    result,
    projects,
    reset,
    browse,
    launch,
    selectRecent,
    togglePin,
  };
}
