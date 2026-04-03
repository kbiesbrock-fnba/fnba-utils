export interface IdentityUser {
  username: string;
  label: string;
  isCustom?: boolean;
}

export interface IdentityConnection {
  label: string;
  server: string;
  isCustom?: boolean;
}

export interface IdentityImposter {
  name: string;
  isCustom?: boolean;
}

export interface IdentityData {
  currentUser: string;
  imposters: IdentityImposter[];
  users: IdentityUser[];
  connections: IdentityConnection[];
}

export interface IdentityState {
  actingAsLogin: string;
  actingAsName: string;
  password: string;
  changedAt: string;
  onHost: string;
}

export interface AssumeIdentityResult {
  server: string;
  login: string;
  before: IdentityState | null;
  after: IdentityState | null;
  passwordChanged: boolean;
  alreadyAssuming: boolean;
  message: string | null;
}

export interface SaveCustomEntryResult {
  addedUser: boolean;
  addedConnection: boolean;
  addedImposter: boolean;
}

export interface DeleteCustomEntryResult {
  deletedUser: boolean;
  deletedConnection: boolean;
  deletedImposter: boolean;
}

export interface RightInfo {
  rightId: number;
  rightName: string;
}

export interface RightAssociate {
  assocId: number;
  nickname: string | null;
  firstName: string | null;
  lastName: string | null;
}

export interface SubagentInfo {
  agentType: string;
  description: string;
}

export interface ClaudeSession {
  pid: number;
  sessionId: string;
  cwd: string;
  startedAt: number;
  kind: string | null;
  name: string | null;
  entrypoint: string | null;
  isAlive: boolean;
  subagentCount: number;
  subagents: SubagentInfo[];
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

import identityDefaults from "../../../data/identity-defaults.json";

async function mockInvoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  // Simulate network latency
  const delay = (ms: number) => new Promise((r) => setTimeout(r, ms));

