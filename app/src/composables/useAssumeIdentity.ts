import { computed, ref } from "vue";
import {
  getIdentityData,
  executeAssumeIdentity,
  saveCustomEntry,
  deleteCustomEntry,
  pinFavorite,
  removeFavorite,
  markFavoriteUsed,
  type IdentityUser,
  type IdentityConnection,
  type IdentityImposter,
  type AssumeIdentityResult,
  type RightAssociate,
  type RightInfo,
} from "@/lib/tauri";
import { resolveSearchServer, rightsForAssociate } from "@/composables/useDirectorySearch";

export type AssumeIdentityStep =
  | "imposter"
  | "user"
  | "userRights"
  | "connection"
  | "confirm"
  | "executing"
  | "result"
  | "error";

/** Outcome of assuming the chosen identity on ONE selected connection. Exactly
 *  one of `result` / `error` is non-null. The combined result step renders one
 *  of these per connection. */
export interface ConnectionRunResult {
  connection: IdentityConnection;
  result: AssumeIdentityResult | null;
  error: string | null;
}

/** Live progress of the sequential per-connection loop, for the executing view. */
export interface ExecutingProgress {
  current: number;
  total: number;
  server: string;
}

// --- Favorites ordering ---
// Composite key matching the Rust `fav_key` (label + U+001F + username). A
// username can repeat across labels in the defaults, so the hot-pick order is
// keyed on the pair, not the username alone.
export function favoriteKey(label: string, username: string): string {
  return `${label}${username}`;
}

// --- Recently Used (transient, localStorage) ---
// Sibling section to Favorites: holds the last N unpinned assumes. New unpinned
// assumes unshift to the front; the oldest drop off when the cap is hit. Pinning
// a recent moves it to Favorites and clears it from here.

const RECENT_KEY = "fnba-utils:recent-users";
const MAX_RECENT = 5;

export interface RecentEntry {
  username: string;
  label: string;
  timestamp: number;
}

function readRecents(): RecentEntry[] {
  try {
    const raw = JSON.parse(localStorage.getItem(RECENT_KEY) || "[]");
    if (!Array.isArray(raw)) return [];
    // Validate shape — a stored `null`, non-array, or entry missing the
    // expected fields would otherwise crash sort/filter at the call site.
    // Module-scope refs evaluate eagerly here, so a crash takes the whole
    // command palette down on init.
    return raw.filter(
      (e): e is RecentEntry =>
        e != null &&
        typeof e === "object" &&
        typeof e.username === "string" &&
        typeof e.label === "string" &&
        typeof e.timestamp === "number",
    );
  } catch {
    return [];
  }
}

function writeRecents(entries: RecentEntry[]) {
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify(entries));
  } catch {
    /* ignore storage errors */
  }
}

function loadRecents(): RecentEntry[] {
  return readRecents().sort((a, b) => (b.timestamp ?? 0) - (a.timestamp ?? 0));
}

function sameComposite(
  a: { label: string; username: string },
  label: string,
  username: string,
) {
  return (
    a.username.toLowerCase() === username.toLowerCase() &&
    a.label.toLowerCase() === label.toLowerCase()
  );
}

// --- Shared state ---

const step = ref<AssumeIdentityStep>("user");
const imposters = ref<IdentityImposter[]>([]);
const currentUser = ref("");
const selectedImposter = ref<string | null>(null); // stores the name string
const users = ref<IdentityUser[]>([]); // favorites, in saved hot-pick order
const connections = ref<IdentityConnection[]>([]);
const selectedUser = ref<IdentityUser | null>(null);
// Multi-select: the connection picker can toggle several connections; the
// assume runs sequentially against each. A single-select (row click / digit)
// is just a one-element list — one unified flow.
const selectedConnections = ref<IdentityConnection[]>([]);
// Per-connection outcomes for the combined result step.
const runResults = ref<ConnectionRunResult[]>([]);
const executingProgress = ref<ExecutingProgress | null>(null);
const error = ref<string | null>(null);
const loading = ref(false);
const dataLoaded = ref(false);
const recentUsers = ref<RecentEntry[]>(loadRecents());

// Run-generation guard. execute() captures this token at loop start; reset()
// bumps it. State refs are module-level singletons, so an execute() left
// in-flight by a closed/reopened palette would otherwise keep mutating shared
// state — between connections we bail if the token went stale.
let runToken = 0;

function recordRecentUser(label: string, username: string) {
  const filtered = readRecents().filter((e) => !sameComposite(e, label, username));
  filtered.unshift({ username, label, timestamp: Date.now() });
  writeRecents(filtered.slice(0, MAX_RECENT));
  recentUsers.value = loadRecents();
}

function removeRecentUser(label: string, username: string) {
  writeRecents(readRecents().filter((e) => !sameComposite(e, label, username)));
  recentUsers.value = loadRecents();
}
// Reverse "what rights does this person have" audit view (a person action).
const inspectedAssociate = ref<RightAssociate | null>(null);
const inspectedRights = ref<RightInfo[]>([]);
// Monotonic counter so a slow rightsForAssociate response for an earlier
// Tab'd associate can't overwrite inspectedRights after the operator has
// already moved on to a different person.
let viewRightsVersion = 0;

