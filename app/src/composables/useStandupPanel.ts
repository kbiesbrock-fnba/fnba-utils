import { ref, computed, onMounted, onUnmounted } from "vue";
import {
  getStandupPanelState,
  runStandup,
  setIssueHidden,
  clearHiddenIssues,
  setIssueOrder,
  clearManualOrder,
  hideWindow,
  onStandupUpdated,
  isTauri,
  type JiraIssue,
  type StandupPanelState,
  type StandupRunSummary,
} from "@/lib/tauri";

const PINNED_KEY = "fnba-utils:standup-panel-pinned";
const SHOW_COMPLETED_KEY = "fnba-utils:standup-show-completed";
const HISTORY_OPEN_KEY = "fnba-utils:standup-history-open";
const REFRESH_INTERVAL_MS = 5 * 60_000;

const STATUS_GROUP_RANK: Record<string, number> = {
  in_progress: 0,
  review: 1,
  todo: 2,
  attention: 3,
  done: 4,
};

function readBool(key: string, fallback = false): boolean {
  try {
    const v = localStorage.getItem(key);
    return v === null ? fallback : v === "1";
  } catch {
    return fallback;
  }
}

function writeBool(key: string, value: boolean) {
  try {
    localStorage.setItem(key, value ? "1" : "0");
  } catch {
    // ignore
  }
}

const pinned = ref(readBool(PINNED_KEY));
const showCompleted = ref(readBool(SHOW_COMPLETED_KEY));
const historyOpen = ref(readBool(HISTORY_OPEN_KEY));

const panelState = ref<StandupPanelState | null>(null);
const loading = ref(false);
const refreshing = ref(false);
const error = ref<string | null>(null);

let pollTimer: ReturnType<typeof setInterval> | null = null;
let cleanupListener: (() => void) | null = null;
let initialized = false;

async function load() {
  try {
    panelState.value = await getStandupPanelState();
    error.value = null;
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  }
}

async function refresh() {
  if (refreshing.value) return;
  refreshing.value = true;
  try {
    await runStandup(false);
    await load();
    error.value = null;
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    refreshing.value = false;
  }
}

async function toggleCompleted(key: string, currentlyCompleted: boolean) {
  await setIssueHidden(key, !currentlyCompleted);
  await load();
}

async function unhideAll() {
  await clearHiddenIssues();
  await load();
}

async function resetOrder() {
  await clearManualOrder();
  await load();
}

function togglePin() {
  pinned.value = !pinned.value;
  writeBool(PINNED_KEY, pinned.value);
}

function toggleShowCompleted() {
  showCompleted.value = !showCompleted.value;
  writeBool(SHOW_COMPLETED_KEY, showCompleted.value);
}

function toggleHistory() {
  historyOpen.value = !historyOpen.value;
  writeBool(HISTORY_OPEN_KEY, historyOpen.value);
}

/** All issues from the report, flattened — preserves source data for sorting. */
function flattenedIssues(state: StandupPanelState): JiraIssue[] {
  if (!state.report) return [];
  return state.report.groups.flatMap((g) => g.issues);
}

/** Sort comparator: manual_order (if set) → status group → priority → due date → key. */
function compareIssues(
  a: JiraIssue,
  b: JiraIssue,
  manualOrders: Record<string, number>,
): number {
  const am = manualOrders[a.key];
  const bm = manualOrders[b.key];
  // Items with a manual_order sort by it first (lower = earlier). Items without
  // manual_order come after anyone who has one.
  if (am !== undefined && bm !== undefined) {
    if (am !== bm) return am - bm;
  } else if (am !== undefined) {
    return -1;
  } else if (bm !== undefined) {
    return 1;
  }

  const ar = STATUS_GROUP_RANK[a.statusGroup] ?? 99;
  const br = STATUS_GROUP_RANK[b.statusGroup] ?? 99;
  if (ar !== br) return ar - br;

  if (a.priorityRank !== b.priorityRank) return a.priorityRank - b.priorityRank;

  // Earlier due date first; null sorts last.
  const ad = a.dueDate ?? "9999-12-31";
  const bd = b.dueDate ?? "9999-12-31";
  if (ad !== bd) return ad < bd ? -1 : 1;

  return a.key.localeCompare(b.key);
}

/** Issues for the Bugs section (top), sorted and filtered by show-completed. */
const bugs = computed<JiraIssue[]>(() => {
  const state = panelState.value;
  if (!state) return [];
  const hidden = new Set(state.hiddenKeys);
  return flattenedIssues(state)
    .filter((i) => i.isBug)
    .filter((i) => showCompleted.value || !hidden.has(i.key))
    .sort((a, b) => compareIssues(a, b, state.manualOrders));
});

/** Non-bug issues (no heading), sorted and filtered by show-completed. */
const tasks = computed<JiraIssue[]>(() => {
  const state = panelState.value;
  if (!state) return [];
  const hidden = new Set(state.hiddenKeys);
  return flattenedIssues(state)
    .filter((i) => !i.isBug)
    .filter((i) => showCompleted.value || !hidden.has(i.key))
    .sort((a, b) => compareIssues(a, b, state.manualOrders));
});

const hiddenSet = computed(
  () => new Set<string>(panelState.value?.hiddenKeys ?? []),
);

function isCompleted(key: string): boolean {
  return hiddenSet.value.has(key);
}

const completedCount = computed(() => panelState.value?.hiddenKeys.length ?? 0);

const history = computed<StandupRunSummary[]>(
  () => panelState.value?.history ?? [],
);

const lastRun = computed(() => panelState.value?.lastRun ?? null);

/**
 * Persist a new order for a section by writing manual_order = index for each key.
 * Triggers a reload to pick up the new state.
 */
async function reorderSection(orderedKeys: string[]) {
  await setIssueOrder(orderedKeys);
  await load();
}

export function useStandupPanel() {
  onMounted(async () => {
    if (!initialized) {
      initialized = true;
      loading.value = true;
      await load();
      loading.value = false;

      pollTimer = setInterval(() => {
        void load();
      }, REFRESH_INTERVAL_MS);

      onStandupUpdated(() => {
        void load();
      }).then((unlisten) => {
        cleanupListener = unlisten;
      });

      if (isTauri) {
        window.addEventListener("blur", () => {
          if (pinned.value) return;
          setTimeout(() => {
            if (document.hasFocus()) return;
            void hideWindow();
          }, 50);
        });
      }
    }
  });

  onUnmounted(() => {
    if (cleanupListener) {
      cleanupListener();
      cleanupListener = null;
    }
    if (pollTimer) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
    initialized = false;
  });

  return {
    pinned,
    showCompleted,
    historyOpen,
    loading,
    refreshing,
    error,
    panelState,
    bugs,
    tasks,
    completedCount,
    history,
    lastRun,
    refresh,
    toggleCompleted,
    isCompleted,
    unhideAll,
    resetOrder,
    reorderSection,
    togglePin,
    toggleShowCompleted,
    toggleHistory,
  };
}
