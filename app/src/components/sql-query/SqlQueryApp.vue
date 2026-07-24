<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { useSqlQuery } from "@/composables/useSqlQuery";
import PinButton from "@/components/common/PinButton.vue";

const {
  server,
  label,
  sql,
  database,
  result,
  error,
  running,
  groups,
  groupedQueries,
  pinned,
  runQuery,
  cancelQuery,
  saveQuery,
  deleteQuery,
  loadQuery,
  moveQuery,
  createGroup,
  renameGroup,
  renameQuery,
  deleteGroup,
  toggleGroupPin,
  isCollapsed,
  toggleCollapsed,
  togglePin,
  closeWindow,
} = useSqlQuery();

// ---- Save dialog state ----
const showSaveInput = ref(false);
const saveName = ref("");
// "" = Ungrouped; "__new__" = create new group; otherwise existing group id
const saveGroupId = ref<string>("");
const newGroupName = ref("");

async function onSave() {
  const trimmedName = saveName.value.trim();
  if (!trimmedName || !sql.value.trim()) return;
  let targetGroupId: string | null = null;
  if (saveGroupId.value === "__new__") {
    const created = await createGroup(newGroupName.value.trim() || "New group");
    if (!created) return;
    targetGroupId = created.id;
  } else if (saveGroupId.value !== "") {
    targetGroupId = saveGroupId.value;
  }
  await saveQuery(trimmedName, targetGroupId);
  cancelSave();
}

function cancelSave() {
  showSaveInput.value = false;
  saveName.value = "";
  newGroupName.value = "";
  saveGroupId.value = "";
}

// ---- Floating menu state (Teleport'd) ----
type MenuKind = "query" | "group";
interface MenuState {
  kind: MenuKind;
  id: string;
  anchor: { left: number; top: number };
}
const menu = ref<MenuState | null>(null);

function openMenuAt(kind: MenuKind, id: string, ev: Event) {
  const trigger = ev.currentTarget as HTMLElement;
  const rect = trigger.getBoundingClientRect();
  menu.value = {
    kind,
    id,
    anchor: { left: Math.round(rect.right - 4), top: Math.round(rect.bottom + 4) },
  };
}

function closeMenu() {
  menu.value = null;
}

const menuQuery = computed(() =>
  menu.value?.kind === "query"
    ? groupedQueries.value
        .flatMap((s) => s.queries)
        .find((q) => q.id === menu.value!.id) ?? null
    : null,
);

const menuGroup = computed(() =>
  menu.value?.kind === "group"
    ? groups.value.find((g) => g.id === menu.value!.id) ?? null
    : null,
);

// ---- Inline rename state ----
interface RenameState {
  kind: MenuKind;
  id: string;
  /** Element to refocus when rename ends. */
  rowEl: HTMLElement | null;
}
const rename = ref<RenameState | null>(null);
const renameDraft = ref("");
const renameInput = ref<HTMLInputElement | null>(null);

async function startRename(kind: MenuKind, id: string, currentName: string, rowEl?: HTMLElement | null) {
  closeMenu();
  rename.value = {
    kind,
    id,
    rowEl: rowEl ?? (document.activeElement instanceof HTMLElement ? document.activeElement : null),
  };
  renameDraft.value = currentName;
  await nextTick();
  renameInput.value?.focus();
  renameInput.value?.select();
}

async function commitRename() {
  if (!rename.value) return;
  const { kind, id, rowEl } = rename.value;
  const name = renameDraft.value.trim();
  if (name) {
    if (kind === "query") await renameQuery(id, name);
    else await renameGroup(id, name);
  }
  rename.value = null;
  renameDraft.value = "";
  await nextTick();
  rowEl?.focus();
}

function cancelRename() {
  const rowEl = rename.value?.rowEl ?? null;
  rename.value = null;
  renameDraft.value = "";
  nextTick().then(() => rowEl?.focus());
}

// ---- Menu actions ----
async function menuMoveTo(groupId: string | null) {
  if (!menuQuery.value) return;
  await moveQuery(menuQuery.value.id, groupId);
  closeMenu();
}