// The directory datasource (always meleagris) that the live user/right search
// queries — independent of the connection the assume ultimately runs on.
const searchServer = computed(() => resolveSearchServer(connections.value));

export function useAssumeIdentity() {
  async function loadData() {
    if (dataLoaded.value) return;
    try {
      const data = await getIdentityData();
      currentUser.value = data.currentUser;
      imposters.value = data.imposters;
      if (!selectedImposter.value) {
        selectedImposter.value = data.currentUser;
      }
      users.value = data.users;
      connections.value = data.connections;
      dataLoaded.value = true;
    } catch (e) {
      error.value = String(e);
      step.value = "error";
    }
  }

  async function reloadData() {
    dataLoaded.value = false;
    await loadData();
  }

  function reset() {
    step.value = "user";
    selectedImposter.value = currentUser.value || null;
    selectedUser.value = null;
    selectedConnections.value = [];
    runResults.value = [];
    executingProgress.value = null;
    inspectedAssociate.value = null;
    inspectedRights.value = [];
    error.value = null;
    loading.value = false;
    // Invalidate any execute() loop still in flight from a prior open.
    runToken++;
  }

  /** Audit a searched person's rights (the reverse "what can they do" view). */
  async function viewRights(assoc: RightAssociate) {
    const version = ++viewRightsVersion;
    inspectedAssociate.value = assoc;
    inspectedRights.value = [];
    step.value = "userRights";
    loading.value = true;
    try {
      const res = await rightsForAssociate(searchServer.value, assoc.assocId);
      // Drop the result if the operator has since Tab'd a different associate
      // (or escaped out). Without this, a slow request for A can overwrite
      // inspectedRights after B is on screen — wrong audit data shown silently.
      if (version === viewRightsVersion) inspectedRights.value = res;
    } catch (e) {
      if (version === viewRightsVersion) {
        error.value = String(e);
        step.value = "error";
      }
    } finally {
      if (version === viewRightsVersion) loading.value = false;
    }
  }

  /** Assume the person currently being audited (jumps to the connection step). */
  function assumeInspected() {
    const a = inspectedAssociate.value;
    if (!a || !a.login) return;
    // `||` (not `??`) so an empty-string job_title / department from the DB
    // cascades to the next fallback. With `??`, "" propagated as the label
    // and the row's pin / remove × buttons (gated on `row.roleLabel` being
    // truthy) hid themselves, stranding the entry.
    selectUser({ username: a.login, label: a.jobTitle || a.department || "Custom" });
  }

  function selectImposter(imp: string) {
    selectedImposter.value = imp;
    step.value = "user";
  }

  function selectUser(user: IdentityUser) {
    selectedUser.value = user;
    step.value = "connection";
  }

  function selectConnections(conns: IdentityConnection[]) {
    if (conns.length === 0) return;
    selectedConnections.value = conns;
    step.value = "confirm";
  }

  /** Is this username already a favorite (under any label)? Drives the pin offer. */
  function isFavorite(username: string): boolean {
    return users.value.some(
      (u) => u.username.toLowerCase() === username.toLowerCase(),
    );
  }

  /** Explicitly pin a user to the distributable favorites list. Triggered by
   *  the row's pin button — the user just used this person, so we also stamp
   *  LastUsed so they land at #1 in Favorites rather than at the bottom. */
  async function pinUser(username: string, label: string) {
    await pinFavorite(username, label);
    // Mark used so the new favorite floats to #1 (the user clearly wanted it
    // top-of-mind — that's why they pinned it).
    try {
      await markFavoriteUsed(label, username);
    } catch {
      /* ignore — pin succeeded, ranking is best-effort */
    }
    await reloadData();
    // Clear the matching recent if any — it's a favorite now, recents must not
    // double-show it (the display filter would also catch this, but be explicit).
    removeRecentUser(label, username);
  }

  /** Remove a favorite — custom or default — from view. */
  async function unpinFavorite(label: string, username: string) {
    await removeFavorite(label, username);
    await reloadData();
  }


  async function execute() {
    if (!selectedImposter.value || !selectedUser.value || selectedConnections.value.length === 0)
      return;
    step.value = "executing";
    loading.value = true;
    runResults.value = [];

    const user = selectedUser.value;
    const imp = selectedImposter.value;
    const conns = selectedConnections.value;
    const token = runToken;

    // The imposter is one value for the whole run — save it (if new) only once,
    // regardless of how many connections we hit. `savedAnything` drives a single
    // best-effort identity-data refresh after the loop.
    let imposterSaved = false;
    let savedAnything = false;
    const results: ConnectionRunResult[] = [];

    for (let i = 0; i < conns.length; i++) {
      // Bail if the palette was reset/reopened mid-run — the module-level state
      // now belongs to a newer flow; don't keep mutating it.
      if (token !== runToken) {
        loading.value = false;
        return;
      }

      const conn = conns[i];
      executingProgress.value = { current: i + 1, total: conns.length, server: conn.server };

      // Users are no longer auto-saved — favorites are explicit (see pinUser).
      // Connections/imposters typed inline are still remembered. Detect newness
      // per connection so several new connections in one run each get saved.
      const isNewConnection = !connections.value.some(
        (c) => c.server.toLowerCase() === conn.server.toLowerCase(),
      );
      const isNewImposter =
        !imposterSaved &&
        !imposters.value.some((iy) => iy.name.toLowerCase() === imp.toLowerCase());

      try {
        const res = await executeAssumeIdentity(imp, user.username, conn.server);

        if (isNewConnection || isNewImposter) {
          try {
            const saved = await saveCustomEntry(
              isNewConnection ? conn.server : undefined,
              isNewConnection ? conn.label : undefined,
              isNewImposter ? imp : undefined,
            );
            const parts: string[] = [];
            if (saved.addedConnection) parts.push(conn.server);
            if (saved.addedImposter) {
              parts.push(imp);
              imposterSaved = true;
            }
            if (parts.length > 0) {
              savedAnything = true;
              const added = parts.join(" and ");
              const existing = res.message ?? "";
              res.message = existing
                ? `${existing} — Saved ${added} for next time.`
                : `Saved ${added} for next time.`;
            }
          } catch (saveErr) {
            const existing = res.message ?? "";
            res.message = existing
              ? `${existing} — Failed to save custom entry: ${saveErr}`
              : `Failed to save custom entry: ${saveErr}`;
          }
        }

        results.push({ connection: conn, result: res, error: null });
      } catch (e) {
        // Per-connection failure never aborts the run — it's captured and shown
        // inline in the combined result. The global "error" step is reserved for
        // pre-flight guard failures only.
        results.push({ connection: conn, result: null, error: String(e) });
      }
    }

    // Loop finished — bail without touching the view if a reset raced us.
    if (token !== runToken) {
      loading.value = false;
      return;
    }

    runResults.value = results;
    executingProgress.value = null;
    step.value = "result";
    loading.value = false;

    // Favorites/recents are keyed on (user, label), not connection — bump ONCE
    // per run. These run after the result is on screen and never mutate `step`.
    // Bubble this favorite to #1 — but only if the *exact* (label, username)
    // pair is currently a favorite. One-off assumes from directory search never
    // become "recents" and never reorder favorites; the favorites list stays a
    // curated thing the user opts into via pinning.
    const isExactFav = users.value.some(
      (u) =>
        u.username.toLowerCase() === user.username.toLowerCase() &&
        u.label.toLowerCase() === user.label.toLowerCase(),
    );
    if (isExactFav) {
      try {
        await markFavoriteUsed(user.label, user.username);
        await reloadData();
      } catch {
        /* ignore — assume already succeeded */
      }
    } else {
      // Unpinned assume → drop it in Recently Used (FIFO cap 5).
      recordRecentUser(user.label, user.username);
      // Pick up any newly-saved connections/imposters for the next open. Fire-
      // and-forget: the assume already succeeded, so a transient DB blip must
      // not clobber the success view. (reloadData above already covers the fav
      // branch.)
      if (savedAnything) {
        dataLoaded.value = false;
        loadData().catch(() => {
          /* best-effort refresh */
        });
      }
    }
  }

  async function deleteCustomConnection(server: string) {
    await deleteCustomEntry(undefined, server);
    await reloadData();
  }

  async function deleteCustomImposter(name: string) {
    await deleteCustomEntry(undefined, undefined, name);
    await reloadData();
  }

  function goBack(): boolean {
    switch (step.value) {
      case "user":
        step.value = "imposter";
        return true;
      case "userRights":
        step.value = "user";
        inspectedAssociate.value = null;
        inspectedRights.value = [];
        return true;
      case "connection":
        step.value = "user";
        selectedUser.value = null;
        return true;
      case "confirm":
        // Back to the picker WITH the checked set intact, so reviewing the
        // selection and backing out doesn't discard it. The picker re-seeds its
        // checkboxes from selectedConnections on mount.
        step.value = "connection";
        return true;
      default:
        return false;
    }
  }

  return {
    step,
    imposters,
    currentUser,
    selectedImposter,
    users,
    connections,
    selectedUser,
    selectedConnections,
    runResults,
    executingProgress,
    error,
    loading,
    recentUsers,
    searchServer,
    inspectedAssociate,
    inspectedRights,
    loadData,
    reset,
    selectImposter,
    selectUser,
    selectConnections,
    isFavorite,
    pinUser,
    unpinFavorite,
    removeRecentUser,
    viewRights,
    assumeInspected,
    execute,
    deleteCustomConnection,
    deleteCustomImposter,
    goBack,
  };
}
