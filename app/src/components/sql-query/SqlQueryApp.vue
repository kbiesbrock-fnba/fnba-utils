<script setup lang="ts">
import { ref } from "vue";
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
  savedQueries,
  pinned,
  runQuery,
  cancelQuery,
  saveQuery,
  removeQuery,
  loadQuery,
  togglePin,
  closeWindow,
} = useSqlQuery();

const saveName = ref("");
const showSaveInput = ref(false);

function onSave() {
  if (!saveName.value.trim()) return;
  saveQuery(saveName.value);
  saveName.value = "";
  showSaveInput.value = false;
}

function onKeydown(e: KeyboardEvent) {
  if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
    e.preventDefault();
    runQuery();
  }
  if (e.key === "Escape") {
    e.preventDefault();
    closeWindow();
  }
}
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
          @keydown="onKeydown"
        />
      </div>
      <div class="sq-divider" />
      <div class="sq-body">
        <div class="sq-sidebar">
          <div class="sq-sidebar-title">Saved Queries</div>
          <div class="sq-saved-list">
            <div
              v-for="(q, i) in savedQueries"
              :key="i"
              class="sq-saved-item"
              :title="q.sql"
              @click="loadQuery(i)"
            >
              <span class="sq-saved-name">{{ q.name }}</span>
              <button class="sq-saved-remove" title="Remove" @click.stop="removeQuery(i)">
                <svg viewBox="0 0 16 16" fill="currentColor" width="9" height="9">
                  <path d="M3.72 3.72a.75.75 0 0 1 1.06 0L8 6.94l3.22-3.22a.75.75 0 1 1 1.06 1.06L9.06 8l3.22 3.22a.75.75 0 1 1-1.06 1.06L8 9.06l-3.22 3.22a.75.75 0 0 1-1.06-1.06L6.94 8 3.72 4.78a.75.75 0 0 1 0-1.06z" />
                </svg>
              </button>
            </div>
            <div v-if="savedQueries.length === 0" class="sq-saved-empty">No saved queries</div>
          </div>
          <div class="sq-save-area">
            <div v-if="showSaveInput" class="sq-save-form">
              <input
                v-model="saveName"
                class="sq-save-input"
                placeholder="Query name..."
                spellcheck="false"
                @keydown.enter="onSave"
                @keydown.escape.stop="showSaveInput = false"
              />
              <button class="sq-save-confirm" :disabled="!saveName.trim() || !sql.trim()" @click="onSave">Save</button>
            </div>
            <button v-else class="sq-save-btn" :disabled="!sql.trim()" @click="showSaveInput = true">+ Save current</button>
          </div>
        </div>
        <div class="sq-main">
          <div class="sq-editor">
            <textarea
              v-model="sql"
              class="sq-textarea"
              placeholder="SELECT TOP 10 * FROM ..."
              spellcheck="false"
              @keydown="onKeydown"
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
  width: 160px;
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

.sq-saved-item {
  display: flex;
  align-items: center;
  padding: 5px 10px;
  cursor: pointer;
  transition: background 0.1s ease;
  gap: 4px;
}

.sq-saved-item:hover {
  background: var(--bg-hover);
}

.sq-saved-name {
  font-size: 11px;
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
  flex: 1;
}

.sq-saved-remove {
  display: none;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  border: none;
  background: transparent;
  color: var(--text-secondary);
  border-radius: 3px;
  cursor: pointer;
  flex-shrink: 0;
}

.sq-saved-item:hover .sq-saved-remove {
  display: flex;
}

.sq-saved-remove:hover {
  background: rgba(248, 113, 113, 0.15);
  color: var(--accent-red);
}

.sq-saved-empty {
  padding: 16px 10px;
  font-size: 10px;
  color: var(--text-placeholder);
  text-align: center;
}

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
  gap: 4px;
}

.sq-save-input {
  flex: 1;
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
  min-height: 64px;
  max-height: 200px;
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
  max-width: 200px;
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
