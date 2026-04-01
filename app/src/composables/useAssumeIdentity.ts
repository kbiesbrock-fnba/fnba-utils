import { ref } from "vue";
import {
  getIdentityData,
  executeAssumeIdentity,
  saveCustomEntry,
  type IdentityUser,
  type IdentityConnection,
  type AssumeIdentityResult,
} from "@/lib/tauri";

export type AssumeIdentityStep =
  | "imposter"
  | "user"
  | "connection"
  | "confirm"
  | "executing"
  | "result"
  | "error";

// --- Recent-user tracking via localStorage ---

const RECENT_KEY = "fnba-utils:recent-users";
const MAX_RECENT = 5;

export interface RecentEntry {
  username: string;
  label: string;
  connectionServer: string;
  connectionLabel: string;
  timestamp: number;
}

function readRecents(): RecentEntry[] {
  try {
    return JSON.parse(localStorage.getItem(RECENT_KEY) || "[]");
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

function recordRecent(user: IdentityUser, connection: IdentityConnection) {
  const entries = readRecents().filter((e) => e.username !== user.username);
  entries.unshift({
    username: user.username,
    label: user.label,
    connectionServer: connection.server,
    connectionLabel: connection.label,
    timestamp: Date.now(),
  });
  writeRecents(entries.slice(0, MAX_RECENT));
}

function deleteRecentUser(username: string) {
  writeRecents(readRecents().filter((e) => e.username !== username));
}

// --- Cross-command bridge ---

export const prefillUsername = ref<string | null>(null);

// --- Shared state ---

const step = ref<AssumeIdentityStep>("user");
const imposters = ref<string[]>([]);
const currentUser = ref("");
const selectedImposter = ref<string | null>(null);
const users = ref<IdentityUser[]>([]);
const connections = ref<IdentityConnection[]>([]);
const selectedUser = ref<IdentityUser | null>(null);
const selectedConnection = ref<IdentityConnection | null>(null);
const result = ref<AssumeIdentityResult | null>(null);
const error = ref<string | null>(null);
const loading = ref(false);
const dataLoaded = ref(false);
const recentUsers = ref<RecentEntry[]>(loadRecents());

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

  function reset() {
    step.value = "user";
    selectedImposter.value = currentUser.value || null;
    selectedUser.value = null;
    selectedConnection.value = null;
    result.value = null;
    error.value = null;
    loading.value = false;
    recentUsers.value = loadRecents();
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

  async function execute() {
    if (!selectedImposter.value || !selectedUser.value || !selectedConnection.value) return;
    step.value = "executing";
    loading.value = true;

    const user = selectedUser.value;
    const conn = selectedConnection.value;

    const imp = selectedImposter.value;
    const isNewUser = !users.value.some(
      (u) => u.username.toLowerCase() === user.username.toLowerCase(),
    );
    const isNewConnection = !connections.value.some(
      (c) => c.server.toLowerCase() === conn.server.toLowerCase(),
    );
    const isNewImposter = !imposters.value.some(
      (i) => i.toLowerCase() === imp.toLowerCase(),
    );

    try {
      result.value = await executeAssumeIdentity(
        selectedImposter.value!,
        user.username,
        conn.server,
      );

      // Save custom entries only after a successful execution
      if (isNewUser || isNewConnection || isNewImposter) {
        try {
          const saved = await saveCustomEntry(
            isNewUser ? user.username : undefined,
            isNewUser ? user.label : undefined,
            isNewConnection ? conn.server : undefined,
            isNewConnection ? conn.label : undefined,
            isNewImposter ? imp : undefined,
          );
          const parts: string[] = [];
          if (saved.addedUser) parts.push(user.username);
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
          loadData();
        } catch (saveErr) {
          const existing = result.value!.message ?? "";
          result.value!.message = existing
            ? `${existing} — Failed to save custom entry: ${saveErr}`
            : `Failed to save custom entry: ${saveErr}`;
        }
      }

      recordRecent(user, conn);
      recentUsers.value = loadRecents();
      step.value = "result";
    } catch (e) {
      error.value = String(e);
      step.value = "error";
    } finally {
      loading.value = false;
    }
  }

  function removeRecentUser(username: string) {
    deleteRecentUser(username);
    recentUsers.value = loadRecents();
  }

  function goBack(): boolean {
    switch (step.value) {
      case "user":
        step.value = "imposter";
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
    loadData,
    reset,
    selectImposter,
    selectUser,
    selectConnection,
    execute,
    removeRecentUser,
    goBack,
  };
}
