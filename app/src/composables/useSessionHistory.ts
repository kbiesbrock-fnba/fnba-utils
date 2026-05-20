import { ref } from "vue";
import {
  listSessionHistory,
  forgetSessionHistory,
  resumeOwnedSession,
  type HistoricalSession,
  type NewSessionInfo,
} from "@/lib/tauri";

/**
 * Reactive mirror of the persistent session history. Caller refreshes on
 * mount and on the MC poll tick (cheap — just reads JSON state).
 *
 * `resume(sid)` re-spawns claude --resume <id> via the backend; caller
 * should follow up by opening the session-detail panel for the returned id.
 * `forget(sid)` permanently drops the entry from history.
 */

const history = ref<HistoricalSession[]>([]);
const loading = ref(false);
const error = ref<string | null>(null);

async function refresh() {
  loading.value = true;
  error.value = null;
  try {
    history.value = await listSessionHistory();
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

async function forget(sessionId: string) {
  history.value = history.value.filter((h) => h.sessionId !== sessionId);
  try {
    await forgetSessionHistory(sessionId);
  } catch (e) {
    error.value = String(e);
    await refresh();
  }
}

async function resume(sessionId: string): Promise<NewSessionInfo | null> {
  try {
    const info = await resumeOwnedSession(sessionId);
    // History entry was popped server-side; reflect that locally.
    history.value = history.value.filter((h) => h.sessionId !== sessionId);
    return info;
  } catch (e) {
    error.value = String(e);
    return null;
  }
}

export function useSessionHistory() {
  return { history, loading, error, refresh, forget, resume };
}
