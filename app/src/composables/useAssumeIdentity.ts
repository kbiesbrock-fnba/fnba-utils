import { ref } from "vue";
import {
  getIdentityData,
  executeAssumeIdentity,
  saveCustomEntry,
  type IdentityUser,
  type AssumeIdentityResult,
} from "../lib/tauri";

export type AssumeIdentityStep =
  | "user"
  | "connection"
  | "confirm"
  | "executing"
  | "result"
  | "error";

// --- Recent-user tracking via localStorage ---

const RECENT_KEY = "fnba-utils:recent-users";
const MAX_RECENT = 5;

interface RecentEntry {
  username: string;
  timestamp: number;
}

function loadRecentUsernames(): string[] {
  try {
    const entries: RecentEntry[] = JSON.parse(
      localStorage.getItem(RECENT_KEY) || "[]",
    );
    return entries
      .sort((a, b) => b.timestamp - a.timestamp)
      .map((e) => e.username);
  } catch {
    return [];
  }
}

function recordRecentUser(username: string) {
  try {
    let entries: RecentEntry[] = JSON.parse(
      localStorage.getItem(RECENT_KEY) || "[]",
    );
    entries = entries.filter((e) => e.username !== username);
    entries.unshift({ username, timestamp: Date.now() });
    localStorage.setItem(
      RECENT_KEY,
      JSON.stringify(entries.slice(0, MAX_RECENT)),
    );
  } catch {
    /* ignore storage errors */
  }
}

function deleteRecentUser(username: string) {
  try {
    let entries: RecentEntry[] = JSON.parse(
      localStorage.getItem(RECENT_KEY) || "[]",
    );
    entries = entries.filter((e) => e.username !== username);
    localStorage.setItem(RECENT_KEY, JSON.stringify(entries));
  } catch {
    /* ignore storage errors */
  }
}

// --- Shared state ---

const step = ref<AssumeIdentityStep>("user");
const users = ref<IdentityUser[]>([]);
const connections = ref<string[]>([]);
const selectedUser = ref<IdentityUser | null>(null);
const selectedConnection = ref<string | null>(null);
const result = ref<AssumeIdentityResult | null>(null);
const error = ref<string | null>(null);
const loading = ref(false);
const dataLoaded = ref(false);
const recentUsernames = ref<string[]>(loadRecentUsernames());

export function useAssumeIdentity() {
  async function loadData() {
    if (dataLoaded.value) return;
    try {
      const data = await getIdentityData();
      users.value = data.users;
      connections.value = data.connections.sort((a, b) => a.localeCompare(b));
      dataLoaded.value = true;
    } catch (e) {
      error.value = String(e);
      step.value = "error";
    }
  }

  function reset() {
    step.value = "user";
    selectedUser.value = null;
    selectedConnection.value = null;
    result.value = null;
    error.value = null;
    loading.value = false;
    recentUsernames.value = loadRecentUsernames();
  }

  function selectUser(user: IdentityUser) {
    selectedUser.value = user;
    step.value = "connection";
  }

  function selectConnection(conn: string) {
    selectedConnection.value = conn;
    step.value = "confirm";
  }

  async function execute() {
    if (!selectedUser.value || !selectedConnection.value) return;
    step.value = "executing";
    loading.value = true;

    const username = selectedUser.value.username;
    const connection = selectedConnection.value;

    const isNewUser = !users.value.some(
      (u) => u.username.toLowerCase() === username.toLowerCase(),
    );
    const isNewConnection = !connections.value.some(
      (c) => c.toLowerCase() === connection.toLowerCase(),
    );

    // Save custom entries BEFORE executing so the PowerShell script
    // can resolve them via ~/.assumeIdentity.json
    let saved: { addedUser: boolean; addedConnection: boolean } | null = null;
    if (isNewUser || isNewConnection) {
      try {
        saved = await saveCustomEntry(
          isNewUser ? username : undefined,
          isNewConnection ? connection : undefined,
        );
      } catch {
        /* best-effort */
      }
    }

    try {
      result.value = await executeAssumeIdentity(username, connection);

      // Append "saved for next time" message on success
      if (saved) {
        const parts: string[] = [];
        if (saved.addedUser) parts.push(username);
        if (saved.addedConnection) parts.push(connection);
        if (parts.length > 0) {
          const added = parts.join(" and ");
          const existing = result.value.message ?? "";
          result.value.message = existing
            ? `${existing} — Saved ${added} for next time.`
            : `Saved ${added} for next time.`;
        }
        dataLoaded.value = false;
        loadData();
      }

      recordRecentUser(username);
      recentUsernames.value = loadRecentUsernames();
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
    recentUsernames.value = loadRecentUsernames();
  }

  function goBack(): boolean {
    switch (step.value) {
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
    users,
    connections,
    selectedUser,
    selectedConnection,
    result,
    error,
    loading,
    recentUsernames,
    loadData,
    reset,
    selectUser,
    selectConnection,
    execute,
    removeRecentUser,
    goBack,
  };
}
