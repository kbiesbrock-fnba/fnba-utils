<script setup lang="ts">
import { ref, computed, nextTick, onMounted, onUnmounted } from "vue";
import type { IdentityUser, RightInfo, RightAssociate } from "@/lib/tauri";
import type { RecentEntry } from "@/composables/useAssumeIdentity";
import {
  recentRights,
  recordRecentRight,
  removeRecentRight,
  loadRights,
  filterRights,
  searchPeople,
  holdersOfRight,
} from "@/composables/useDirectorySearch";
import { useListNavigation } from "@/composables/useListNavigation";
import CommandInput from "../CommandInput.vue";
import LabelPrompt from "./LabelPrompt.vue";

const props = defineProps<{
  users: IdentityUser[];
  recentUsers: RecentEntry[];
  searchServer: string;
}>();

const emit = defineEmits<{
  select: [user: IdentityUser];
  removeFavorite: [label: string, username: string];
  removeRecent: [label: string, username: string];
  pin: [username: string, label: string];
  viewRights: [assoc: RightAssociate];
}>();

type Scope = "people" | "rights";
const scope = ref<Scope>("people");
const query = ref("");
const listRef = ref<HTMLElement | null>(null);

// Pin offer shown after selecting a person who isn't already a favorite.
const pinMode = ref<{ username: string; defaultLabel: string } | null>(null);

// Rights scope
const allRights = ref<RightInfo[]>([]);
const rightDrill = ref<RightInfo | null>(null);
const holders = ref<RightAssociate[]>([]);
const holdersLoading = ref(false);

// People directory search (debounced)
const directory = ref<RightAssociate[]>([]);
const searching = ref(false);
let debounceTimer: ReturnType<typeof setTimeout> | null = null;
let searchVersion = 0;

// Whether to expand the collapsed login-less section in directory / holders results.
// Resets to false on each new search / drill so fresh queries always start collapsed.
const showNoLogin = ref(false);

onMounted(async () => {
  try {
    allRights.value = await loadRights(props.searchServer);
  } catch {
    allRights.value = [];
  }
});
onUnmounted(() => {
  if (debounceTimer) clearTimeout(debounceTimer);
});

// --- Row model (single shape to keep template type-checking simple) ---
interface Row {
  kind: "header" | "person" | "right" | "searching" | "empty" | "noLoginToggle";
  text?: string; // header label / empty text
  count?: number; // noLoginToggle: number of hidden login-less people
  flatIndex?: number; // selectable position (drives digit / arrow selection)
  badge?: string; // 1–9 hot-pick digit
  // person
  username?: string | null;
  roleLabel?: string;
  primary?: string;
  secondary?: string;
  isFavoriteRow?: boolean; // row came from props.users — removable via Delete
  isRecentUser?: boolean; // row came from props.recentUsers — transient, removable via Delete
  noLogin?: boolean;
  isRecent?: boolean; // used by right rows (recent rights, in the Rights scope)
  assoc?: RightAssociate; // present for directory / holder rows (enables "view rights")
  // right
  right?: RightInfo;
}

function badgeFor(idx: number): string | undefined {
  return idx < 9 ? String(idx + 1) : undefined;
}

// Reset the list scroll so a freshly-rendered view (e.g. drilling into a
// right's holders) starts at the top with row 0 visible, rather than keeping
// the previous view's scroll position.
function scrollListTop() {
  nextTick(() => {
    if (listRef.value) listRef.value.scrollTop = 0;
  });
}

function personLabel(a: RightAssociate): string {
  return a.jobTitle || a.department || "Search";
}
function personPrimary(a: RightAssociate): string {
  return a.nickname || a.login || `#${a.assocId}`;
}
function personSecondary(a: RightAssociate): string {
  const name = [a.firstName, a.lastName].filter(Boolean).join(" ");
  const role = a.jobTitle || a.department || "";
  return [name, role].filter(Boolean).join(" · ");
}

/** Composite (label, username) favorite check — drives the pin-offer decision
 *  and lets us hide recents that have since been pinned under the same label. */
function isFavComposite(label: string, username: string): boolean {
  const lo = label.toLowerCase();
  const un = username.toLowerCase();
  return props.users.some(
    (u) => u.label.toLowerCase() === lo && u.username.toLowerCase() === un,
  );
}

