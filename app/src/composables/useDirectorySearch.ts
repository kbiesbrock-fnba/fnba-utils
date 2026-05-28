import { ref } from "vue";
import {
  getAllRights,
  getRightAssociates,
  getAssociateRights,
  searchAssociates,
  getAssumeLogin,
  type IdentityConnection,
  type RightInfo,
  type RightAssociate,
} from "@/lib/tauri";

/**
 * Shared FNBA directory search, used by both Right Lookup and Assume Identity.
 * Owns the recent-rights store, a rights cache, and the search primitives so the
 * two commands resolve people / rights / logins through one implementation.
 */

// --- Search datasource ---------------------------------------------------
// Assume Identity always queries the directory on meleagris (the dev box that
// hosts perdb + logincheck). The *assume* itself still runs on the connection
// the operator selects. (Right Lookup queries its own selected connection.)
export const SEARCH_DATASOURCE = "meleagris";

/** Resolve the meleagris connection's full hostname from the connection list. */
export function resolveSearchServer(connections: IdentityConnection[]): string {
  const match = connections.find((c) =>
    c.server.toLowerCase().includes(SEARCH_DATASOURCE),
  );
  return match?.server ?? "meleagris.fnba.com";
}

// --- Recent-rights store (shared singleton) ------------------------------
const RECENT_RIGHTS_KEY = "fnba-utils:recent-rights";
const MAX_RECENT_RIGHTS = 5;

export interface RecentRight {
  rightId: number;
  rightName: string;
  timestamp: number;
}

function readRecentRights(): RecentRight[] {
  try {
    const raw = JSON.parse(localStorage.getItem(RECENT_RIGHTS_KEY) || "[]");
    if (!Array.isArray(raw)) return [];
    // Validate shape — a stored `null`, non-array, or entry missing the
    // expected fields would otherwise crash sort/filter at the call site
    // (and these refs evaluate at module scope, so a crash here takes the
    // whole command palette down with it).
    return raw.filter(
      (e): e is RecentRight =>
        e != null &&
        typeof e === "object" &&
        typeof e.rightId === "number" &&
        typeof e.rightName === "string" &&
        typeof e.timestamp === "number",
    );
  } catch {
    return [];
  }
}

function writeRecentRights(entries: RecentRight[]) {
  try {
    localStorage.setItem(RECENT_RIGHTS_KEY, JSON.stringify(entries));
  } catch {
    /* ignore storage errors */
  }
}

function sortedRecents(entries: RecentRight[]): RecentRight[] {
  return [...entries].sort((a, b) => (b.timestamp ?? 0) - (a.timestamp ?? 0));
}

/** Reactive recent rights, shared across every consumer of this module. */
export const recentRights = ref<RecentRight[]>(sortedRecents(readRecentRights()));

export function refreshRecentRights() {
  recentRights.value = sortedRecents(readRecentRights());
}

export function recordRecentRight(right: RightInfo) {
  const entries = readRecentRights().filter((e) => e.rightId !== right.rightId);
  entries.unshift({
    rightId: right.rightId,
    rightName: right.rightName,
    timestamp: Date.now(),
  });
  writeRecentRights(entries.slice(0, MAX_RECENT_RIGHTS));
  refreshRecentRights();
}

export function removeRecentRight(rightId: number) {
  writeRecentRights(readRecentRights().filter((e) => e.rightId !== rightId));
  refreshRecentRights();
}

// --- Rights cache --------------------------------------------------------
// The full rights list per server, fetched once and filtered client-side
// (Assume Identity's Rights scope filters as you type; Right Lookup reuses the
// cached fetch on connection select).
const rightsCache = new Map<string, RightInfo[]>();

// In-flight promises, keyed by server. Concurrent callers (e.g., UserPicker's
// onMounted firing while a parent also kicks off a refresh) deduplicate onto
// the same promise instead of each issuing their own getAllRights round-trip.
const rightsInflight = new Map<string, Promise<RightInfo[]>>();

export async function loadRights(server: string, force = false): Promise<RightInfo[]> {
  if (!force) {
    const cached = rightsCache.get(server);
    if (cached) return cached;
    const inflight = rightsInflight.get(server);
    if (inflight) return inflight;
  }
  const promise = (async () => {
    try {
      const list = await getAllRights(server);
      rightsCache.set(server, list);
      return list;
    } finally {
      rightsInflight.delete(server);
    }
  })();
  rightsInflight.set(server, promise);
  return promise;
}

export function filterRights(rights: RightInfo[], query: string): RightInfo[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  // Match by name or by right ID (so a numeric query finds a right by its id).
  return rights.filter(
    (r) => r.rightName.toLowerCase().includes(q) || String(r.rightId).includes(q),
  );
}

// --- People / login primitives ------------------------------------------
export function searchPeople(server: string, query: string): Promise<RightAssociate[]> {
  return searchAssociates(server, query);
}

export function holdersOfRight(server: string, right: RightInfo): Promise<RightAssociate[]> {
  // Match by right_id, the canonical primary key in notedb.fnba.rights. The
  // name isn't uniqueness-enforced, so passing rightName risked conflating
  // holders of two rights that happened to share a label.
  return getRightAssociates(server, null, right.rightId);
}

export function rightsForAssociate(server: string, assocId: number): Promise<RightInfo[]> {
  return getAssociateRights(server, assocId);
}

/** Resolve a person's bare Windows login (Right Lookup -> Assume hand-off). */
export function resolveAssumeLogin(server: string, assocId: number): Promise<string | null> {
  return getAssumeLogin(server, assocId);
}

/** Best-effort display name for a directory person. */
export function associateDisplayName(a: RightAssociate): string {
  const full = [a.firstName, a.lastName].filter(Boolean).join(" ").trim();
  return full || a.nickname || a.login || `#${a.assocId}`;
}
