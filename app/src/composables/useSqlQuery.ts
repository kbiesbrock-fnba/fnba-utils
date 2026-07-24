import { computed, ref, type ComputedRef } from "vue";
import {
  addSqlGroup,
  addSqlQuery,
  executeSqlQuery,
  getIdentityData,
  killSqlQuery,
  listSqlGroups,
  listSqlQueries,
  migrateLegacySqlQueries,
  moveSqlQueryToGroup,
  onSqlQueriesChanged,
  recordSqlQueryUsed,
  removeSqlGroup,
  removeSqlQuery,
  renameSqlGroup,
  setSqlGroupPinned,
  updateSqlQuery,
  type IdentityConnection,
  type LegacySavedSqlQuery,
  type QueryResult,
  type SavedSqlQuery,
  type SqlGroup,
} from "@/lib/tauri";
import {
  isPanelPinned,
  readHashParams,
  rememberWindowFocus,
  setPanelPinned,
  updatePinnedPanel,
  type PinnedPanel,
} from "@/lib/panelStorage";

const LEGACY_KEY = "fnba-utils:saved-sql-queries";
const LEGACY_MIGRATED_KEY = "fnba-utils:saved-sql-queries:migrated";
const COLLAPSE_KEY = "fnba-utils:sql-group-collapsed";

/** A section in the sidebar — either a real group, or the synthetic "Ungrouped" bucket (group=null). */
export interface QuerySection {
  group: SqlGroup | null;
  queries: SavedSqlQuery[];
}

function readCollapsed(): Set<string> {
  try {
    const raw = localStorage.getItem(COLLAPSE_KEY);
    if (!raw) return new Set();
    const arr = JSON.parse(raw);
    return Array.isArray(arr) ? new Set(arr.map(String)) : new Set();
  } catch {
    return new Set();
  }
}

function writeCollapsed(ids: Set<string>) {
  try {
    localStorage.setItem(COLLAPSE_KEY, JSON.stringify([...ids]));
  } catch {
    /* ignore */
  }
}

/** Synthetic key used for the Ungrouped bucket's collapsed state. */
const UNGROUPED_KEY = "__ungrouped__";

const params = readHashParams();
const initialServer = params.get("server") ?? "";
const initialLabel = params.get("label") ?? "";
// Stable per-window identity, decoupled from the connection. Minted here for
// legacy/direct opens that predate the id param so pin/restore still work.
const panelId =
  params.get("id") ||
  (typeof crypto !== "undefined" && "randomUUID" in crypto
    ? crypto.randomUUID()
    : `sql-${Date.now()}-${Math.random().toString(36).slice(2)}`);

const server = ref(initialServer);
const label = ref(initialLabel);
const sql = ref("");
const database = ref("");
const result = ref<QueryResult | null>(null);
const error = ref<string | null>(null);
const running = ref(false);
const currentQueryId = ref<string | null>(null);
// Connections available in the header dropdown (canonical registry — same set
// Mission Control lists, no health probing).
const connections = ref<IdentityConnection[]>([]);

function ownPanel(): PinnedPanel {
  return { kind: "sql-query", id: panelId, server: server.value, label: label.value };
}

const pinned = ref(server.value ? isPanelPinned(ownPanel()) : false);

const groups = ref<SqlGroup[]>([]);
const queries = ref<SavedSqlQuery[]>([]);
const collapsedGroupIds = ref<Set<string>>(readCollapsed());
const loading = ref(false);

let initialised = false;
let listening = false;

const groupedQueries: ComputedRef<QuerySection[]> = computed(() => {
  // Sort groups: pinned section first, alphabetical within each section.
  // orderIdx is kept in the schema for a future manual-reorder affordance;
  // until that ships, alphabetical is the only order users see.
  const sortedGroups = [...groups.value].sort((a, b) => {
    if (a.pinned !== b.pinned) return a.pinned ? -1 : 1;
    return a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
  });

  // Queries: alphabetical. lastUsedAt is still recorded for future "recently
  // used" affordances but doesn't drive sort order.
  const sortQueries = (qs: SavedSqlQuery[]) =>
    [...qs].sort((a, b) =>
      a.name.localeCompare(b.name, undefined, { sensitivity: "base" }),
    );

  const sections: QuerySection[] = sortedGroups.map((g) => ({
    group: g,
    queries: sortQueries(queries.value.filter((q) => q.groupId === g.id)),
  }));

  const ungroupedQueries = sortQueries(queries.value.filter((q) => q.groupId == null));
  if (ungroupedQueries.length > 0 || sections.length === 0) {
    sections.push({ group: null, queries: ungroupedQueries });
  }
  return sections;
});

