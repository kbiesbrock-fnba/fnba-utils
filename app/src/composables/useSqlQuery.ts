import { ref } from "vue";
import {
  executeSqlQuery,
  isTauri,
  type QueryResult,
} from "@/lib/tauri";

const SAVED_KEY = "fnba-utils:saved-sql-queries";
const PINNED_KEY = "fnba-utils:sql-query-pinned";

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
    // ignore
  }
}

function readInitialConnection(): { server: string; label: string } | null {
  try {
    const raw = localStorage.getItem("fnba-utils:sql-query-connection");
    return raw ? JSON.parse(raw) : null;
  } catch {
    return null;
  }
}

const initial = readInitialConnection();
const server = ref(initial?.server ?? "");
const label = ref(initial?.label ?? "");
const sql = ref("");
const database = ref("");
const result = ref<QueryResult | null>(null);
const error = ref<string | null>(null);
const running = ref(false);
const pinned = ref(localStorage.getItem(PINNED_KEY) === "true");
const savedQueries = ref<SavedQuery[]>(readSaved());

let listening = false;

function applyConnection(s: string, l: string) {
  server.value = s;
  label.value = l;
  result.value = null;
  error.value = null;
}

async function startListening() {
  if (listening) return;
  listening = true;

  window.addEventListener("blur", async () => {
    if (!pinned.value) {
      if (isTauri) {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        await getCurrentWindow().hide();
      }
    }
  });

  // Re-read localStorage every time this window gains focus.
  // This catches the initial open (where the event may fire before the
  // listener is registered) and subsequent connection switches.
  window.addEventListener("focus", () => {
    const conn = readInitialConnection();
    if (conn && conn.server && conn.server !== server.value) {
      applyConnection(conn.server, conn.label);
    }
  });

  if (isTauri) {
    const { listen } = await import("@tauri-apps/api/event");
    await listen<{ server: string; label: string }>(
      "connection-selected",
      (event) => {
        applyConnection(event.payload.server, event.payload.label);
      },
    );
  }
}

async function runQuery() {
  const query = sql.value.trim();
  if (!query || !server.value) return;

  running.value = true;
  error.value = null;
  result.value = null;

  try {
    result.value = await executeSqlQuery(server.value, database.value, query);
  } catch (e) {
    error.value = String(e);
  } finally {
    running.value = false;
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
  try { localStorage.setItem(PINNED_KEY, String(pinned.value)); } catch { /* ignore */ }
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
    pinned,
    saveQuery,
    removeQuery,
    loadQuery,
    togglePin,
    closeWindow,
  };
}