async function menuMoveToNewGroup() {
  if (!menuQuery.value) return;
  const name = window.prompt("New group name:");
  closeMenu();
  if (!name || !name.trim()) return;
  const created = await createGroup(name.trim());
  if (created && menuQuery.value) {
    await moveQuery(menuQuery.value.id, created.id);
  }
}

async function menuDeleteQuery() {
  if (!menuQuery.value) return;
  const ok = window.confirm(`Delete query "${menuQuery.value.name}"?`);
  closeMenu();
  if (ok) await deleteQuery(menuQuery.value.id);
}

async function menuDeleteGroup() {
  if (!menuGroup.value) return;
  const ok = window.confirm(
    `Delete group "${menuGroup.value.name}"? Its queries will be moved to Ungrouped.`,
  );
  closeMenu();
  if (ok) await deleteGroup(menuGroup.value.id);
}

async function menuTogglePin() {
  if (!menuGroup.value) return;
  await toggleGroupPin(menuGroup.value.id);
  closeMenu();
}

// ---- Keyboard navigation across rows ----
function focusableRows(): HTMLElement[] {
  const root = document.querySelector(".sq-saved-list");
  if (!root) return [];
  return Array.from(root.querySelectorAll<HTMLElement>(".sq-row[tabindex]"));
}

function moveFocus(currentEl: HTMLElement, delta: 1 | -1) {
  const rows = focusableRows();
  const idx = rows.indexOf(currentEl);
  if (idx === -1 || rows.length === 0) return;
  const next = rows[(idx + delta + rows.length) % rows.length];
  next?.focus();
}

function onQueryRowKeydown(e: KeyboardEvent, queryId: string, queryName: string) {
  const rowEl = e.currentTarget as HTMLElement;
  if (e.key === "Enter") {
    e.preventDefault();
    loadQuery(queryId);
  } else if (e.key === "F2") {
    e.preventDefault();
    startRename("query", queryId, queryName, rowEl);
  } else if (e.key === "Delete") {
    e.preventDefault();
    const ok = window.confirm(`Delete query "${queryName}"?`);
    if (ok) deleteQuery(queryId);
  } else if (e.key === "ArrowDown") {
    e.preventDefault();
    moveFocus(rowEl, 1);
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    moveFocus(rowEl, -1);
  }
}

function onGroupRowKeydown(
  e: KeyboardEvent,
  groupId: string | null,
  groupName: string,
) {
  const rowEl = e.currentTarget as HTMLElement;
  if (e.key === "Enter" || e.key === " ") {
    e.preventDefault();
    toggleCollapsed(groupId);
  } else if (e.key === "F2" && groupId) {
    e.preventDefault();
    startRename("group", groupId, groupName, rowEl);
  } else if (e.key === "Delete" && groupId) {
    e.preventDefault();
    const ok = window.confirm(
      `Delete group "${groupName}"? Its queries will be moved to Ungrouped.`,
    );
    if (ok) deleteGroup(groupId);
  } else if (e.key === "ArrowDown") {
    e.preventDefault();
    moveFocus(rowEl, 1);
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    moveFocus(rowEl, -1);
  }
}

// ---- Editor keyboard ----
function onEditorKeydown(e: KeyboardEvent) {
  if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
    e.preventDefault();
    runQuery();
  }
  if (e.key === "Escape") {
    e.preventDefault();
    closeWindow();
  }
}

// ---- Global Escape: cascade through layers ----
function onGlobalKeydown(e: KeyboardEvent) {
  if (e.key !== "Escape") return;
  if (menu.value) {
    e.preventDefault();
    closeMenu();
    return;
  }
  if (rename.value) {
    e.preventDefault();
    cancelRename();
    return;
  }
  if (showSaveInput.value) {
    e.preventDefault();
    cancelSave();
    return;
  }
  // Otherwise let editor / row Escape handlers run.
}

// ---- Click-outside to close menu ----
function onDocumentClick(e: MouseEvent) {
  if (!menu.value) return;
  const target = e.target as HTMLElement | null;
  if (!target) return;
  if (target.closest(".sq-menu") || target.closest(".sq-menu-trigger")) return;
  closeMenu();
}

onMounted(() => {
  document.addEventListener("mousedown", onDocumentClick);
  document.addEventListener("keydown", onGlobalKeydown);
});
onBeforeUnmount(() => {
  document.removeEventListener("mousedown", onDocumentClick);
  document.removeEventListener("keydown", onGlobalKeydown);
});

