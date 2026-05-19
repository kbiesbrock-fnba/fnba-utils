import { ref } from "vue";
import {
  getAllRights,
  getRightAssociates,
  getAssociateRights,
  getIdentityData,
  deleteCustomEntry,
  type IdentityConnection,
  type RightInfo,
  type RightAssociate,
} from "@/lib/tauri";

const DEFAULT_SERVER = "meleagris";

export type RightLookupStep =
  | "connection"
  | "loading"
  | "rights"
  | "executing"
  | "result"
  | "associateResult"
  | "error";

// --- Recent-right tracking via localStorage ---

const RECENT_RIGHTS_KEY = "fnba-utils:recent-rights";
const MAX_RECENT_RIGHTS = 5;

export interface RecentRight {
  rightId: number;
  rightName: string;
  timestamp: number;
}

function readRecentRights(): RecentRight[] {
  try {
    return JSON.parse(localStorage.getItem(RECENT_RIGHTS_KEY) || "[]");
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

function loadRecentRights(): RecentRight[] {
  return readRecentRights().sort((a, b) => (b.timestamp ?? 0) - (a.timestamp ?? 0));
}

function recordRecentRight(right: RightInfo) {
  const entries = readRecentRights().filter((e) => e.rightId !== right.rightId);
  entries.unshift({
    rightId: right.rightId,
    rightName: right.rightName,
    timestamp: Date.now(),
  });
  writeRecentRights(entries.slice(0, MAX_RECENT_RIGHTS));
}

function deleteRecentRight(rightId: number) {
  writeRecentRights(readRecentRights().filter((e) => e.rightId !== rightId));
}

// --- Shared state (singleton) ---

const step = ref<RightLookupStep>("connection");
const connections = ref<IdentityConnection[]>([]);
const selectedConnection = ref<IdentityConnection | null>(null);
const rights = ref<RightInfo[]>([]);
const selectedRight = ref<RightInfo | null>(null);
const selectedAssociate = ref<RightAssociate | null>(null);
const associates = ref<RightAssociate[]>([]);
const associateRights = ref<RightInfo[]>([]);
const error = ref<string | null>(null);
const loading = ref(false);
const connectionsLoaded = ref(false);
const recentRights = ref<RecentRight[]>(loadRecentRights());

function server(): string {
  return selectedConnection.value?.server ?? DEFAULT_SERVER;
}

export function useRightLookup() {
  async function loadConnections() {
    if (connectionsLoaded.value) return;
    try {
      const data = await getIdentityData();
      connections.value = data.connections;
      connectionsLoaded.value = true;
      if (!selectedConnection.value) {
        const defaultConn = data.connections.find(
          (c) => c.server.toLowerCase().includes(DEFAULT_SERVER),
        );
        if (defaultConn) {
          selectConnection(defaultConn);
        }
      }
    } catch (e) {
      error.value = String(e);
      step.value = "error";
    }
  }

  async function loadRights() {
    step.value = "loading";
    loading.value = true;
    try {
      rights.value = await getAllRights(server());
      step.value = "rights";
    } catch (e) {
      error.value = String(e);
      step.value = "error";
    } finally {
      loading.value = false;
    }
  }

  function selectConnection(conn: IdentityConnection) {
    selectedConnection.value = conn;
    loadRights();
  }

  function reset() {
    step.value = "connection";
    rights.value = [];
    selectedRight.value = null;
    selectedAssociate.value = null;
    associates.value = [];
    associateRights.value = [];
    error.value = null;
    loading.value = false;
    recentRights.value = loadRecentRights();
  }

  async function selectRight(right: RightInfo) {
    selectedRight.value = right;
    selectedAssociate.value = null;
    step.value = "executing";
    loading.value = true;
    try {
      associates.value = await getRightAssociates(server(), right.rightName, null);
      recordRecentRight(right);
      recentRights.value = loadRecentRights();
      step.value = "result";
    } catch (e) {
      error.value = String(e);
      step.value = "error";
    } finally {
      loading.value = false;
    }
  }

  function removeRecentRight(rightId: number) {
    deleteRecentRight(rightId);
    recentRights.value = loadRecentRights();
  }

  async function selectAssociate(assoc: RightAssociate) {
    selectedAssociate.value = assoc;
    selectedRight.value = null;
    step.value = "executing";
    loading.value = true;
    try {
      associateRights.value = await getAssociateRights(server(), assoc.assocId);
      step.value = "associateResult";
    } catch (e) {
      error.value = String(e);
      step.value = "error";
    } finally {
      loading.value = false;
    }
  }

  async function deleteCustomConnection(server: string) {
    await deleteCustomEntry(undefined, server);
    connectionsLoaded.value = false;
    await loadConnections();
  }

  function goBack(): boolean {
    switch (step.value) {
      case "rights":
      case "loading":
        step.value = "connection";
        rights.value = [];
        return true;
      case "result":
      case "associateResult":
        step.value = "rights";
        selectedRight.value = null;
        selectedAssociate.value = null;
        associates.value = [];
        associateRights.value = [];
        return true;
      case "error":
        if (selectedRight.value || selectedAssociate.value) {
          step.value = "rights";
          selectedRight.value = null;
          selectedAssociate.value = null;
          error.value = null;
          return true;
        }
        step.value = "connection";
        error.value = null;
        return true;
      default:
        return false;
    }
  }

  return {
    step,
    connections,
    selectedConnection,
    rights,
    selectedRight,
    selectedAssociate,
    associates,
    associateRights,
    error,
    loading,
    recentRights,
    loadConnections,
    loadRights,
    reset,
    selectConnection,
    selectRight,
    selectAssociate,
    deleteCustomConnection,
    removeRecentRight,
    goBack,
  };
}
