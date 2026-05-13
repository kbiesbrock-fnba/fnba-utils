import { ref } from "vue";
import {
  executeSqlQuery,
  isTauri,
  killSqlQuery,
  type QueryResult,
} from "@/lib/tauri";
import {
  isPanelPinned,
  readHashParams,
  rememberWindowFocus,
  setPanelPinned,
  type PinnedPanel,
} from "@/lib/panelStorage";

const SAVED_KEY = "fnba-utils:saved-sql-queries";

export interface SavedQuery {
  name: string;
  sql: string;
  database: string;
}

function readSaved(): SavedQuery[] {
  try {
    const raw = localStorage.getItem(SAVED_KEY);
    return raw ? JSON.parse(raw) : [];
  } catch {
    return [];
  }
}

function writeSaved(queries: SavedQuery[]) {
  try {
    localStorage.setItem(SAVED_KEY, JSON.stringify(queries));
  } catch {
    /* ignore */
  }
}

const params = readHashParams();
const initialServer = params.get("server") ?? "";
const initialLabel = params.get("label") ?? "";

const server = ref(initialServer);
const label = ref(initialLabel);
const sql = ref("");
const database = ref("");
const result = ref<QueryResult | null>(null);
const error = ref<string | null>(null);
const running = ref(false);
const currentQueryId = ref<string | null>(null);

function ownPanel(): PinnedPanel {
  return { kind: "sql-query", server: server.value, label: label.value };
}

const pinned = ref(server.value ? isPanelPinned(ownPanel()) : false);
const savedQueries = ref<SavedQuery[]>(readSaved());

let listening = false;

async function startListening() {
  if (listening) return;
  listening = true;

  if (isTauri) {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    rememberWindowFocus(getCurrentWindow().label);
  }

  window.addEventListener("blur", async () => {
    if (!pinned.value) {
      if (isTauri) {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        await getCurrentWindow().hide();
      }
    }
  });
}

async function runQuery() {
  const query = sql.value.trim();
  if (!query || !server.value) return;

  const queryId =
    typeof crypto !== "undefined" && "randomUUID" in crypto
      ? crypto.randomUUID()
      : `q-${Date.now()}-${Math.random().toString(36).slice(2)}`;
  currentQueryId.value = queryId;
  running.value = true;
  error.value = null;
  result.value = null;

  try {
    result.value = await executeSqlQuery(server.value, database.value, query, queryId);
  } catch (e) {
    error.value = String(e);
  } finally {
    running.value = false;
    currentQueryId.value = null;
  }
}

async function cancelQuery() {
  const id = currentQueryId.value;
  if (!id) return;
  try {
    await killSqlQuery(id);
  } catch (e) {
    error.value = String(e);
  }
}

function saveQuery(name: string) {
  if (!name.trim() || !sql.value.trim()) return;
  savedQueries.value.push({
    name: name.trim(),
    sql: sql.value.trim(),
    database: database.value,
  });
  writeSaved(savedQueries.value);
}

function removeQuery(index: number) {
  savedQueries.value.splice(index, 1);
  writeSaved(savedQueries.value);
}

function loadQuery(index: number) {
  const q = savedQueries.value[index];
  if (!q) return;
  sql.value = q.sql;
  database.value = q.database;
}

function togglePin() {
  pinned.value = !pinned.value;
  setPanelPinned(ownPanel(), pinned.value);
}

async function closeWindow() {
  if (!isTauri) return;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().hide();
}

export function useSqlQuery() {
  startListening();

  return {
    server,
    label,
    sql,
    database,
    result,
    error,
    running,
    savedQueries,
    runQuery,
    cancelQuery,
    pinned,
    saveQuery,
    removeQuery,
    loadQuery,
    togglePin,
    closeWindow,
  };
}