// ---- Misc derived state ----
const totalSavedCount = computed(() =>
  groupedQueries.value.reduce((acc, section) => acc + section.queries.length, 0),
);
</script>

<template>
  <div class="sq-app">
    <div v-if="!server" class="sq-empty">Click a connection in Mission Control</div>
    <template v-else>
      <div class="sq-header">
        <span class="sq-server"><strong>{{ server.split('.')[0] }}</strong><span class="sq-domain">.{{ server.split('.').slice(1).join('.') }}</span></span>
        <span class="sq-badge">{{ label }}</span>
        <PinButton :pinned="pinned" @toggle="togglePin" />
        <input
          v-model="database"
          class="sq-db-input"
          placeholder="database (master)"
          spellcheck="false"
          @keydown="onEditorKeydown"
        />
      </div>
      <div class="sq-divider" />
      <div class="sq-body">
        <div class="sq-sidebar">
          <div class="sq-sidebar-title">Saved Queries</div>
          <div class="sq-saved-list">
            <div v-if="totalSavedCount === 0" class="sq-saved-empty">
              No saved queries
            </div>
            <div
              v-for="section in groupedQueries"
              :key="section.group ? section.group.id : '__ungrouped__'"
              class="sq-group"
            >
              <div
                class="sq-row sq-group-row"
                tabindex="0"
                @click="toggleCollapsed(section.group ? section.group.id : null)"
                @keydown="onGroupRowKeydown($event, section.group ? section.group.id : null, section.group ? section.group.name : 'Ungrouped')"
              >
                <span
                  class="sq-chevron"
                  :class="{ collapsed: isCollapsed(section.group ? section.group.id : null) }"
                >
                  <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" width="9" height="9">
                    <path d="M3 4l3 3 3-3" />
                  </svg>
                </span>
                <span
                  v-if="section.group && section.group.color"
                  class="sq-color-dot"
                  :style="{ background: section.group.color }"
                />
                <template v-if="rename && rename.kind === 'group' && section.group && rename.id === section.group.id">
                  <input
                    ref="renameInput"
                    v-model="renameDraft"
                    class="sq-rename-input sq-rename-input--group"
                    spellcheck="false"
                    @click.stop
                    @keydown.enter.stop.prevent="commitRename"
                    @keydown.escape.stop.prevent="cancelRename"
                    @keydown.stop
                    @blur="commitRename"
                  />
                </template>
                <template v-else>
                  <span
                    class="sq-group-name"
                    :class="{ pinned: section.group?.pinned }"
                  >
                    {{ section.group ? section.group.name : "Ungrouped" }}
                  </span>
                </template>
                <span class="sq-count">{{ section.queries.length }}</span>
                <button
                  v-if="section.group"
                  class="sq-menu-trigger"
                  :class="{ active: menu?.kind === 'group' && menu.id === section.group.id }"
                  title="More actions (F2 to rename, Delete to remove)"
                  aria-label="Group actions"
                  @click.stop="openMenuAt('group', section.group.id, $event)"
                  @keydown.stop
                >
                  <svg viewBox="0 0 16 16" fill="currentColor" width="11" height="11">
                    <circle cx="3.5" cy="8" r="1.3" />
                    <circle cx="8" cy="8" r="1.3" />
                    <circle cx="12.5" cy="8" r="1.3" />
                  </svg>
                </button>
              </div>
              <div
                v-show="!isCollapsed(section.group ? section.group.id : null)"
                class="sq-group-body"
              >
                <div
                  v-for="q in section.queries"
                  :key="q.id"
                  class="sq-row sq-query-row"
                  tabindex="0"
                  :title="q.sql"
                  @click="loadQuery(q.id)"
                  @keydown="onQueryRowKeydown($event, q.id, q.name)"
                >
                  <template v-if="rename && rename.kind === 'query' && rename.id === q.id">
                    <input
                      ref="renameInput"
                      v-model="renameDraft"
                      class="sq-rename-input"
                      spellcheck="false"
                      @click.stop
                      @keydown.enter.stop.prevent="commitRename"
                      @keydown.escape.stop.prevent="cancelRename"
                      @keydown.stop
                      @blur="commitRename"
                    />
                  </template>
                  <template v-else>
                    <span class="sq-query-name">{{ q.name }}</span>
                  </template>
                  <button
                    class="sq-menu-trigger"
                    :class="{ active: menu?.kind === 'query' && menu.id === q.id }"
                    title="More actions (Enter to load, F2 to rename, Delete to remove)"
                    aria-label="Query actions"
                    @click.stop="openMenuAt('query', q.id, $event)"
                    @keydown.stop
                  >
                    <svg viewBox="0 0 16 16" fill="currentColor" width="11" height="11">
                      <circle cx="3.5" cy="8" r="1.3" />
                      <circle cx="8" cy="8" r="1.3" />
                      <circle cx="12.5" cy="8" r="1.3" />
                    </svg>
                  </button>
                </div>
              </div>
            </div>
          </div>
          <div class="sq-save-area">
            <div v-if="showSaveInput" class="sq-save-form" @click.stop>
              <input
                v-model="saveName"
                class="sq-save-input"
                placeholder="Query name..."
                spellcheck="false"
                autofocus
                @keydown.enter="onSave"
                @keydown.escape.stop="cancelSave"
              />
              <select v-model="saveGroupId" class="sq-save-group-select">
                <option value="">Ungrouped</option>
                <option v-for="g in groups" :key="g.id" :value="g.id">{{ g.name }}</option>
                <option value="__new__">+ New group...</option>
              </select>
              <input
                v-if="saveGroupId === '__new__'"
                v-model="newGroupName"
                class="sq-save-input"
                placeholder="New group name..."
                spellcheck="false"
                @keydown.enter="onSave"
                @keydown.escape.stop="cancelSave"
              />
              <div class="sq-save-form-actions">
                <button class="sq-save-cancel" @click="cancelSave">Cancel</button>
                <button class="sq-save-confirm" :disabled="!saveName.trim() || !sql.trim()" @click="onSave">Save</button>
              </div>
            </div>
            <button v-else class="sq-save-btn" :disabled="!sql.trim()" @click.stop="showSaveInput = true">+ Save current</button>
          </div>
        </div>
        <div class="sq-main">
          <div class="sq-editor">
            <textarea
              v-model="sql"
              class="sq-textarea"
              placeholder="SELECT TOP 10 * FROM ..."
              spellcheck="false"
              @keydown="onEditorKeydown"
            />
            <div class="sq-editor-footer">
              <span class="sq-hint">Ctrl+Enter to run</span>
              <button class="sq-run-btn" :disabled="running || !sql.trim()" @click="runQuery">
                {{ running ? "Running..." : "Run" }}
              </button>
              <button v-if="running" class="sq-cancel-btn" @click="cancelQuery">
                Cancel
              </button>
            </div>
          </div>
          <div v-if="error" class="sq-error">{{ error }}</div>
          <div v-if="result" class="sq-results">
            <div v-if="result.columns.length === 0" class="sq-results-empty">Query returned no results</div>
            <div v-else class="sq-table-wrap">
              <table class="sq-table">
                <thead>
                  <tr>
                    <th v-for="col in result.columns" :key="col">{{ col }}</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="(row, i) in result.rows" :key="i">
                    <td v-for="(cell, j) in row" :key="j" :title="cell">{{ cell }}</td>
                  </tr>
                </tbody>
              </table>
            </div>
            <div class="sq-row-count">{{ result.rowCount }} row{{ result.rowCount !== 1 ? "s" : "" }}</div>
          </div>
        </div>
      </div>
    </template>

    <!-- Floating action menu, escapes the sidebar's scroll/overflow box. -->
    <Teleport to="body">
      <div
        v-if="menu"
        class="sq-menu"
        role="menu"
        :style="{ left: menu.anchor.left + 'px', top: menu.anchor.top + 'px' }"
        @click.stop
      >
        <template v-if="menu.kind === 'query' && menuQuery">
          <button class="sq-menu-item" @click="loadQuery(menuQuery.id); closeMenu()">
            <span class="sq-menu-label">Load</span>
            <span class="sq-menu-shortcut">Enter</span>
          </button>
          <button class="sq-menu-item" @click="startRename('query', menuQuery.id, menuQuery.name)">
            <span class="sq-menu-label">Rename</span>
            <span class="sq-menu-shortcut">F2</span>
          </button>
          <div class="sq-menu-divider" />
          <div class="sq-menu-title">Move to</div>
          <button
            v-for="g in groups"
            :key="g.id"
            class="sq-menu-item"
            :class="{ current: menuQuery.groupId === g.id }"
            :disabled="menuQuery.groupId === g.id"
            @click="menuMoveTo(g.id)"
          >
            <span
              v-if="g.color"
              class="sq-color-dot small"
              :style="{ background: g.color }"
            />
            <span class="sq-menu-label">{{ g.name }}</span>
          </button>
          <button
            class="sq-menu-item"
            :class="{ current: menuQuery.groupId == null }"
            :disabled="menuQuery.groupId == null"
            @click="menuMoveTo(null)"
          >
            <span class="sq-menu-label">Ungrouped</span>
          </button>
          <button class="sq-menu-item" @click="menuMoveToNewGroup">
            <span class="sq-menu-label">+ New group...</span>
          </button>
          <div class="sq-menu-divider" />
          <button class="sq-menu-item danger" @click="menuDeleteQuery">
            <span class="sq-menu-label">Delete</span>
            <span class="sq-menu-shortcut">Del</span>
          </button>
        </template>
        <template v-else-if="menu.kind === 'group' && menuGroup">
          <button class="sq-menu-item" @click="startRename('group', menuGroup.id, menuGroup.name)">
            <span class="sq-menu-label">Rename group</span>
            <span class="sq-menu-shortcut">F2</span>
          </button>
          <button class="sq-menu-item" @click="menuTogglePin">
            <span class="sq-menu-label">{{ menuGroup.pinned ? "Unpin group" : "Pin to top" }}</span>
          </button>
          <div class="sq-menu-divider" />
          <button class="sq-menu-item danger" @click="menuDeleteGroup">
            <span class="sq-menu-label">Delete group</span>
            <span class="sq-menu-shortcut">Del</span>
          </button>
        </template>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.sq-app {
  width: 100%;
  height: 100vh;
  background: var(--bg-primary);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: var(--radius-lg);
  box-shadow:
    0 0 0 1px rgba(255, 255, 255, 0.06),
    0 0 20px rgba(96, 165, 250, 0.08),
    0 25px 50px -12px rgba(0, 0, 0, 0.6);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.sq-empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  color: var(--text-secondary);
}

