import { ref } from "vue";
import {
  getIdentityData,
  executeAssumeIdentity,
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

const step = ref<AssumeIdentityStep>("user");
const users = ref<IdentityUser[]>([]);
const connections = ref<string[]>([]);
const selectedUser = ref<IdentityUser | null>(null);
const selectedConnection = ref<string | null>(null);
const result = ref<AssumeIdentityResult | null>(null);
const error = ref<string | null>(null);
const loading = ref(false);
const dataLoaded = ref(false);

export function useAssumeIdentity() {
  async function loadData() {
    if (dataLoaded.value) return;
    try {
      const data = await getIdentityData();
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
    selectedUser.value = null;
    selectedConnection.value = null;
    result.value = null;
    error.value = null;
    loading.value = false;
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
    try {
      result.value = await executeAssumeIdentity(
        selectedUser.value.username,
        selectedConnection.value,
      );
      step.value = "result";
    } catch (e) {
      error.value = String(e);
      step.value = "error";
    } finally {
      loading.value = false;
    }
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
    loadData,
    reset,
    selectUser,
    selectConnection,
    execute,
    goBack,
  };
}
