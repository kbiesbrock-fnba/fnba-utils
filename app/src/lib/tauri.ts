export interface IdentityUser {
  username: string;
  labels: string;
}

export interface IdentityData {
  imposter: string;
  users: IdentityUser[];
  connections: string[];
}

export interface IdentityState {
  acting_as_login: string;
  acting_as_name: string;
  password: string;
  changed_at: string;
  on_host: string;
}

export interface AssumeIdentityResult {
  server: string;
  login: string;
  before: IdentityState | null;
  after: IdentityState | null;
  password_changed: boolean;
  already_assuming: boolean;
  message: string | null;
}

// Detect if running inside Tauri (window.__TAURI_INTERNALS__ exists)
const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri) {
    const { invoke: tauriInvoke } = await import("@tauri-apps/api/core");
    return tauriInvoke<T>(cmd, args);
  }
  return mockInvoke<T>(cmd, args);
}

// --- Mock layer for browser development ---

import identityDefaults from "../../src-tauri/data/identity-defaults.json";

function buildUserMap(
  users: Array<{ label: string; username: string }>,
): IdentityUser[] {
  const map = new Map<string, string[]>();
  for (const u of users) {
    const labels = map.get(u.username) ?? [];
    if (u.label && !labels.includes(u.label)) labels.push(u.label);
    map.set(u.username, labels);
  }
  return Array.from(map.entries()).map(([username, labels]) => ({
    username,
    labels: labels.join(" | "),
  }));
}

async function mockInvoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  // Simulate network latency
  const delay = (ms: number) => new Promise((r) => setTimeout(r, ms));

  switch (cmd) {
    case "get_identity_data": {
      await delay(100);
      return {
        imposter: identityDefaults.imposter,
        users: buildUserMap(identityDefaults.defaultUsers),
        connections: identityDefaults.defaultConnections,
      } as T;
    }

    case "execute_assume_identity": {
      const user = (args?.user as string) ?? "unknown";
      const connection = (args?.connection as string) ?? "unknown";
      await delay(1500); // Simulate SQL round-trip
      return {
        server: connection,
        login: `FNBA\\${identityDefaults.imposter}`,
        before: {
          acting_as_login: `FNBA\\${identityDefaults.imposter}`,
          acting_as_name: "self",
          password: "OldP@ss123",
          changed_at: "09:15:22 03-25-2026",
          on_host: connection.split(".")[0].toUpperCase(),
        },
        after: {
          acting_as_login: user.includes("\\") ? user : `FNBA\\${user}`,
          acting_as_name: `${user} (mock)`,
          password: "NewP@ss456",
          changed_at: new Date().toLocaleTimeString("en-US", { hour12: false }) + " 03-25-2026",
          on_host: connection.split(".")[0].toUpperCase(),
        },
        password_changed: true,
        already_assuming: false,
        message: "Identity switched successfully.",
      } as T;
    }

    case "hide_window": {
      console.log("[mock] hide_window — no-op in browser");
      return undefined as T;
    }

    default:
      throw new Error(`[mock] Unknown command: ${cmd}`);
  }
}

// --- Public API (same interface whether Tauri or browser) ---

export function getIdentityData(): Promise<IdentityData> {
  return invoke<IdentityData>("get_identity_data");
}

export function executeAssumeIdentity(
  user: string,
  connection: string,
): Promise<AssumeIdentityResult> {
  return invoke<AssumeIdentityResult>("execute_assume_identity", {
    user,
    connection,
  });
}

export function hideWindow(): Promise<void> {
  return invoke<void>("hide_window");
}
