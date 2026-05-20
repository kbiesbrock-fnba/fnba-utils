import { computed, ref } from "vue";
import {
  listProjects,
  addProject,
  updateProject,
  removeProject,
  type Project,
} from "@/lib/tauri";

/**
 * Frontend mirror of the persistent project registry. Caller `refresh()`es
 * on mount; pin/unpin/add/remove operations mutate locally then sync.
 *
 * Sort order: pinned first (alphabetically by displayName), then unpinned
 * by `lastUsedAt` descending. The launcher (`CwdPicker` consumer) presents
 * this order verbatim; no additional sorting at render time.
 */

const projects = ref<Project[]>([]);
const loading = ref(false);
const error = ref<string | null>(null);

const sorted = computed<Project[]>(() => {
  const list = projects.value.slice();
  list.sort((a, b) => {
    if (a.pinned !== b.pinned) return a.pinned ? -1 : 1;
    if (a.pinned && b.pinned) return a.displayName.localeCompare(b.displayName);
    return b.lastUsedAt - a.lastUsedAt;
  });
  return list;
});

async function refresh() {
  loading.value = true;
  error.value = null;
  try {
    projects.value = await listProjects();
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

async function pin(cwd: string, pinned: boolean) {
  // Optimistic update.
  const idx = projects.value.findIndex((p) => p.cwd === cwd);
  if (idx >= 0) projects.value[idx].pinned = pinned;
  try {
    await updateProject(cwd, null, pinned, null);
  } catch (e) {
    error.value = String(e);
    await refresh();
  }
}

async function rename(cwd: string, displayName: string) {
  const trimmed = displayName.trim();
  if (!trimmed) return;
  const idx = projects.value.findIndex((p) => p.cwd === cwd);
  if (idx >= 0) projects.value[idx].displayName = trimmed;
  try {
    await updateProject(cwd, trimmed, null, null);
  } catch (e) {
    error.value = String(e);
    await refresh();
  }
}

async function add(cwd: string) {
  try {
    await addProject(cwd, null, false, null);
    await refresh();
  } catch (e) {
    error.value = String(e);
  }
}

async function remove(cwd: string) {
  projects.value = projects.value.filter((p) => p.cwd !== cwd);
  try {
    await removeProject(cwd);
  } catch (e) {
    error.value = String(e);
    await refresh();
  }
}

export function useProjects() {
  return {
    projects: sorted,
    loading,
    error,
    refresh,
    pin,
    rename,
    add,
    remove,
  };
}