.sq-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 14px;
  -webkit-app-region: drag;
}

.sq-server {
  font-size: 13px;
  font-weight: 400;
  color: var(--text-secondary);
}

.sq-server strong {
  font-weight: 600;
  color: var(--text-primary);
}

.sq-domain {
  font-size: 11px;
}

.sq-badge {
  font-size: 10px;
  padding: 0 5px;
  border-radius: 3px;
  font-weight: 500;
  background: rgba(255, 255, 255, 0.06);
  color: var(--text-secondary);
  line-height: 16px;
}

.sq-db-input {
  margin-left: auto;
  font-size: 11px;
  font-family: var(--font-mono);
  padding: 3px 8px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--bg-input);
  color: var(--text-primary);
  outline: none;
  width: 140px;
  -webkit-app-region: no-drag;
}

.sq-db-input:focus {
  border-color: var(--accent-blue);
}

.sq-db-input::placeholder {
  color: var(--text-placeholder);
}

.sq-divider {
  height: 1px;
  background: var(--border-subtle);
  flex-shrink: 0;
}

.sq-body {
  flex: 1;
  display: flex;
  min-height: 0;
}

/* --- Sidebar --- */

.sq-sidebar {
  width: 220px;
  flex-shrink: 0;
  border-right: 1px solid var(--border-subtle);
  display: flex;
  flex-direction: column;
}