async function migrateLegacyOnce() {
  if (localStorage.getItem(LEGACY_MIGRATED_KEY)) return;
  let raw: string | null = null;
  try {
    raw = localStorage.getItem(LEGACY_KEY);
  } catch {
    return;
  }
  if (!raw) {
    localStorage.setItem(LEGACY_MIGRATED_KEY, "1");
    return;
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    localStorage.setItem(LEGACY_MIGRATED_KEY, "1");
    return;
  }
  if (!Array.isArray(parsed) || parsed.length === 0) {
    localStorage.setItem(LEGACY_MIGRATED_KEY, "1");
    return;
  }
  const entries: LegacySavedSqlQuery[] = parsed
    .filter(
      (e): e is { name: string; sql: string; database?: string } =>
        !!e &&
        typeof e === "object" &&
        typeof (e as { name?: unknown }).name === "string" &&
        typeof (e as { sql?: unknown }).sql === "string",
    )
    .map((e) => ({
      name: e.name,
      sql: e.sql,
      database: typeof e.database === "string" ? e.database : "",
    }));
  if (entries.length === 0) {
    localStorage.setItem(LEGACY_MIGRATED_KEY, "1");
    return;
  }
  try {
    await migrateLegacySqlQueries(entries);
  } catch (e) {
    // Don't set the migrated flag if the call failed — leave the localStorage
    // entries in place so a future load can retry.
    console.warn("[sql-query] legacy migration failed:", e);
    return;
  }
  localStorage.setItem(LEGACY_MIGRATED_KEY, "1");
}

async function refresh() {
  loading.value = true;
  try {
    const [g, q] = await Promise.all([listSqlGroups(), listSqlQueries()]);
    groups.value = g;
    queries.value = q;
  } catch (e) {
    console.warn("[sql-query] failed to load saved queries:", e);
  } finally {
    loading.value = false;
  }
}

async function loadConnections() {
  try {
    connections.value = (await getIdentityData()).connections;
  } catch (e) {
    console.warn("[sql-query] failed to load connections:", e);
  }
}

async function ensureLoaded() {
  if (initialised) return;
  initialised = true;
  syncTitle();
  await Promise.all([migrateLegacyOnce().then(refresh), loadConnections()]);
}

/** Reflect the active connection in the window/document title (taskbar,
 *  alt-tab, restored-window identity for the user). */
function syncTitle() {
  document.title = server.value ? `SQL — ${label.value || server.value}` : "SQL Query";
}

/** Switch the panel to a different connection. Runs are per-call on the
 *  backend (fresh connection each time), so this only swaps the target and
 *  clears the now-stale result. If this panel is pinned, persist the new
 *  connection so it restores here on next launch. */
function changeConnection(nextServer: string, nextLabel: string) {
  if (
    nextServer.toLowerCase() === server.value.toLowerCase() &&
    nextLabel === label.value
  ) {
    return;
  }
  server.value = nextServer;
  label.value = nextLabel;
  // The prior result belongs to the prior connection — drop it so the grid
  // never misattributes rows to the newly-selected server.
  result.value = null;
  error.value = null;
  syncTitle();
  if (pinned.value) {
    updatePinnedPanel(ownPanel());
  }
}

async function startListening() {
  if (listening) return;
  listening = true;

  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  rememberWindowFocus(getCurrentWindow().label);

  window.addEventListener("blur", async () => {
    if (!pinned.value) {
      const { getCurrentWindow: getCW } = await import("@tauri-apps/api/window");
      await getCW().hide();
    }
  });

  // Saved queries + groups are global, not per-server. Every panel listens for
  // changes so they all stay in sync after a save / delete / move in any one.
  // Best-effort: a failure just means this panel relies on its optimistic
  // local state until the next manual reload.
  try {
    await onSqlQueriesChanged(() => {
      void refresh();
    });
  } catch (e) {
    console.warn("[sql-query] failed to subscribe to sql-queries-changed:", e);
  }

  await ensureLoaded();
}