/**
 * Append selectable associate rows (those with a login) followed by a
 * collapsible toggle for any login-less associates. Returns the updated
 * selectable-index counter so callers can continue numbering other sections.
 *
 * Login-less people are genuinely unassumable (no Windows login exists), so
 * they are hidden by default. The toggle row has no flatIndex and therefore
 * does not interfere with digit/arrow navigation.
 */
function pushAssociateRows(rows: Row[], withLogin: RightAssociate[], noLogin: RightAssociate[], idx: number): number {
  for (const a of withLogin) {
    const fi = idx++;
    rows.push({
      kind: "person",
      flatIndex: fi,
      badge: badgeFor(fi),
      username: a.login,
      roleLabel: personLabel(a),
      primary: personPrimary(a),
      secondary: personSecondary(a),
      assoc: a,
    });
  }
  if (noLogin.length) {
    rows.push({ kind: "noLoginToggle", count: noLogin.length });
    if (showNoLogin.value) {
      for (const a of noLogin) {
        rows.push({
          kind: "person",
          username: null,
          roleLabel: personLabel(a),
          primary: personPrimary(a),
          secondary: personSecondary(a),
          noLogin: true,
          assoc: a,
        });
      }
    }
  }
  return idx;
}

function buildPeopleRows(): { rows: Row[]; selectable: number } {
  const rows: Row[] = [];
  let idx = 0;
  const q = query.value.trim().toLowerCase();
  // Used only to dedup directory results against each other (within-section).
  // Crucially NOT populated from favorites/recents — a person already pinned
  // under one label should still appear in directory search so the operator
  // can choose to pin them again under a different role label.
  const seen = new Set<string>();

  if (!q) {
    if (props.users.length) {
      rows.push({ kind: "header", text: "Favorites" });
      for (const u of props.users) {
        const fi = idx++;
        rows.push({
          kind: "person",
          flatIndex: fi,
          badge: badgeFor(fi),
          username: u.username,
          roleLabel: u.label,
          primary: u.username,
          secondary: u.label,
          isFavoriteRow: true,
        });
      }
    }

    // Recently Used — last N unpinned assumes. Filter out any that have since
    // been pinned (composite match against favorites). Numbering continues from
    // favorites, so digit 1–9 spans both sections in display order.
    const visibleRecents = props.recentUsers.filter(
      (r) => !isFavComposite(r.label, r.username),
    );
    if (visibleRecents.length) {
      rows.push({ kind: "header", text: "Recently Used" });
      for (const r of visibleRecents) {
        const fi = idx++;
        rows.push({
          kind: "person",
          flatIndex: fi,
          badge: badgeFor(fi),
          username: r.username,
          roleLabel: r.label,
          primary: r.username,
          secondary: r.label,
          isRecentUser: true,
        });
      }
    }

    if (idx === 0) {
      rows.push({ kind: "empty", text: "No favorites yet — type to search the directory" });
    }
    return { rows, selectable: idx };
  }

  const favMatches = props.users.filter(
    (u) => u.username.toLowerCase().includes(q) || u.label.toLowerCase().includes(q),
  );
  if (favMatches.length) {
    rows.push({ kind: "header", text: "Favorites" });
    for (const u of favMatches) {
      const fi = idx++;
      rows.push({
        kind: "person",
        flatIndex: fi,
        badge: badgeFor(fi),
        username: u.username,
        roleLabel: u.label,
        primary: u.username,
        secondary: u.label,
        isFavoriteRow: true,
      });
    }
  }

  // Matching Recents (between favorites and Directory). Skip composite matches
  // against favorites; allow same-username under a different label.
  const recentMatches = props.recentUsers.filter(
    (r) =>
      !isFavComposite(r.label, r.username) &&
      (r.username.toLowerCase().includes(q) || r.label.toLowerCase().includes(q)),
  );
  if (recentMatches.length) {
    rows.push({ kind: "header", text: "Recently Used" });
    for (const r of recentMatches) {
      const fi = idx++;
      rows.push({
        kind: "person",
        flatIndex: fi,
        badge: badgeFor(fi),
        username: r.username,
        roleLabel: r.label,
        primary: r.username,
        secondary: r.label,
        isRecentUser: true,
      });
    }
  }

  rows.push({ kind: "header", text: "Directory" });
  if (searching.value) {
    rows.push({ kind: "searching" });
  } else if (q.length < 2) {
    rows.push({ kind: "empty", text: "Keep typing to search…" });
  } else {
    const dir = directory.value.filter((a) => !a.login || !seen.has(a.login.toLowerCase()));
    if (dir.length === 0) {
      rows.push({ kind: "empty", text: "No directory matches" });
    } else {
      // Partition: associates with a Windows login are selectable and numbered;
      // login-less associates are hidden behind a collapsible toggle (they cannot
      // be assumed — no login exists — so they must not occupy selectable slots).
      const withLogin = dir.filter((a) => a.login);
      const noLogin = dir.filter((a) => !a.login);
      // Populate seen with every login we're about to render so callers that
      // later dedup against this set stay correct.
      for (const a of withLogin) {
        if (a.login) seen.add(a.login.toLowerCase());
      }
      idx = pushAssociateRows(rows, withLogin, noLogin, idx);
    }
  }
  return { rows, selectable: idx };
}

