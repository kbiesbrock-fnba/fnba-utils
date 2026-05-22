import { ref, onMounted } from "vue";
import {
  deleteTestUser,
  listTestUsers,
  setTestUserEnabled,
  upsertTestUser,
  type TestUser,
} from "@/lib/tauri";

export function blankUser(): TestUser {
  return {
    id: null,
    label: "",
    firstName: null,
    lastName: null,
    ssn: null,
    dob: null,
    email: null,
    phone: null,
    address: null,
    accountNum: null,
    routingNum: null,
    cards: [],
    enabled: true,
  };
}

export function useTestUsers() {
  const users = ref<TestUser[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);

  async function load() {
    loading.value = true;
    try {
      users.value = await listTestUsers();
      error.value = null;
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e);
    } finally {
      loading.value = false;
    }
  }

  async function save(user: TestUser): Promise<number | null> {
    try {
      const id = await upsertTestUser(user);
      await load();
      return id;
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e);
      return null;
    }
  }

  async function remove(id: number) {
    try {
      await deleteTestUser(id);
      await load();
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e);
    }
  }

  async function setEnabled(id: number, enabled: boolean) {
    try {
      await setTestUserEnabled(id, enabled);
      const row = users.value.find((u) => u.id === id);
      if (row) row.enabled = enabled;
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e);
    }
  }

  onMounted(() => void load());

  return { users, loading, error, load, save, remove, setEnabled };
}