.sq-sidebar-title {
  font-size: 10px;
  font-weight: 600;
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.5px;
  padding: 8px 10px 4px;
}

.sq-saved-list {
  flex: 1;
  overflow-y: auto;
  min-height: 0;
}

/* Shared row layout: flex so the name column takes whatever space is left,
   regardless of which other elements (chevron, count, color dot) are present
   on a given row kind. */
.sq-row {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 6px 4px 8px;
  cursor: pointer;
  user-select: none;
}

.sq-row:hover {
  background: var(--bg-hover);
}

.sq-row:focus {
  outline: none;
  background: var(--bg-hover);
  box-shadow: inset 2px 0 0 var(--accent-blue);
}

.sq-row:focus-visible {
  background: var(--bg-hover);
  box-shadow: inset 2px 0 0 var(--accent-blue);
}

.sq-group + .sq-group {
  margin-top: 2px;
}

/* Group header style */
.sq-group-row {
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.4px;
  color: var(--text-secondary);
}

.sq-chevron {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 12px;
  height: 12px;
  color: var(--text-secondary);
  transition: transform 0.1s ease;
}

.sq-chevron.collapsed {
  transform: rotate(-90deg);
}

.sq-color-dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
  box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.2) inset;
}

.sq-color-dot.small {
  width: 6px;
  height: 6px;
}