function buildRightsRows(): { rows: Row[]; selectable: number } {
  const rows: Row[] = [];
  let idx = 0;
  const q = query.value.trim();

  if (rightDrill.value) {
    rows.push({ kind: "header", text: `Holders of ${rightDrill.value.rightName}` });
    if (holdersLoading.value) {
      rows.push({ kind: "searching" });
    } else if (holders.value.length === 0) {
      rows.push({ kind: "empty", text: "No holders found" });
    } else {
      const withLogin = holders.value.filter((a) => a.login);
      const noLogin = holders.value.filter((a) => !a.login);
      idx = pushAssociateRows(rows, withLogin, noLogin, idx);
    }
    return { rows, selectable: idx };
  }

  if (!q) {
    if (recentRights.value.length) {
      rows.push({ kind: "header", text: "Recent Rights" });
      for (const r of recentRights.value) {
        const fi = idx++;
        rows.push({
          kind: "right",
          flatIndex: fi,
          badge: badgeFor(fi),
          right: { rightId: r.rightId, rightName: r.rightName },
          isRecent: true,
        });
      }
    } else {
      rows.push({ kind: "empty", text: "Type to search rights" });
    }
    return { rows, selectable: idx };
  }

  const matches = filterRights(allRights.value, q);
  if (matches.length === 0) {
    rows.push({ kind: "empty", text: "No matching rights" });
  } else {
    rows.push({ kind: "header", text: `Rights matching "${q}"` });
    for (const r of matches) {
      const fi = idx++;
      rows.push({ kind: "right", flatIndex: fi, badge: badgeFor(fi), right: r });
    }
  }
  return { rows, selectable: idx };
}

const displayData = computed(() =>
  scope.value === "people" ? buildPeopleRows() : buildRightsRows(),
);
const totalSelectable = computed(() => displayData.value.selectable);

function rowAtIndex(i: number): Row | undefined {
  return displayData.value.rows.find(
    (r) => (r.kind === "person" || r.kind === "right") && r.flatIndex === i,
  );
}

function selectAtIndex(i: number) {
  const row = rowAtIndex(i);
  if (!row) return;
  if (row.kind === "right" && row.right) {
    selectRight(row.right);
  } else if (row.kind === "person" && row.username) {
    selectPerson(row.username, row.roleLabel ?? "Custom");
  }
}

function selectPerson(username: string, label: string) {
  // Assume directly. Non-favorite assumes flow into Recently Used (via the
  // composable's execute()). Pinning is now an explicit side-action — see
  // startPin() / the row's pin button — not something that ever interrupts
  // the assume path.
  emit("select", { username, label });
}

/** Open the label prompt to pin a row to favorites. Triggered by the row's
 *  pin button (mouse) — explicit, never accidental. */
function startPin(username: string, label: string) {
  pinMode.value = { username, defaultLabel: label };
}

// Version counter for the holders-of-right fetch. Operators drill quickly
// (especially while exploring) — a slow earlier response must not overwrite a
// freshly-drilled right's holders.
let drillVersion = 0;

async function selectRight(right: RightInfo) {
  const version = ++drillVersion;
  rightDrill.value = right;
  recordRecentRight(right);
  resetIndex();
  showNoLogin.value = false;
  scrollListTop();
  holders.value = [];
  holdersLoading.value = true;
  try {
    const res = await holdersOfRight(props.searchServer, right);
    if (version !== drillVersion) return; // a newer drill superseded us
    holders.value = res;
  } catch {
    if (version === drillVersion) holders.value = [];
  } finally {
    if (version === drillVersion) {
      holdersLoading.value = false;
      // Re-anchor on the first holder now that the list has populated.
      resetIndex();
      scrollListTop();
    }
  }
}