async function runQuery() {
  const queryText = sql.value.trim();
  if (!queryText || !server.value) return;

  const queryId =
    typeof crypto !== "undefined" && "randomUUID" in crypto
      ? crypto.randomUUID()
      : `q-${Date.now()}-${Math.random().toString(36).slice(2)}`;
  currentQueryId.value = queryId;
  running.value = true;
  error.value = null;
  result.value = null;

  // Capture the target so a connection switch mid-run can't misattribute this
  // result to the newly-selected server (the dropdown stays enabled while
  // running). If the server changed by the time we resolve, discard silently.
  const runServer = server.value;

  try {
    const res = await executeSqlQuery(runServer, database.value, queryText, queryId);
    if (server.value === runServer) result.value = res;
  } catch (e) {
    if (server.value === runServer) error.value = String(e);
  } finally {
    if (currentQueryId.value === queryId) {
      running.value = false;
      currentQueryId.value = null;
    }
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

async function saveQuery(name: string, groupId: string | null) {
  const trimmedName = name.trim();
  const trimmedSql = sql.value.trim();
  if (!trimmedName || !trimmedSql) return;
  try {
    const created = await addSqlQuery(trimmedName, trimmedSql, database.value, groupId);
    queries.value = [created, ...queries.value];
  } catch (e) {
    error.value = String(e);
  }
}

async function deleteQuery(id: string) {
  try {
    await removeSqlQuery(id);
    queries.value = queries.value.filter((q) => q.id !== id);
  } catch (e) {
    error.value = String(e);
  }
}

async function loadQuery(id: string) {
  const q = queries.value.find((x) => x.id === id);
  if (!q) return;
  sql.value = q.sql;
  if (q.database) database.value = q.database;
  recordSqlQueryUsed(id).then(
    () => {
      const now = Date.now();
      queries.value = queries.value.map((x) =>
        x.id === id ? { ...x, lastUsedAt: now } : x,
      );
    },
    () => {
      /* fire-and-forget */
    },
  );
}

async function moveQuery(queryId: string, groupId: string | null) {
  try {
    await moveSqlQueryToGroup(queryId, groupId);
    queries.value = queries.value.map((q) =>
      q.id === queryId ? { ...q, groupId } : q,
    );
  } catch (e) {
    error.value = String(e);
  }
}

async function createGroup(name: string): Promise<SqlGroup | null> {
  const trimmed = name.trim();
  if (!trimmed) return null;
  try {
    const created = await addSqlGroup(trimmed);
    groups.value = [...groups.value, created];
    return created;
  } catch (e) {
    error.value = String(e);
    return null;
  }
}

async function renameGroup(id: string, name: string) {
  const trimmed = name.trim();
  if (!trimmed) return;
  try {
    await renameSqlGroup(id, trimmed);
    groups.value = groups.value.map((g) => (g.id === id ? { ...g, name: trimmed } : g));
  } catch (e) {
    error.value = String(e);
  }
}

async function deleteGroup(id: string) {
  try {
    await removeSqlGroup(id);
    groups.value = groups.value.filter((g) => g.id !== id);
    // FK ON DELETE SET NULL in the DB demoted these to ungrouped server-side.
    // Reflect that locally without a refetch.
    queries.value = queries.value.map((q) =>
      q.groupId === id ? { ...q, groupId: null } : q,
    );
    collapsedGroupIds.value.delete(id);
    writeCollapsed(collapsedGroupIds.value);
  } catch (e) {
    error.value = String(e);
  }
}

async function renameQuery(id: string, name: string) {
  const trimmed = name.trim();
  if (!trimmed) return;
  const target = queries.value.find((q) => q.id === id);
  if (!target) return;
  try {
    await updateSqlQuery(id, trimmed, target.sql, target.database);
    queries.value = queries.value.map((q) =>
      q.id === id ? { ...q, name: trimmed } : q,
    );
  } catch (e) {
    error.value = String(e);
  }
}

async function toggleGroupPin(id: string) {
  const target = groups.value.find((g) => g.id === id);
  if (!target) return;
  const next = !target.pinned;
  try {
    await setSqlGroupPinned(id, next);
    groups.value = groups.value.map((g) => (g.id === id ? { ...g, pinned: next } : g));
  } catch (e) {
    error.value = String(e);
  }
}

function isCollapsed(groupId: string | null): boolean {
  return collapsedGroupIds.value.has(groupId ?? UNGROUPED_KEY);
}

function toggleCollapsed(groupId: string | null) {
  const key = groupId ?? UNGROUPED_KEY;
  const next = new Set(collapsedGroupIds.value);
  if (next.has(key)) next.delete(key);
  else next.add(key);
  collapsedGroupIds.value = next;
  writeCollapsed(next);
}

function togglePin() {
  pinned.value = !pinned.value;
  setPanelPinned(ownPanel(), pinned.value);
}

async function closeWindow() {
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().hide();
}

export function useSqlQuery() {
  startListening();

  return {
    server,
    label,
    connections,
    changeConnection,
    sql,
    database,
    result,
    error,
    running,
    pinned,
    loading,
    groups,
    queries,
    groupedQueries,
    runQuery,
    cancelQuery,
    saveQuery,
    deleteQuery,
    loadQuery,
    moveQuery,
    createGroup,
    renameGroup,
    renameQuery,
    deleteGroup,
    toggleGroupPin,
    isCollapsed,
    toggleCollapsed,
    togglePin,
    closeWindow,
  };
}
