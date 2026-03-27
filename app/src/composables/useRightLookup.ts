import { ref } from "vue";
import {
  getAllRights,
  getRightAssociates,
  getAssociateRights,
  type RightInfo,
  type RightAssociate,
} from "../lib/tauri";

export type RightLookupStep =
  | "loading"
  | "rights"
  | "executing"
  | "result"
  | "associateResult"
  | "error";

// --- Shared state (singleton) ---

const step = ref<RightLookupStep>("loading");
const rights = ref<RightInfo[]>([]);
const selectedRight = ref<RightInfo | null>(null);
const selectedAssociate = ref<RightAssociate | null>(null);
const associates = ref<RightAssociate[]>([]);
const associateRights = ref<RightInfo[]>([]);
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
    selectedAssociate.value = null;
    associates.value = [];
    associateRights.value = [];
    error.value = null;
    loading.value = false;
  }

  async function selectRight(right: RightInfo) {
    selectedRight.value = right;
    selectedAssociate.value = null;
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

  async function selectAssociate(assoc: RightAssociate) {
    selectedAssociate.value = assoc;
    selectedRight.value = null;
    step.value = "executing";
    loading.value = true;
    try {
      associateRights.value = await getAssociateRights(assoc.assocId);
      step.value = "associateResult";
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
        return false;
      default:
        return false;
    }
  }

  return {
    step,
    rights,
    selectedRight,
    selectedAssociate,
    associates,
    associateRights,
    error,
    loading,
    loadRights,
    reset,
    selectRight,
    selectAssociate,
    goBack,
  };
}
