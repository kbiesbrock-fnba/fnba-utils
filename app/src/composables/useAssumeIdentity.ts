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
const selectedConnection = ref<IdentityConnection | null>(null);
const result = ref<AssumeIdentityResult | null>(null);
const error = ref<string | null>(null);
const loading = ref(false);
const dataLoaded = ref(false);
const recentUsers = ref<RecentEntry[]>(loadRecents());

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
    selectedConnection.value = null;
    inspectedAssociate.value = null;
    inspectedRights.value = [];
    result.value = null;
    error.value = null;
    loading.value = false;
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

  function selectConnection(conn: IdentityConnection) {
    selectedConnection.value = conn;
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
    if (!selectedImposter.value || !selectedUser.value || !selectedConnection.value) return;
    step.value = "executing";
    loading.value = true;

    const user = selectedUser.value;
    const conn = selectedConnection.value;
    const imp = selectedImposter.value;

    // Users are no longer auto-saved — favorites are explicit (see pinUser).
    // Connections/imposters typed inline are still remembered.
    const isNewConnection = !connections.value.some(
      (c) => c.server.toLowerCase() === conn.server.toLowerCase(),
    );
    const isNewImposter = !imposters.value.some(
      (i) => i.name.toLowerCase() === imp.toLowerCase(),
    );

    try {
      result.value = await executeAssumeIdentity(imp, user.username, conn.server);

      if (isNewConnection || isNewImposter) {
        try {
          const saved = await saveCustomEntry(
            undefined,
            undefined,
            isNewConnection ? conn.server : undefined,
            isNewConnection ? conn.label : undefined,
            isNewImposter ? imp : undefined,
          );
          const parts: string[] = [];
          if (saved.addedConnection) parts.push(conn.server);
          if (saved.addedImposter) parts.push(imp);
          if (parts.length > 0) {
            const added = parts.join(" and ");
            const existing = result.value.message ?? "";
            result.value.message = existing
              ? `${existing} — Saved ${added} for next time.`
              : `Saved ${added} for next time.`;
          }
          dataLoaded.value = false;
          // Fire-and-forget: the assume itself succeeded and step='result' is
          // about to be set below — swallow any error from this background
          // refresh so a transient DB blip in getIdentityData() can't clobber
          // the success view with an error step. The next palette open will
          // refetch.
          loadData().catch(() => {
            /* assume already succeeded; stale data refresh is best-effort */
          });
        } catch (saveErr) {
          const existing = result.value!.message ?? "";
          result.value!.message = existing
            ? `${existing} — Failed to save custom entry: ${saveErr}`
            : `Failed to save custom entry: ${saveErr}`;
        }
      }

      // Bubble this favorite to #1 — but only if the *exact* (label, username)
      // pair is currently a favorite. One-off assumes from directory search
      // never become "recents" and never reorder favorites; the favorites list
      // stays a curated thing the user opts into via pinning. (A pin done in
      // the same flow lands before this check, because pinUser awaits
      // reloadData before the user reaches the connection step.)
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
        // Unpinned assume → drop it in Recently Used (FIFO cap 5). The user
        // can pin it later from the recents row; until then it cycles out
        // naturally as new unpinned assumes push it down.
        recordRecentUser(user.label, user.username);
      }
      step.value = "result";
    } catch (e) {
      error.value = String(e);
      step.value = "error";
    } finally {
      loading.value = false;
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
        step.value = "connection";
        selectedConnection.value = null;
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
    selectedConnection,
    result,
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
    selectConnection,
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