.sq-group-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-weight: 600;
  color: var(--text-primary);
  font-size: 10px;
}

.sq-group-name.pinned {
  color: var(--accent-blue);
}

.sq-count {
  font-size: 9px;
  padding: 0 5px;
  border-radius: 3px;
  background: rgba(255, 255, 255, 0.06);
  color: var(--text-secondary);
  line-height: 14px;
  flex-shrink: 0;
}

/* Query row style */
.sq-query-row {
  /* Indent so queries align under their group's name (past the chevron). */
  padding-left: 22px;
}

.sq-query-name {
  flex: 1;
  min-width: 0;
  font-size: 11px;
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* Menu trigger — always present at low opacity, brightens on row focus/hover. */
.sq-menu-trigger {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 18px;
  flex-shrink: 0;
  border: none;
  border-radius: 3px;
  background: transparent;
  color: var(--text-secondary);
  opacity: 0.35;
  cursor: pointer;
  padding: 0;
  transition: opacity 0.1s ease, background 0.1s ease;
}

.sq-row:hover .sq-menu-trigger,
.sq-row:focus .sq-menu-trigger,
.sq-row:focus-visible .sq-menu-trigger,
.sq-menu-trigger.active {
  opacity: 1;
}

.sq-menu-trigger:hover,
.sq-menu-trigger.active {
  background: rgba(255, 255, 255, 0.08);
  color: var(--text-primary);
}

/* Inline rename input — sits in place of the name span. */
.sq-rename-input {
  flex: 1;
  font-size: 11px;
  padding: 1px 5px;
  border: 1px solid var(--accent-blue);
  border-radius: var(--radius-sm);
  background: var(--bg-input);
  color: var(--text-primary);
  outline: none;
  min-width: 0;
}

.sq-rename-input--group {
  font-size: 10px;
  font-weight: 600;
  text-transform: none;
  letter-spacing: 0;
}

.sq-saved-empty {
  padding: 16px 10px;
  font-size: 10px;
  color: var(--text-placeholder);
  text-align: center;
}

/* --- Floating menu (teleported to body) --- */

.sq-menu {
  position: fixed;
  z-index: 2000;
  min-width: 180px;
  background: var(--bg-secondary);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  box-shadow: 0 10px 24px rgba(0, 0, 0, 0.5);
  padding: 4px 0;
  /* Anchor's right edge is `left`, so shift menu so its right edge meets it. */
  transform: translateX(-100%);
}

.sq-menu-title {
  font-size: 9px;
  font-weight: 600;
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.4px;
  padding: 6px 12px 2px;
}

.sq-menu-item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  text-align: left;
  font-size: 11px;
  padding: 6px 12px;
  border: none;
  background: transparent;
  color: var(--text-primary);
  cursor: pointer;
}

.sq-menu-item:hover:not(:disabled) {
  background: var(--bg-hover);
}

.sq-menu-item.current {
  color: var(--text-secondary);
}

.sq-menu-item:disabled {
  cursor: default;
  opacity: 0.6;
}

.sq-menu-item.danger {
  color: var(--accent-red);
}

.sq-menu-item.danger:hover:not(:disabled) {
  background: rgba(248, 113, 113, 0.12);
}