  switch (cmd) {
    case "get_identity_data": {
      await delay(100);
      const mockUser = "mockuser";
      return {
        currentUser: mockUser,
        imposters: [
          { name: mockUser },
          ...identityDefaults.imposters.map((i: string) => ({ name: i })),
          { name: "customImposter", isCustom: true },
        ],
        users: [
          ...identityDefaults.users,
          { username: "customuser1", label: "Other", isCustom: true },
          { username: "customuser2", label: "Custom Team", isCustom: true },
        ],
        connections: [
          ...identityDefaults.connections,
          { label: "Local", server: "custom-local.dev", isCustom: true },
        ],
      } as T;
    }

    case "execute_assume_identity": {
      const imposter = (args?.imposter as string) ?? "mockuser";
      const user = (args?.user as string) ?? "unknown";
      const connection = (args?.connection as string) ?? "unknown";
      await delay(1500); // Simulate SQL round-trip
      return {
        server: connection,
        login: `FNBA\\${imposter}`,
        before: {
          actingAsLogin: `FNBA\\${imposter}`,
          actingAsName: "self",
          password: "OldP@ss123",
          changedAt: "09:15:22 03-25-2026",
          onHost: connection.split(".")[0].toUpperCase(),
        },
        after: {
          actingAsLogin: user.includes("\\") ? user : `FNBA\\${user}`,
          actingAsName: `${user} (mock)`,
          password: "NewP@ss456",
          changedAt: new Date().toLocaleTimeString("en-US", { hour12: false }) + " 03-25-2026",
          onHost: connection.split(".")[0].toUpperCase(),
        },
        passwordChanged: true,
        alreadyAssuming: false,
        message: null,
      } as T;
    }

    case "save_custom_entry": {
      await delay(50);
      console.log("[mock] save_custom_entry", args);
      return { addedUser: !!args?.user, addedConnection: !!args?.connection, addedImposter: !!args?.imposter } as T;
    }

    case "delete_custom_entry": {
      await delay(50);
      console.log("[mock] delete_custom_entry", args);
      return { deletedUser: !!args?.user, deletedConnection: !!args?.connection, deletedImposter: !!args?.imposter } as T;
    }

    case "hide_window": {
      console.log("[mock] hide_window — no-op in browser");
      return undefined as T;
    }

    case "get_all_rights": {
      await delay(800);
      return [
        { rightId: 100, rightName: "Account Log Edit" },
        { rightId: 101, rightName: "Account Log View" },
        { rightId: 200, rightName: "Admin Panel" },
        { rightId: 300, rightName: "Billing Adjustments" },
        { rightId: 301, rightName: "Billing View" },
        { rightId: 400, rightName: "Customer Edit" },
        { rightId: 401, rightName: "Customer View" },
        { rightId: 500, rightName: "Dashboard Admin" },
        { rightId: 600, rightName: "Reports Export" },
        { rightId: 700, rightName: "User Management" },
      ] as T;
    }

    case "get_right_associates": {
      await delay(1500);
      return [
        { assocId: 1001, nickname: "jsmith", firstName: "John", lastName: "Smith" },
        { assocId: 1002, nickname: "jdoe", firstName: "Jane", lastName: "Doe" },
        { assocId: 1003, nickname: "mbrown", firstName: "Mike", lastName: "Brown" },
        { assocId: 1004, nickname: "agarcia", firstName: "Ana", lastName: "Garcia" },
        { assocId: 1005, nickname: null, firstName: null, lastName: null },
      ] as T;
    }

    case "search_associates": {
      const q = ((args?.query as string) ?? "").toLowerCase();
      await delay(600);
      const all = [
        { assocId: 1001, nickname: "jsmith", firstName: "John", lastName: "Smith" },
        { assocId: 1002, nickname: "jdoe", firstName: "Jane", lastName: "Doe" },
        { assocId: 1003, nickname: "mbrown", firstName: "Mike", lastName: "Brown" },
        { assocId: 1004, nickname: "agarcia", firstName: "Ana", lastName: "Garcia" },
        { assocId: 1006, nickname: "twilson", firstName: "Tom", lastName: "Wilson" },
      ];
      return all.filter(
        (a) =>
          a.nickname.includes(q) ||
          a.firstName.toLowerCase().includes(q) ||
          a.lastName.toLowerCase().includes(q),
      ) as T;
    }

    case "get_associate_rights": {
      await delay(1000);
      return [
        { rightId: 100, rightName: "Account Log Edit" },
        { rightId: 200, rightName: "Admin Panel" },
        { rightId: 401, rightName: "Customer View" },
      ] as T;
    }

    case "get_claude_sessions": {
      await delay(200);
      const now = Date.now();
      const sessions: ClaudeSession[] = [
        {
          pid: 12345,
          sessionId: "abc-123-def",
          cwd: "/mnt/c/dev/my-project",
          startedAt: now - 3600000,
          kind: "interactive",
          name: null,
          entrypoint: "cli",
          isAlive: true,
          subagentCount: 3,
          subagents: [
            { agentType: "Explore", description: "Explore codebase structure" },
            { agentType: "Explore", description: "Explore test patterns" },
            { agentType: "Plan", description: "Design auth feature" },
          ],
        },
        {
          pid: 67890,
          sessionId: "ghi-456-jkl",
          cwd: "/mnt/c/dev/other-project",
          startedAt: now - 900000,
          kind: "interactive",
          name: "refactor-auth",
          entrypoint: "cli",
          isAlive: true,
          subagentCount: 1,
          subagents: [
            { agentType: "Plan", description: "Plan migration strategy" },
          ],
        },
        ...(Math.random() > 0.5
          ? [
              {
                pid: 11111,
                sessionId: "mno-789-pqr",
                cwd: "/mnt/c/dev/docs",
                startedAt: now - 120000,
                kind: "interactive",
                name: "update-readme",
                entrypoint: "cli",
                isAlive: true,
                subagentCount: 0,
                subagents: [],
              } satisfies ClaudeSession,
            ]
          : []),
      ];
      return sessions as T;
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
  imposter: string,
  user: string,
  connection: string,
): Promise<AssumeIdentityResult> {
  return invoke<AssumeIdentityResult>("execute_assume_identity", {
    imposter,
    user,
    connection,
  });
}

export function saveCustomEntry(
  user?: string,
  userLabel?: string,
  connection?: string,
  connectionLabel?: string,
  imposter?: string,
): Promise<SaveCustomEntryResult> {
  return invoke<SaveCustomEntryResult>("save_custom_entry", {
    user: user ?? null,
    userLabel: userLabel ?? null,
    connection: connection ?? null,
    connectionLabel: connectionLabel ?? null,
    imposter: imposter ?? null,
  });
}

export function deleteCustomEntry(
  user?: string,
  connection?: string,
  imposter?: string,
): Promise<DeleteCustomEntryResult> {
  return invoke<DeleteCustomEntryResult>("delete_custom_entry", {
    user: user ?? null,
    connection: connection ?? null,
    imposter: imposter ?? null,
  });
}

export function hideWindow(): Promise<void> {
  return invoke<void>("hide_window");
}

export function getAllRights(server: string): Promise<RightInfo[]> {
  return invoke<RightInfo[]>("get_all_rights", { server });
}

export function getRightAssociates(
  server: string,
  rightName: string | null,
  rightId: number | null,
): Promise<RightAssociate[]> {
  return invoke<RightAssociate[]>("get_right_associates", {
    server,
    rightName,
    rightId,
  });
}

export function searchAssociates(server: string, query: string): Promise<RightAssociate[]> {
  return invoke<RightAssociate[]>("search_associates", { server, query });
}

export function getAssociateRights(server: string, assocId: number): Promise<RightInfo[]> {
  return invoke<RightInfo[]>("get_associate_rights", { server, assocId });
}

export function getClaudeSessions(): Promise<ClaudeSession[]> {
  return invoke<ClaudeSession[]>("get_claude_sessions");
}