function exitDrill() {
  rightDrill.value = null;
  resetIndex();
  scrollListTop();
}

function onPinConfirm(label: string) {
  const pm = pinMode.value;
  if (!pm) return;
  pinMode.value = null;
  // Pin only — do NOT emit select. Pinning is a side-action; the user remains
  // on the picker view, where the just-pinned entry has now moved into
  // Favorites (at the top, because pinUser also stamps LastUsed).
  emit("pin", pm.username, label);
}
function onPinCancel() {
  pinMode.value = null;
}

function switchScope(s: Scope) {
  if (scope.value === s && !(s === "rights" && rightDrill.value)) return;
  scope.value = s;
  resetIndex();
  scrollListTop();
  if (s === "rights") {
    rightDrill.value = null;
  } else if (query.value.trim().length >= 2) {
    scheduleSearch(query.value);
  }
}

async function runPeopleSearch(v: string, version: number) {
  try {
    const res = await searchPeople(props.searchServer, v);
    if (version === searchVersion) directory.value = res;
  } catch {
    if (version === searchVersion) directory.value = [];
  } finally {
    if (version === searchVersion) searching.value = false;
  }
}

function scheduleSearch(value: string) {
  if (debounceTimer) clearTimeout(debounceTimer);
  const v = value.trim();
  const version = ++searchVersion;
  if (v.length >= 2) {
    searching.value = true;
    debounceTimer = setTimeout(() => runPeopleSearch(v, version), 250);
  } else {
    directory.value = [];
    searching.value = false;
  }
}

function onUpdate(value: string) {
  query.value = value;
  resetIndex();
  showNoLogin.value = false;
  if (scope.value === "people") scheduleSearch(value);
}

const { selectedIndex, resetIndex } = useListNavigation({
  itemCount: () => (pinMode.value ? 0 : totalSelectable.value),
  onSelect: selectAtIndex,
  onEnterEmpty: () => {
    if (pinMode.value) return;
    if (scope.value === "people" && query.value.trim()) {
      selectPerson(query.value.trim(), "Custom");
    }
  },
  extraKeys: [
    {
      key: "ArrowRight",
      preventDefault: false,
      handler: (e) => {
        if (pinMode.value) return false;
        e.preventDefault();
        if (scope.value === "people") switchScope("rights");
      },
    },
    {
      key: "ArrowLeft",
      preventDefault: false,
      handler: (e) => {
        if (pinMode.value) return false;
        e.preventDefault();
        if (scope.value === "rights" && rightDrill.value) {
          exitDrill();
        } else if (scope.value === "rights") {
          switchScope("people");
        }
      },
    },
    {
      key: "Escape",
      handler: () => {
        if (pinMode.value) return false; // LabelPrompt owns Escape while open
        if (scope.value === "rights" && rightDrill.value) {
          exitDrill();
          return; // consumed
        }
        return false; // let the command step back to the imposter picker
      },
    },
    {
      key: "Delete",
      handler: () => {
        if (pinMode.value) return false;
        const row = rowAtIndex(selectedIndex.value);
        if (!row) return false;
        if (row.kind === "right" && row.isRecent && row.right) {
          removeRecentRight(row.right.rightId);
          return;
        }
        if (
          row.kind === "person" &&
          row.isFavoriteRow &&
          row.username &&
          row.roleLabel
        ) {
          emit("removeFavorite", row.roleLabel, row.username);
          return;
        }
        if (
          row.kind === "person" &&
          row.isRecentUser &&
          row.username &&
          row.roleLabel
        ) {
          emit("removeRecent", row.roleLabel, row.username);
          return;
        }
        return false;
      },
    },
    {
      // View the highlighted person's rights (the "what can they do" audit).
      key: "Tab",
      handler: () => {
        if (pinMode.value) return false;
        const row = rowAtIndex(selectedIndex.value);
        if (row?.assoc) emit("viewRights", row.assoc);
      },
    },
    ...["1", "2", "3", "4", "5", "6", "7", "8", "9"].map((d) => ({
      key: d,
      preventDefault: false,
      handler: (e: KeyboardEvent) => {
        if (pinMode.value) return false;
        // In the rights list, digits type into the search box (search by id).
        // Elsewhere (people / a right's holders) digits quick-select the Nth
        // row — but only when the search box is empty. Once the operator is
        // typing a query (e.g. a login or name containing a digit), digits
        // must reach the input or they'd be hijacked into a wrong-row select.
        // Mirrors ConnectionPicker's `query.value.trim()` guard.
        if (scope.value === "rights" && !rightDrill.value) return false;
        if (query.value.trim()) return false;
        e.preventDefault();
        const n = parseInt(d, 10) - 1;
        if (n < totalSelectable.value) selectAtIndex(n);
      },
    })),
  ],
  listRef,
  scrollStrategy: "selected-class",
});
</script>