.sq-menu-label {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.sq-menu-shortcut {
  font-size: 9px;
  color: var(--text-placeholder);
  font-family: var(--font-mono);
  padding-left: 8px;
}

.sq-menu-divider {
  height: 1px;
  background: var(--border-subtle);
  margin: 4px 0;
}

/* --- Save form --- */

.sq-save-area {
  padding: 6px 10px;
  border-top: 1px solid var(--border-subtle);
}

.sq-save-btn {
  width: 100%;
  font-size: 10px;
  padding: 4px 0;
  border: 1px dashed var(--border-subtle);
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  transition: background 0.1s ease, color 0.1s ease;
}

.sq-save-btn:hover:not(:disabled) {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.sq-save-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.sq-save-form {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.sq-save-input {
  font-size: 10px;
  padding: 3px 6px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--bg-input);
  color: var(--text-primary);
  outline: none;
  min-width: 0;
}

.sq-save-input:focus {
  border-color: var(--accent-blue);
}

.sq-save-group-select {
  font-size: 10px;
  padding: 3px 6px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--bg-input);
  color: var(--text-primary);
  outline: none;
}

.sq-save-form-actions {
  display: flex;
  gap: 4px;
  justify-content: flex-end;
}

.sq-save-cancel {
  font-size: 10px;
  padding: 3px 8px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
}

.sq-save-cancel:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.sq-save-confirm {
  font-size: 10px;
  padding: 3px 8px;
  border: none;
  border-radius: var(--radius-sm);
  background: var(--accent-blue);
  color: #fff;
  cursor: pointer;
  font-weight: 500;
}

.sq-save-confirm:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

/* --- Main area --- */

.sq-main {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
}

.sq-editor {
  display: flex;
  flex-direction: column;
  padding: 10px;
  gap: 6px;
}

.sq-textarea {
  width: 100%;
  font-size: 11px;
  font-family: var(--font-mono);
  padding: 8px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--bg-input);
  color: var(--text-primary);
  outline: none;
  resize: vertical;
  min-height: 120px;
  max-height: 360px;
  line-height: 1.4;
  box-sizing: border-box;
}

.sq-textarea:focus {
  border-color: var(--accent-blue);
}

.sq-textarea::placeholder {
  color: var(--text-placeholder);
}

.sq-editor-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.sq-hint {
  font-size: 10px;
  color: var(--text-placeholder);
}

.sq-run-btn {
  font-size: 11px;
  padding: 4px 14px;
  border: none;
  border-radius: var(--radius-sm);
  background: var(--accent-blue);
  color: #fff;
  cursor: pointer;
  font-weight: 500;
}

.sq-run-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.sq-run-btn:not(:disabled):hover {
  filter: brightness(1.1);
}

.sq-cancel-btn {
  font-size: 11px;
  padding: 4px 14px;
  border: none;
  border-radius: var(--radius-sm);
  background: var(--accent-red);
  color: #fff;
  cursor: pointer;
  font-weight: 500;
  margin-left: 6px;
}

.sq-cancel-btn:hover {
  filter: brightness(1.1);
}

.sq-error {
  margin: 0 10px 8px;
  font-size: 11px;
  color: var(--accent-red);
  padding: 6px 8px;
  background: rgba(248, 113, 113, 0.08);
  border-radius: var(--radius-sm);
  word-break: break-word;
}

.sq-results {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
  padding: 0 10px 10px;
}

.sq-results-empty {
  font-size: 11px;
  color: var(--text-secondary);
  padding: 16px 0;
  text-align: center;
}

.sq-table-wrap {
  flex: 1;
  overflow: auto;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  min-height: 0;
}

.sq-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 10px;
  font-family: var(--font-mono);
}

.sq-table th {
  position: sticky;
  top: 0;
  background: var(--bg-secondary);
  color: var(--text-secondary);
  font-weight: 600;
  text-align: left;
  padding: 4px 8px;
  border-bottom: 1px solid var(--border-subtle);
  white-space: nowrap;
}

.sq-table td {
  padding: 3px 8px;
  color: var(--text-primary);
  border-bottom: 1px solid rgba(255, 255, 255, 0.04);
  max-width: 320px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.sq-table tbody tr:hover {
  background: var(--bg-hover);
}

.sq-row-count {
  margin-top: 6px;
  font-size: 10px;
  color: var(--text-placeholder);
  flex-shrink: 0;
}
</style>
