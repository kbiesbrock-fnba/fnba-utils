<script setup lang="ts">
import { ref, computed } from "vue";
import { useTestUsers, blankUser } from "@/composables/useTestUsers";
import type { TestUser, TestCard } from "@/lib/tauri";

const emit = defineEmits<{ (e: "close"): void }>();

const { users, loading, error, save, remove, setEnabled } = useTestUsers();

const editing = ref<TestUser | null>(null);
const isNew = computed(() => editing.value?.id == null);

function startNew() {
  editing.value = blankUser();
  editing.value.label = "New Test User";
}

function startEdit(u: TestUser) {
  // Clone so cancel doesn't mutate the list row.
  editing.value = JSON.parse(JSON.stringify(u));
}

function cancel() {
  editing.value = null;
}

async function commit() {
  if (!editing.value) return;
  if (!editing.value.label.trim()) {
    editing.value.label = "Unnamed";
  }
  const id = await save(editing.value);
  if (id != null) {
    editing.value = null;
  }
}

function addCard() {
  if (!editing.value) return;
  editing.value.cards.push({ number: "", expiry: "", cvv: "" });
}

function removeCard(idx: number) {
  if (!editing.value) return;
  editing.value.cards.splice(idx, 1);
}

async function onDelete(u: TestUser) {
  if (u.id == null) return;
  await remove(u.id);
}

function toggleEnabled(u: TestUser) {
  if (u.id == null) return;
  void setEnabled(u.id, !u.enabled);
}
</script>

<template>
  <div class="overlay" @mousedown.self="emit('close')">
    <section class="panel" @mousedown.stop>
      <header class="head">
        <div>
          <div class="title">Test Users</div>
          <div class="sub">
            Identities used to substitute detected PII on sensitive clipboard entries.
            One user is sticky-bound to each captured record at scan time.
          </div>
        </div>
        <button class="x" @click="emit('close')">×</button>
      </header>

      <div class="toolbar">
        <button class="btn primary" @click="startNew">+ New</button>
        <span v-if="loading" class="dim">Loading…</span>
        <span v-if="error" class="err">{{ error }}</span>
      </div>

      <ul v-if="!editing" class="list">
        <li v-for="u in users" :key="u.id ?? u.label" class="row" :class="{ off: !u.enabled }">
          <div class="row-main">
            <div class="row-label">{{ u.label }}</div>
            <div class="row-sub">
              <span v-if="u.ssn">SSN {{ u.ssn }}</span>
              <span v-if="u.dob">DOB {{ u.dob }}</span>
              <span v-if="u.email">{{ u.email }}</span>
              <span v-if="u.cards.length">{{ u.cards.length }} card{{ u.cards.length === 1 ? '' : 's' }}</span>
            </div>
          </div>
          <div class="row-actions">
            <button class="btn small" @click="toggleEnabled(u)">
              {{ u.enabled ? "Enabled" : "Disabled" }}
            </button>
            <button class="btn small" @click="startEdit(u)">Edit</button>
            <button class="btn small danger" @click="onDelete(u)">Delete</button>
          </div>
        </li>
        <li v-if="!users.length && !loading" class="empty">
          No test users yet. Add one to enable PII substitution.
        </li>
      </ul>

      <div v-else class="edit">
        <div class="edit-grid">
          <label>Label
            <input v-model="editing.label" placeholder="Test Alice Tester" />
          </label>
          <label>First name
            <input v-model="editing.firstName" placeholder="Alice" />
          </label>
          <label>Last name
            <input v-model="editing.lastName" placeholder="Tester" />
          </label>
          <label>SSN
            <input v-model="editing.ssn" placeholder="900-11-1111" />
          </label>
          <label>DOB
            <input v-model="editing.dob" placeholder="1990-01-15" />
          </label>
          <label>Email
            <input v-model="editing.email" placeholder="alice@test.fnba.local" />
          </label>
          <label>Phone
            <input v-model="editing.phone" placeholder="555-010-0001" />
          </label>
          <label>Address
            <input v-model="editing.address" placeholder="100 Test Lane" />
          </label>
          <label>Account #
            <input v-model="editing.accountNum" placeholder="100010000001" />
          </label>
          <label>Routing #
            <input v-model="editing.routingNum" placeholder="021000021" />
          </label>
        </div>

        <div class="cards-section">
          <div class="cards-header">
            <span>Cards</span>
            <button class="btn small" @click="addCard">+ Add card</button>
          </div>
          <div v-for="(card, i) in editing.cards" :key="i" class="card-row">
            <input v-model="card.number" placeholder="4242424242424242" />
            <input v-model="card.expiry" placeholder="12/29" class="narrow" />
            <input v-model="card.cvv" placeholder="123" class="narrow" />
            <button class="btn small danger" @click="removeCard(i)">×</button>
          </div>
          <div v-if="!editing.cards.length" class="dim">No cards.</div>
        </div>

        <label class="enabled-toggle">
          <input type="checkbox" v-model="editing.enabled" />
          Enabled (eligible for sticky-binding to new sensitive captures)
        </label>

        <footer class="actions">
          <button class="btn" @click="cancel">Cancel</button>
          <button class="btn primary" @click="commit">{{ isNew ? "Create" : "Save" }}</button>
        </footer>
      </div>
    </section>
  </div>