<template>
  <LabelPrompt
    v-if="pinMode"
    :value="pinMode.username"
    :initial="pinMode.defaultLabel"
    default-label="Other"
    :placeholder="`Role label for ${pinMode.username} (edit or press Enter)…`"
    @confirm="onPinConfirm"
    @cancel="onPinCancel"
  />
  <template v-else>
    <CommandInput
      :value="query"
      :placeholder="scope === 'people' ? 'Search people…' : 'Search rights…'"
      @update="onUpdate"
    />
    <div class="segmented">
      <button class="seg" :class="{ active: scope === 'people' }" @click="switchScope('people')">
        People
      </button>
      <button class="seg" :class="{ active: scope === 'rights' }" @click="switchScope('rights')">
        Rights
      </button>
      <span class="seg-hint">←/→ switch</span>
    </div>
    <div class="picker-divider" />
    <div ref="listRef" class="picker-list">
      <template v-for="(row, i) in displayData.rows" :key="i">
        <div v-if="row.kind === 'header'" class="section-header">{{ row.text }}</div>

        <div v-else-if="row.kind === 'empty'" class="empty">{{ row.text }}</div>

        <div v-else-if="row.kind === 'searching'" class="searching-row">
          <div class="mini-spinner" />
          <span>Searching…</span>
        </div>

        <div
          v-else-if="row.kind === 'noLoginToggle'"
          class="no-login-toggle"
          @click="showNoLogin = !showNoLogin"
        >
          <span class="chev">{{ showNoLogin ? "▾" : "▸" }}</span>
          {{ showNoLogin ? "Hide" : "Show" }} {{ row.count }}
          {{ row.count === 1 ? "person" : "people" }} without a Windows login
        </div>

        <div
          v-else-if="row.kind === 'right'"
          class="picker-item right-item"
          :class="{ selected: row.flatIndex === selectedIndex }"
          @click="row.right && selectRight(row.right)"
          @mouseenter="row.flatIndex !== undefined && (selectedIndex = row.flatIndex)"
        >
          <span v-if="row.badge" class="kbd">{{ row.badge }}</span>
          <span class="picker-name">{{ row.right?.rightName }}</span>
          <span class="picker-id">#{{ row.right?.rightId }}</span>
          <button
            v-if="row.isRecent && row.right"
            class="remove-btn"
            title="Remove from recent (Del)"
            @click.stop="removeRecentRight(row.right.rightId)"
          >
            <svg viewBox="0 0 16 16" fill="currentColor" width="12" height="12">
              <path d="M3.72 3.72a.75.75 0 0 1 1.06 0L8 6.94l3.22-3.22a.75.75 0 1 1 1.06 1.06L9.06 8l3.22 3.22a.75.75 0 1 1-1.06 1.06L8 9.06l-3.22 3.22a.75.75 0 0 1-1.06-1.06L6.94 8 3.72 4.78a.75.75 0 0 1 0-1.06Z" />
            </svg>
          </button>
        </div>

        <div
          v-else
          class="picker-item"
          :class="{
            selected: row.flatIndex !== undefined && row.flatIndex === selectedIndex,
            'no-login': row.noLogin,
          }"
          @click="row.username && selectPerson(row.username, row.roleLabel ?? 'Custom')"
          @mouseenter="row.flatIndex !== undefined && (selectedIndex = row.flatIndex)"
        >
          <span v-if="row.badge" class="kbd">{{ row.badge }}</span>
          <span class="picker-name">{{ row.primary }}</span>
          <span v-if="row.secondary" class="picker-labels">{{ row.secondary }}</span>
          <span v-if="row.noLogin" class="custom-badge">no login</span>
          <button
            v-if="row.assoc"
            class="rights-btn"
            title="View this person's rights (Tab)"
            @click.stop="row.assoc && emit('viewRights', row.assoc)"
          >
            rights
          </button>
          <button
            v-if="row.isRecentUser && row.username && row.roleLabel"
            class="rights-btn"
            title="Pin to favorites"
            @click.stop="row.username && row.roleLabel && startPin(row.username, row.roleLabel)"
          >
            pin
          </button>
          <button
            v-if="row.isFavoriteRow && row.username && row.roleLabel"
            class="remove-btn"
            title="Remove from favorites (Del)"
            @click.stop="row.roleLabel && row.username && emit('removeFavorite', row.roleLabel, row.username)"
          >
            <svg viewBox="0 0 16 16" fill="currentColor" width="12" height="12">
              <path d="M3.72 3.72a.75.75 0 0 1 1.06 0L8 6.94l3.22-3.22a.75.75 0 1 1 1.06 1.06L9.06 8l3.22 3.22a.75.75 0 1 1-1.06 1.06L8 9.06l-3.22 3.22a.75.75 0 0 1-1.06-1.06L6.94 8 3.72 4.78a.75.75 0 0 1 0-1.06Z" />
            </svg>
          </button>
          <button
            v-else-if="row.isRecentUser && row.username && row.roleLabel"
            class="remove-btn"
            title="Dismiss from recently used (Del)"
            @click.stop="row.roleLabel && row.username && emit('removeRecent', row.roleLabel, row.username)"
          >
            <svg viewBox="0 0 16 16" fill="currentColor" width="12" height="12">
              <path d="M3.72 3.72a.75.75 0 0 1 1.06 0L8 6.94l3.22-3.22a.75.75 0 1 1 1.06 1.06L9.06 8l3.22 3.22a.75.75 0 1 1-1.06 1.06L8 9.06l-3.22 3.22a.75.75 0 0 1-1.06-1.06L6.94 8 3.72 4.78a.75.75 0 0 1 0-1.06Z" />
            </svg>
          </button>
        </div>
      </template>
    </div>
  </template>
