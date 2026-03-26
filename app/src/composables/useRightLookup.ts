import { ref } from "vue";
import {
  getAllRights,
  getRightAssociates,
  type RightInfo,
  type RightAssociate,
} from "../lib/tauri";

export type RightLookupStep =
  | "loading"
  | "rights"
  | "executing"
  | "result"
  | "error";

// --- Shared state (singleton) ---

const step = ref<RightLookupStep>("loading");
const rights = ref<RightInfo[]>([]);
const selectedRight = ref<RightInfo | null>(null);
const associates = ref<RightAssociate[]>([]);
const error = ref<string | null>(null);
const loading = ref(false);

export function useRightLookup() {
  async function loadRights() {
    step.value = "loading";
    loading.value = true;
    try {
      rights.value = await getAllRights();
      step.value = "rights";
    } catch (e) {
      error.value = String(e);
      step.value = "error";
    } finally {
      loading.value = false;
    }
  }

  function reset() {
    step.value = "loading";
    rights.value = [];
    selectedRight.value = null;
    associates.value = [];
    error.value = null;
    loading.value = false;
  }

  async function selectRight(right: RightInfo) {
    selectedRight.value = right;
    step.value = "executing";
    loading.value = true;
    try {
      associates.value = await getRightAssociates(right.rightName, null);
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
      case "result":
        step.value = "rights";
        selectedRight.value = null;
        associates.value = [];
        return true;
      case "error":
        if (selectedRight.value) {
          step.value = "rights";
          selectedRight.value = null;
          error.value = null;
          return true;
        }
        return false;
      default:
        return false;
    }
  }

  return {
    step,
    rights,
    selectedRight,
    associates,
    error,
    loading,
    loadRights,
    reset,
    selectRight,
    goBack,
  };
}