</template>

<style scoped>
.overlay {
  position: fixed;
  inset: 0;
  z-index: 50;
  background: rgba(8, 10, 16, 0.65);
  backdrop-filter: blur(2px);
  display: flex;
  align-items: stretch;
  justify-content: center;
  padding: 24px;
}
.panel {
  background: rgba(20, 24, 33, 0.98);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 10px;
  width: min(720px, 100%);
  max-height: 100%;
  display: flex;
  flex-direction: column;
  color: rgba(255, 255, 255, 0.92);
  overflow: hidden;
}
.head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  padding: 14px 18px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
}
.title {
  font-weight: 600;
  font-size: 14px;
}
.sub {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.55);
  margin-top: 2px;
  max-width: 540px;
  line-height: 1.4;
}
.x {
  background: transparent;
  border: none;
  color: rgba(255, 255, 255, 0.6);
  font-size: 16px;
  cursor: pointer;
  padding: 4px 8px;
  border-radius: 4px;
}
.x:hover {
  background: rgba(255, 255, 255, 0.08);
}

.toolbar {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 18px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
}
.dim {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.45);
}
.err {
  font-size: 11px;
  color: rgba(248, 113, 113, 0.95);
}

.list {
  list-style: none;
  margin: 0;
  padding: 6px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 4px;
  flex: 1;
}
.row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 12px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 6px;
}
.row.off {
  opacity: 0.45;
}
.row-main {
  min-width: 0;
  flex: 1;
}
.row-label {
  font-weight: 500;
  font-size: 13px;
}
.row-sub {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  font-size: 11px;
  color: rgba(255, 255, 255, 0.55);
  margin-top: 2px;
}
.row-actions {
  display: flex;
  gap: 4px;
}
.empty {
  text-align: center;
  padding: 24px;
  color: rgba(255, 255, 255, 0.5);
  font-size: 12px;
}

.edit {
  padding: 14px 18px;
  overflow-y: auto;
}
.edit-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px 14px;
}
.edit-grid label {
  display: flex;
  flex-direction: column;
  gap: 3px;
  font-size: 11px;
  color: rgba(255, 255, 255, 0.6);
  text-transform: uppercase;
  letter-spacing: 0.4px;
}
.edit-grid input,
.card-row input {
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.1);
  color: rgba(255, 255, 255, 0.95);
  padding: 6px 8px;
  border-radius: 4px;
  font-size: 12px;
  outline: none;
  text-transform: none;
  letter-spacing: 0;
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
}
.edit-grid input:focus,
.card-row input:focus {
  border-color: rgba(96, 165, 250, 0.6);
}

.cards-section {
  margin-top: 16px;
  padding-top: 12px;
  border-top: 1px solid rgba(255, 255, 255, 0.06);
}
.cards-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.4px;
  color: rgba(255, 255, 255, 0.6);
  margin-bottom: 6px;
}
.card-row {
  display: flex;
  gap: 6px;
  margin-bottom: 4px;
}
.card-row input {
  flex: 1;
}
.card-row input.narrow {
  flex: 0 0 80px;
}

.enabled-toggle {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: rgba(255, 255, 255, 0.85);
  margin-top: 14px;
}

.actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
  padding-top: 14px;
  margin-top: 14px;
  border-top: 1px solid rgba(255, 255, 255, 0.06);
}

.btn {
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.1);
  color: rgba(255, 255, 255, 0.9);
  padding: 5px 12px;
  border-radius: 4px;
  font-size: 12px;
  cursor: pointer;
}
.btn:hover {
  background: rgba(255, 255, 255, 0.1);
}
.btn.small {
  padding: 3px 8px;
  font-size: 11px;
}
.btn.primary {
  background: rgba(96, 165, 250, 0.2);
  border-color: rgba(96, 165, 250, 0.5);
  color: rgba(220, 235, 255, 0.95);
}
.btn.primary:hover {
  background: rgba(96, 165, 250, 0.3);
}
.btn.danger {
  color: rgba(248, 113, 113, 0.95);
  border-color: rgba(248, 113, 113, 0.25);
}
.btn.danger:hover {
  background: rgba(248, 113, 113, 0.15);
}
</style>