</template>

<style scoped>
@import "./picker-shared.css";

.segmented {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0 16px 10px;
}

.seg {
  padding: 3px 12px;
  border: 1px solid var(--border-subtle);
  background: transparent;
  color: var(--text-secondary);
  border-radius: var(--radius-sm);
  font-size: 11px;
  font-family: var(--font-sans);
  cursor: pointer;
  transition: border-color 0.15s ease, color 0.15s ease, background 0.15s ease;
}

.seg.active {
  border-color: var(--accent-blue);
  color: var(--text-primary);
  background: var(--bg-selected);
}

.seg-hint {
  margin-left: auto;
  font-size: 10px;
  color: var(--text-secondary);
  opacity: 0.7;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

.picker-item {
  justify-content: flex-start;
}

.kbd {
  flex-shrink: 0;
  min-width: 16px;
  height: 16px;
  padding: 0 3px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--border-subtle);
  border-radius: 3px;
  font-size: 10px;
  font-family: var(--font-mono);
  color: var(--text-secondary);
}

.picker-labels {
  margin-left: auto;
}

.right-item .picker-id {
  font-size: 12px;
  font-family: var(--font-mono);
  color: var(--text-secondary);
  margin-left: auto;
}

.rights-btn {
  flex-shrink: 0;
  padding: 1px 8px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-secondary);
  font-size: 10px;
  font-family: var(--font-sans);
  cursor: pointer;
  opacity: 0;
  transition: opacity 0.1s ease, border-color 0.15s ease, color 0.15s ease;
}

.picker-item:hover .rights-btn,
.picker-item.selected .rights-btn {
  opacity: 1;
}

.rights-btn:hover {
  border-color: var(--text-secondary);
  color: var(--text-primary);
}

.no-login {
  opacity: 0.5;
  cursor: default;
}

.no-login:hover {
  background: transparent;
}

.no-login-toggle {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px 16px;
  font-size: 11px;
  color: var(--text-secondary);
  cursor: pointer;
  user-select: none;
  transition: color 0.15s ease;
}

.no-login-toggle:hover {
  color: var(--text-primary);
}

.chev {
  font-size: 10px;
  opacity: 0.7;
}

.searching-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 16px;
  color: var(--text-secondary);
  font-size: 13px;
}

.mini-spinner {
  width: 14px;
  height: 14px;
  border: 2px solid var(--border-subtle);
  border-top-color: var(--accent-blue);
  border-radius: 50%;
  animation: spin 0.6s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
