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
  /** Bare Windows username to assume (no `DOMAIN\`); null if no login row. */
  login: string | null;
  /** associate.job_title — preferred prefill for the favorite label. */
  jobTitle: string | null;
  /** department.name — fallback prefill when jobTitle is empty. */
  department: string | null;
}

export interface SubagentInfo {
  agentType: string;
  description: string;
}

export type SessionStatus = "idle" | "busy" | "dead" | "unknown";

/**
 * Classification of a row in Mission Control's session list.
 *   `mc`              — launched by MC (tmux name `claude-<uuid>`, in OwnedSessionsState)
 *   `claude-external` — tmux session whose foreground process is `claude` but
 *                       MC didn't spawn (e.g. user ran `claude` in IntelliJ)
 *   `tmux`            — any other tmux session (bash, vim, etc.)
 */
export type SessionSource = "mc" | "claude-external" | "tmux";

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
  status: SessionStatus;
  lastMessageAt: string | null;
  /** User-assigned friendly name (Feature #20). */
  label: string | null;
  /** If launched into a git worktree (Feature #7), the worktree path. */
  worktreePath: string | null;
  /** Where this row came from — MC-owned, external claude, or plain tmux. */
  source: SessionSource;
  /** tmux session name. For MC sessions this is `claude-<uuid>`. */
  tmuxSessionName: string;
  /** Active pane's `pane_current_command` (e.g. "claude", "vim", "bash"). */
  runningCommand: string | null;
  /** Active pane's `pane_current_path`. */
  currentPath: string | null;
  /** True if any tmux client is currently attached. */
  attached: boolean;
  /** Number of tmux windows in the session. */
  windowCount: number;
}

export interface ConversationMessage {
  role: string;
  timestamp: string;
  summary: string;
  toolName: string | null;
}

export interface SessionStats {
  messageCount: number;
  userMessageCount: number;
  assistantMessageCount: number;
  totalInputTokens: number;
  totalOutputTokens: number;
}

export interface SessionDetail {
  pid: number;
  sessionId: string;
  cwd: string;
  startedAt: number;
  kind: string | null;
  name: string | null;
  entrypoint: string | null;
  isAlive: boolean;
  gitBranch: string | null;
  status: SessionStatus;
  stats: SessionStats;
  recentMessages: ConversationMessage[];
  subagents: SubagentInfo[];
  label: string | null;
  worktreePath: string | null;
}

/** Returned by `start_new_claude_session`. */
export interface NewSessionInfo {
  sessionId: string;
  pid: number;
  jsonlPath: string;
  startedAt: number;
  cwd: string;
  worktreePath: string | null;
}

/** Entry in the persistent project registry (Wave 2). */
export interface Project {
  cwd: string;
  displayName: string;
  pinned: boolean;
  /** Unix epoch ms of the most recent launch, or 0 if never launched. */
  lastUsedAt: number;
  notes: string | null;
}

/** Historical session row (Wave 4 #26) — a session that has ended. */
export interface HistoricalSession {
  sessionId: string;
  cwd: string;
  pid: number;
  startedAt: number;
  endedAt: number | null;
  label: string | null;
  claudeHome: string;
  worktreePath: string | null;
  tmuxSession: string;
}

export interface ConnectionStatus {
  label: string;
  server: string;
  actingAsLogin: string | null;
  actingAsName: string | null;
  isSelf: boolean;
  error: string | null;
}

export interface QueryResult {
  columns: string[];
  rows: string[][];
  rowCount: number;
}

/** A user-created folder that organizes saved SQL queries. */
export interface SqlGroup {
  id: string;
  name: string;
  orderIdx: number;
  color: string | null;
  pinned: boolean;
}

/** A saved SQL query, optionally bound to a group via `groupId`. */
export interface SavedSqlQuery {
  id: string;
  name: string;
  sql: string;
  database: string;
  groupId: string | null;
  lastUsedAt: number;
  createdAt: number;
}

/** Shape of legacy localStorage entries handed to migrate_legacy_sql_queries. */
export interface LegacySavedSqlQuery {
  name: string;
  sql: string;
  database: string;
}

// --- Claude SDK (stream-json) session types ---

/**
 * Discriminated union of Claude Code stream-json events.
 * Unknown variants land in `Unknown` so the UI can stay forward-compatible
 * with new event shapes added by future Claude releases.
 */
export type ClaudeEvent =
  | { type: "system"; subtype?: string; [k: string]: unknown }
  | { type: "user"; message: { role: "user"; content: unknown }; [k: string]: unknown }
  | {
      type: "assistant";
      message: {
        role: "assistant";
        content: Array<
          | { type: "text"; text: string }
          | { type: "tool_use"; id: string; name: string; input: unknown }
          | { type: string; [k: string]: unknown }
        >;
        usage?: {
          input_tokens?: number;
          output_tokens?: number;
          cache_read_input_tokens?: number;
          cache_creation_input_tokens?: number;
        };
        [k: string]: unknown;
      };
      [k: string]: unknown;
    }
  | {
      type: "result";
      subtype?: "success" | "error" | string;
      duration_ms?: number;
      num_turns?: number;
      total_cost_usd?: number;
      [k: string]: unknown;
    }
  | { type: "stderr"; text: string }
  | { type: "raw"; text: string }
  | { type: "pty"; text: string }
  | { type: string; [k: string]: unknown };

export interface ClaudeEventEnvelope {
  sessionId: string;
  event: ClaudeEvent;
}

export interface ClaudeSessionClosedEvent {
  sessionId: string;
  exitCode: number;
}

// --- Standup ---

export interface StandupConfigView {
  enabled: boolean;
  hasCredentials: boolean;
  jiraDomain: string;
  teamsConfigured: boolean;
  configPath: string | null;
}

export interface AppConfigView {
  standup: StandupConfigView;
}

export type StandupGroupKey =
  | "in_progress"
  | "review"
  | "todo"
  | "attention"
  | "done";

export interface JiraIssue {
  key: string;
  summary: string;
  status: string;
  statusCategory: string;
  statusGroup: StandupGroupKey;
  storyPoints: number | null;
  url: string;
  priority: string | null;
  priorityRank: number; // lower = higher priority; 10 = unknown
  dueDate: string | null; // YYYY-MM-DD
  issueType: string;
  isBug: boolean;
  /** True when the Smart Checklist custom field has any non-empty content. */
  hasChecklist: boolean;
  /** Raw Smart Checklist field text (Smart Checklist syntax). */
  checklistText: string | null;
  /** Parsed checklist items derived from checklistText. */
  checklist: ChecklistItem[];
}

export interface StandupGroup {
  group: StandupGroupKey;
  label: string;
  emoji: string;
  issues: JiraIssue[];
  totalPoints: number;
}

export interface StandupReport {
  generatedAt: string;
  issueCount: number;
  groups: StandupGroup[];
}

export interface StandupRunResult {
  report: StandupReport;
  postedToTeams: boolean;
  copiedToClipboard: boolean;
  warnings: string[];
  /** Whether a Teams webhook is configured. Drives whether the Post button is enabled. */
  teamsConfigured: boolean;
  /** Channel deep-link to open after a successful post. Null means leave Teams alone. */
  teamsChannelUrl: string | null;
}

export interface StandupLastRun {
  at: string;
  issueCount: number;
  postedToTeams: boolean;
  error: string | null;
}

export interface StandupRunSummary {
  id: number;
  runAt: string;
  issueCount: number;
  postedToTeams: boolean;
  error: string | null;
}

export interface ChecklistItem {
  text: string;
  checked: boolean;
  isHeader: boolean;
}

export interface IssueDetail {
  key: string;
  url: string;
  summary: string;
  status: string;
  statusGroup: StandupGroupKey;
  priority: string | null;
  dueDate: string | null;
  storyPoints: number | null;
  issueType: string;
  isBug: boolean;
  assignee: string | null;
  reporter: string | null;
  labels: string[];
  description: string;
  spec: string | null;
  checklist: ChecklistItem[];
  checklistRaw: string | null;
  created: string | null;
  updated: string | null;
}

// --- Clipboard Manager ---

export type ClipboardKind = "text" | "html" | "image";

export type PiiKind =
  | "ssn"
  | "card"
  | "routing"
  | "account"
  | "dob"
  | "email"
  | "phone";

export interface ClipboardEntrySummary {
  id: number;
  kind: ClipboardKind;
  /** Obfuscated preview for sensitive rows; original for non-sensitive. */
  textPreview: string | null;
  thumbBase64: string | null;
  width: number | null;
  height: number | null;
  byteSize: number;
  sensitive: boolean;
  /** Detected PII categories ("ssn", "card", ...). Empty for non-sensitive rows. */
  piiKinds: PiiKind[];
  sourceProcess: string | null;
  capturedAt: number;
  pinned: boolean;
}

export interface ClipboardEntryFull {
  id: number;
  kind: ClipboardKind;
  /** Original captured text. Only crosses the bridge — never displayed
   *  directly for sensitive rows. */
  textContent: string | null;
  htmlContent: string | null;
  imageBase64: string | null;
  width: number | null;
  height: number | null;
  byteSize: number;
  sensitive: boolean;
  /** Test-user-substituted text. The default paste path uses this for
   *  sensitive entries — no reveal token needed. */
  obfuscatedText: string | null;
  /** Which Test User was used for substitution (NULL if no test users were
   *  defined at capture time and the mask-style fallback was used instead). */
  testUserId: number | null;
  piiKinds: PiiKind[];
  sourceProcess: string | null;
  capturedAt: number;
  pinned: boolean;
  contentHash: string;
}

export interface ClipboardSettings {
  textCap: number;
  imageCap: number;
  captureEnabled: boolean;
  ignoredProcesses: string[];
}

export interface ClipboardRevealToken {
  id: number;
  token: string;
  expiresInMs: number;
}

export interface ClipboardPasteOptions {
  simulatePaste: boolean;
  /** When true on a sensitive entry, paste the original (requires revealToken).
   *  When false/omitted, paste the obfuscated/substituted text. Ignored for
   *  non-sensitive entries. */
  pasteOriginal?: boolean;
  revealToken?: string;
}

// --- Test Users (PII substitution pool) ---

export interface TestCard {
  number: string;
  expiry: string;
  cvv: string;
}

export interface TestUser {
  id: number | null;
  label: string;
  firstName: string | null;
  lastName: string | null;
  ssn: string | null;
  dob: string | null;
  email: string | null;
  phone: string | null;
  address: string | null;
  accountNum: string | null;
  routingNum: string | null;
  cards: TestCard[];
  enabled: boolean;
}

export interface StandupPanelState {
  report: StandupReport | null;
  lastRun: StandupRunSummary | null;
  hiddenKeys: string[];
  /** Map of issue key -> manual order index (lower comes first). Missing keys have no manual override. */
  manualOrders: Record<string, number>;
  history: StandupRunSummary[];
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

// In-flight mock SQL queries, keyed by queryId. Each entry holds the timer handle
// and a reject() callback so the mock kill_sql_query can abort the pending query.
const mockSqlQueries = new Map<
  string,
  { timer: ReturnType<typeof setTimeout>; reject: (err: Error) => void }
>();

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

    case "open_app_data_folder": {
      console.log("[mock] open_app_data_folder — no-op in browser");
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
      await delay(1200);
      return [
        { assocId: 1001, nickname: "jsmith", firstName: "John", lastName: "Smith", login: "jsmith", jobTitle: "Underwriter", department: "Underwriting" },
        { assocId: 1002, nickname: "jdoe", firstName: "Jane", lastName: "Doe", login: "jdoe", jobTitle: "Processor", department: "Operations" },
        { assocId: 1003, nickname: "mbrown", firstName: "Mike", lastName: "Brown", login: "mbrown", jobTitle: "Accountant", department: "Accounting" },
        { assocId: 1004, nickname: "agarcia", firstName: "Ana", lastName: "Garcia", login: "agarcia", jobTitle: null, department: "Reporting" },
        { assocId: 1005, nickname: null, firstName: null, lastName: null, login: null, jobTitle: null, department: null },
      ] as T;
    }

    case "search_associates": {
      const q = ((args?.query as string) ?? "").toLowerCase();
      await delay(400);
      const all = [
        { assocId: 1001, nickname: "jsmith", firstName: "John", lastName: "Smith", login: "jsmith", jobTitle: "Underwriter", department: "Underwriting" },
        { assocId: 1002, nickname: "jdoe", firstName: "Jane", lastName: "Doe", login: "jdoe", jobTitle: "Processor", department: "Operations" },
        { assocId: 1003, nickname: "mbrown", firstName: "Mike", lastName: "Brown", login: "mbrown", jobTitle: "Accountant", department: "Accounting" },
        { assocId: 1004, nickname: "agarcia", firstName: "Ana", lastName: "Garcia", login: "agarcia", jobTitle: "Collections Specialist", department: "Collections" },
        { assocId: 1006, nickname: "twilson", firstName: "Tom", lastName: "Wilson", login: "twilson", jobTitle: "Operations Manager", department: "Operations" },
      ];
      return all.filter(
        (a) =>
          a.nickname.includes(q) ||
          a.firstName.toLowerCase().includes(q) ||
          a.lastName.toLowerCase().includes(q) ||
          a.login.includes(q),
      ) as T;
    }

    case "get_assume_login": {
      await delay(150);
      const id = args?.assocId as number;
      const map: Record<number, string> = {
        1001: "jsmith",
        1002: "jdoe",
        1003: "mbrown",
        1004: "agarcia",
        1006: "twilson",
      };
      return (map[id] ?? null) as T;
    }

    case "pin_favorite": {
      await delay(50);
      console.log("[mock] pin_favorite", args);
      return true as T;
    }

    case "remove_favorite": {
      await delay(50);
      console.log("[mock] remove_favorite", args);
      return undefined as T;
    }

    case "mark_favorite_used": {
      await delay(50);
      console.log("[mock] mark_favorite_used", args);
      return undefined as T;
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
          entrypoint: "mc",
          isAlive: true,
          subagentCount: 3,
          subagents: [
            { agentType: "Explore", description: "Explore codebase structure" },
            { agentType: "Explore", description: "Explore test patterns" },
            { agentType: "Plan", description: "Design auth feature" },
          ],
          status: "busy",
          lastMessageAt: new Date(now - 30000).toISOString(),
          label: null,
          worktreePath: null,
          source: "mc",
          tmuxSessionName: "claude-abc-123-def",
          runningCommand: "node",
          currentPath: "/mnt/c/dev/my-project",
          attached: true,
          windowCount: 1,
        },
        {
          pid: 67890,
          sessionId: "ghi-456-jkl",
          cwd: "/mnt/c/dev/other-project",
          startedAt: now - 900000,
          kind: "interactive",
          name: "refactor-auth",
          entrypoint: "mc",
          isAlive: true,
          subagentCount: 1,
          subagents: [
            { agentType: "Plan", description: "Plan migration strategy" },
          ],
          status: "idle",
          lastMessageAt: new Date(now - 120000).toISOString(),
          label: "refactor-auth",
          worktreePath: "/mnt/c/dev/other-project/.worktrees/abc12345",
          source: "mc",
          tmuxSessionName: "claude-ghi-456-jkl",
          runningCommand: "node",
          currentPath: "/mnt/c/dev/other-project/.worktrees/abc12345",
          attached: false,
          windowCount: 1,
        },
        {
          pid: 22001,
          sessionId: "tmux:fnba-utils",
          cwd: "/mnt/c/dev/fnba-utils",
          startedAt: now - 7200000,
          kind: null,
          name: null,
          entrypoint: null,
          isAlive: true,
          subagentCount: 0,
          subagents: [],
          status: "unknown",
          lastMessageAt: null,
          label: null,
          worktreePath: null,
          source: "claude-external",
          tmuxSessionName: "fnba-utils",
          runningCommand: "claude",
          currentPath: "/mnt/c/dev/fnba-utils",
          attached: true,
          windowCount: 2,
        },
        {
          pid: 22002,
          sessionId: "tmux:accounting",
          cwd: "/mnt/c/dev/accounting",
          startedAt: now - 14400000,
          kind: null,
          name: null,
          entrypoint: null,
          isAlive: true,
          subagentCount: 0,
          subagents: [],
          status: "unknown",
          lastMessageAt: null,
          label: null,
          worktreePath: null,
          source: "tmux",
          tmuxSessionName: "accounting",
          runningCommand: "vim",
          currentPath: "/mnt/c/dev/accounting/src",
          attached: false,
          windowCount: 1,
        },
        {
          pid: 22003,
          sessionId: "tmux:misc",
          cwd: "/home/kbiesbrock",
          startedAt: now - 1800000,
          kind: null,
          name: null,
          entrypoint: null,
          isAlive: true,
          subagentCount: 0,
          subagents: [],
          status: "unknown",
          lastMessageAt: null,
          label: null,
          worktreePath: null,
          source: "tmux",
          tmuxSessionName: "misc",
          runningCommand: "bash",
          currentPath: "/home/kbiesbrock",
          attached: false,
          windowCount: 1,
        },
      ];
      return sessions as T;
    }

    case "attach_tmux_session": {
      console.log("[mock] attach_tmux_session", args);
      const name = (args?.name as string) ?? "mock";
      const sid = `tmux:${name}`;
      window.dispatchEvent(
        new CustomEvent("mock-claude-event", {
          detail: {
            sessionId: sid,
            event: { type: "pty", text: `\r\n[mock] attached to tmux session '${name}'\r\n$ ` },
          },
        }),
      );
      return undefined as T;
    }

    case "get_connection_statuses": {
      await delay(1200);
      return [
        {
          label: "Local",
          server: "dsqlaleroy.fnba-dev.network",
          actingAsLogin: "FNBA\\mockuser",
          actingAsName: "self",
          isSelf: true,
          error: null,
        },
        {
          label: "Development",
          server: "meleagris.fnba.com",
          actingAsLogin: "FNBA\\ccollins",
          actingAsName: "Chris Collins",
          isSelf: false,
          error: null,
        },
        {
          label: "Staging",
          server: "caster.fnba.com",
          actingAsLogin: null,
          actingAsName: null,
          isSelf: false,
          error: "Connection to caster.fnba.com timed out after 8s",
        },
      ] as T;
    }

    case "execute_sql_query": {
      const queryId = (args?.queryId as string) ?? "";
      const q = ((args?.sql as string) ?? "").toLowerCase();
      // Longer delay if the query mentions "slow" or "waitfor" so the mock cancel is testable.
      const ms = q.includes("waitfor") || q.includes("slow") ? 30_000 : 800;
      try {
        await new Promise<void>((resolve, reject) => {
          const timer = setTimeout(() => {
            mockSqlQueries.delete(queryId);
            resolve();
          }, ms);
          if (queryId) mockSqlQueries.set(queryId, { timer, reject });
        });
      } catch (e) {
        if (queryId) mockSqlQueries.delete(queryId);
        throw e;
      }
      if (q.includes("error")) throw new Error("Mock SQL error: syntax error near 'error'");
      return {
        columns: ["id", "name", "status", "created_at"],
        rows: [
          ["1", "Alice", "active", "2026-01-15 09:30:00"],
          ["2", "Bob", "inactive", "2026-02-20 14:15:00"],
          ["3", "Charlie", "active", "2026-03-10 11:00:00"],
        ],
        rowCount: 3,
      } as T;
    }

    case "kill_sql_query": {
      const queryId = (args?.queryId as string) ?? "";
      const entry = mockSqlQueries.get(queryId);
      if (entry) {
        clearTimeout(entry.timer);
        mockSqlQueries.delete(queryId);
        entry.reject(new Error("Query was cancelled"));
      }
      return undefined as T;
    }

    case "get_session_detail": {
      await delay(300);
      const now = Date.now();
      return {
        pid: 12345,
        sessionId: (args as Record<string, unknown>)?.sessionId ?? "abc-123",
        cwd: "/mnt/c/dev/my-project",
        startedAt: now - 3600000,
        kind: "interactive",
        name: "refactor-auth",
        entrypoint: "mc",
        isAlive: true,
        gitBranch: "feature/auth-rework",
        status: "idle",
        label: null,
        worktreePath: null,
        stats: {
          messageCount: 47,
          userMessageCount: 22,
          assistantMessageCount: 25,
          totalInputTokens: 1250000,
          totalOutputTokens: 48000,
        },
        recentMessages: [
          {
            role: "user",
            timestamp: new Date(now - 300000).toISOString(),
            summary: "Can you refactor the auth middleware to use JWT?",
            toolName: null,
          },
          {
            role: "assistant",
            timestamp: new Date(now - 295000).toISOString(),
            summary: "[Read]",
            toolName: "Read",
          },
          {
            role: "assistant",
            timestamp: new Date(now - 280000).toISOString(),
            summary:
              "I see the current auth middleware uses session tokens. I'll refactor it to use JWT with refresh token rotation.",
            toolName: null,
          },
          {
            role: "assistant",
            timestamp: new Date(now - 260000).toISOString(),
            summary: "[Edit]",
            toolName: "Edit",
          },
          {
            role: "assistant",
            timestamp: new Date(now - 240000).toISOString(),
            summary:
              "Done. The middleware now validates JWTs and handles refresh token rotation automatically.",
            toolName: null,
          },
        ],
        subagents: [
          { agentType: "Explore", description: "Explore auth patterns" },
          { agentType: "Plan", description: "Design JWT migration" },
        ],
      } as T;
    }

    case "kill_session": {
      await delay(100);
      console.log("[mock] kill_session", args);
      return undefined as T;
    }

    case "start_claude_session": {
      console.log("[mock] start_claude_session", args);
      const sid = (args?.sessionId as string) ?? "mock-session";
      const emit = (event: ClaudeEvent) =>
        window.dispatchEvent(
          new CustomEvent("mock-claude-event", { detail: { sessionId: sid, event } }),
        );
      (async () => {
        await delay(200);
        emit({ type: "system", subtype: "init", session_id: sid, model: "claude-opus-4-7" });
        await delay(300);
        emit({
          type: "assistant",
          message: {
            role: "assistant",
            content: [{ type: "text", text: "Hi! Mock chat ready." }],
            usage: { input_tokens: 12, output_tokens: 5 },
          },
        });
        emit({ type: "result", subtype: "success", duration_ms: 500, num_turns: 1 });
      })();
      return undefined as T;
    }

    case "send_claude_message": {
      console.log("[mock] send_claude_message", args);
      const sid = (args?.sessionId as string) ?? "mock-session";
      const content = (args?.content as string) ?? "";
      (async () => {
        await delay(200);
        const reply: ClaudeEvent = {
          type: "assistant",
          message: {
            role: "assistant",
            content: [
              {
                type: "text",
                text: `Mock reply to: ${content.slice(0, 80)}`,
              },
            ],
            usage: { input_tokens: 24, output_tokens: 18 },
          },
        };
        window.dispatchEvent(
          new CustomEvent("mock-claude-event", { detail: { sessionId: sid, event: reply } }),
        );
        await delay(80);
        const result: ClaudeEvent = {
          type: "result",
          subtype: "success",
          duration_ms: 280,
          num_turns: 1,
        };
        window.dispatchEvent(
          new CustomEvent("mock-claude-event", { detail: { sessionId: sid, event: result } }),
        );
      })();
      return undefined as T;
    }

    case "stop_claude_session": {
      console.log("[mock] stop_claude_session", args);
      const sid = (args?.sessionId as string) ?? "mock-session";
      window.dispatchEvent(
        new CustomEvent("mock-claude-session-closed", {
          detail: { sessionId: sid, exitCode: 0 },
        }),
      );
      return undefined as T;
    }

    case "disconnect_session": {
      console.log("[mock] disconnect_session", args);
      return undefined as T;
    }

    case "open_in_explorer": {
      await delay(100);
      console.log("[mock] open_in_explorer", args);
      return undefined as T;
    }

    case "get_app_config": {
      await delay(20);
      // Mock with standup enabled so the command is visible in browser dev mode.
      return {
        standup: {
          enabled: true,
          hasCredentials: true,
          jiraDomain: "fnba.atlassian.net",
          teamsConfigured: true,
          configPath: "%LOCALAPPDATA%\\fnba-utils\\config.yaml",
        },
      } as T;
    }

    case "get_standup_last_run": {
      await delay(20);
      return {
        at: new Date(Date.now() - 1000 * 60 * 60 * 26).toISOString(),
        issueCount: 14,
        postedToTeams: true,
        error: null,
      } as T;
    }

    case "get_standup_report":
    case "run_standup":
    case "preview_standup":
    case "post_standup_to_teams": {
      await delay(400);
      const mockIssue = (
        key: string,
        summary: string,
        status: string,
        statusGroup: StandupGroupKey,
        storyPoints: number | null,
        opts: {
          priority?: string;
          dueDate?: string;
          issueType?: string;
          checklist?: ChecklistItem[];
          checklistText?: string;
        } = {},
      ): JiraIssue => {
        const issueType = opts.issueType ?? "Task";
        const priorityRank =
          opts.priority === "Highest"
            ? 1
            : opts.priority === "High"
              ? 2
              : opts.priority === "Medium"
                ? 3
                : opts.priority === "Low"
                  ? 4
                  : opts.priority === "Lowest"
                    ? 5
                    : 10;
        const checklist = opts.checklist ?? [];
        return {
          key,
          summary,
          status,
          statusCategory: statusGroup === "done" ? "done" : "indeterminate",
          statusGroup,
          storyPoints,
          url: `https://fnba.atlassian.net/browse/${key}`,
          priority: opts.priority ?? null,
          priorityRank,
          dueDate: opts.dueDate ?? null,
          issueType,
          isBug: issueType.toLowerCase() === "bug",
          hasChecklist: checklist.length > 0,
          checklistText: opts.checklistText ?? null,
          checklist,
        };
      };
      const sample: StandupReport = {
        generatedAt: new Date().toISOString(),
        issueCount: 5,
        groups: [
          {
            group: "in_progress",
            label: "In Progress",
            emoji: "💻",
            totalPoints: 8,
            issues: [
              mockIssue("MIN-1243", "Hot path fix for assumeIdentity", "Implement", "in_progress", 5, {
                priority: "High",
                issueType: "Bug",
                dueDate: "2026-05-22",
                checklist: [
                  { text: "Setup", checked: false, isHeader: true },
                  { text: "Reproduce locally", checked: true, isHeader: false },
                  { text: "Implementation", checked: false, isHeader: true },
                  { text: "Idempotency token in proc call", checked: false, isHeader: false },
                  { text: "Frontend guard while in-flight", checked: false, isHeader: false },
                ],
              }),
              mockIssue("MIN-1301", "Refactor permission cache", "Investigate", "in_progress", 3, {
                priority: "Medium",
              }),
            ],
          },
          {
            group: "review",
            label: "In Review",
            emoji: "🔍",
            totalPoints: 2,
            issues: [
              mockIssue("MIN-1199", "Right lookup recent pinning", "Ready to Review", "review", 2, {
                priority: "Medium",
              }),
            ],
          },
          {
            group: "done",
            label: "Done This Week",
            emoji: "✅",
            totalPoints: 5,
            issues: [
              mockIssue("MIN-1180", "Mission Control parallel resume", "Done", "done", 5),
            ],
          },
        ],
      };
      if (cmd === "get_standup_report") {
        return sample as T;
      }
      // Toggle this to dev the "missing channel URL" hint in the browser:
      const mockTeamsConfigured = true;
      const mockChannelUrl =
        "https://teams.microsoft.com/l/channel/19%3aexample%40thread.tacv2/Standup";
      let postedToTeams: boolean;
      let reportForResult: StandupReport;
      if (cmd === "preview_standup") {
        postedToTeams = false;
        reportForResult = sample;
      } else if (cmd === "post_standup_to_teams") {
        postedToTeams = true;
        // Echo back the report the frontend passed in, so generatedAt round-trips
        // correctly (the real backend posts exactly what was previewed).
        reportForResult =
          (args && (args.report as StandupReport | undefined)) ?? sample;
        // Pretend Teams was opened.
        console.info("[mock] openExternal:", mockChannelUrl);
      } else {
        // legacy run_standup path
        postedToTeams = !!(args && (args.postToTeamsFlag as boolean));
        reportForResult = sample;
      }
      return {
        report: reportForResult,
        postedToTeams,
        // Auto-copy was removed in v1.13.2 — users click the Copy button
        // instead, which routes through "copy_standup_report" below.
        copiedToClipboard: false,
        warnings: [],
        teamsConfigured: mockTeamsConfigured,
        teamsChannelUrl: mockTeamsConfigured ? mockChannelUrl : null,
      } as T;
    }

    case "copy_standup_report": {
      await delay(20);
      const report = (args?.report as StandupReport | undefined) ?? null;
      const text = report
        ? report.groups
            .filter((g) => g.group !== "attention")
            .map((g) => {
              const head = `${g.emoji} ${g.label} (${g.issues.length})`;
              const rows = g.issues
                .map(
                  (i) =>
                    `  [${i.key}] ${i.summary}: ${i.status} (${
                      i.storyPoints ?? "—"
                    })`,
                )
                .join("\n");
              return `${head}\n${rows}`;
            })
            .join("\n\n") + "\n"
        : "";
      console.info("[mock] copy_standup_report ->", text);
      return text as T;
    }

    case "get_standup_panel_state": {
      await delay(60);
      const now = new Date();
      const yesterday = new Date(now.getTime() - 1000 * 60 * 60 * 26);
      const pIssue = (
        key: string,
        summary: string,
        status: string,
        statusGroup: StandupGroupKey,
        storyPoints: number | null,
        opts: {
          priority?: string;
          dueDate?: string;
          issueType?: string;
          checklist?: ChecklistItem[];
        } = {},
      ): JiraIssue => {
        const issueType = opts.issueType ?? "Task";
        const priorityRank =
          opts.priority === "Highest"
            ? 1
            : opts.priority === "High"
              ? 2
              : opts.priority === "Medium"
                ? 3
                : opts.priority === "Low"
                  ? 4
                  : opts.priority === "Lowest"
                    ? 5
                    : 10;
        const checklist = opts.checklist ?? [];
        return {
          key,
          summary,
          status,
          statusCategory: statusGroup === "done" ? "done" : "indeterminate",
          statusGroup,
          storyPoints,
          url: `https://fnba.atlassian.net/browse/${key}`,
          priority: opts.priority ?? null,
          priorityRank,
          dueDate: opts.dueDate ?? null,
          issueType,
          isBug: issueType.toLowerCase() === "bug",
          hasChecklist: checklist.length > 0,
          checklistText: null,
          checklist,
        };
      };
      return {
        report: {
          generatedAt: yesterday.toISOString(),
          issueCount: 5,
          groups: [
            {
              group: "in_progress",
              label: "In Progress",
              emoji: "💻",
              totalPoints: 8,
              issues: [
                pIssue("MIN-1243", "Hot path fix for assumeIdentity", "Implement", "in_progress", 5, {
                  priority: "Highest",
                  issueType: "Bug",
                  dueDate: "2026-05-21",
                  checklist: [
                    { text: "Setup", checked: false, isHeader: true },
                    { text: "Reproduce locally", checked: true, isHeader: false },
                    { text: "Add integration test", checked: true, isHeader: false },
                    { text: "Implementation", checked: false, isHeader: true },
                    { text: "Idempotency token in proc call", checked: false, isHeader: false },
                    { text: "Frontend guard while in-flight", checked: false, isHeader: false },
                  ],
                }),
                pIssue("MIN-1301", "Refactor permission cache", "Investigate", "in_progress", 3, {
                  priority: "Medium",
                }),
              ],
            },
            {
              group: "review",
              label: "In Review",
              emoji: "🔍",
              totalPoints: 2,
              issues: [
                pIssue("MIN-1199", "Right lookup recent pinning", "Ready to Review", "review", 2, {
                  priority: "Low",
                }),
                pIssue("MIN-1310", "NPE on login when SSO times out", "Investigate", "review", 1, {
                  priority: "High",
                  issueType: "Bug",
                  dueDate: "2026-05-25",
                }),
              ],
            },
            {
              group: "done",
              label: "Done This Week",
              emoji: "✅",
              totalPoints: 5,
              issues: [
                pIssue("MIN-1180", "Mission Control parallel resume", "Done", "done", 5),
              ],
            },
          ],
        },
        lastRun: {
          id: 42,
          runAt: yesterday.toISOString(),
          issueCount: 5,
          postedToTeams: true,
          error: null,
        },
        hiddenKeys: [],
        manualOrders: {},
        history: [
          {
            id: 42,
            runAt: yesterday.toISOString(),
            issueCount: 3,
            postedToTeams: true,
            error: null,
          },
          {
            id: 41,
            runAt: new Date(now.getTime() - 1000 * 60 * 60 * 50).toISOString(),
            issueCount: 5,
            postedToTeams: true,
            error: null,
          },
        ],
      } as T;
    }

    case "set_issue_hidden":
    case "clear_hidden_issues":
    case "set_issue_order":
    case "clear_manual_order": {
      await delay(20);
      console.log(`[mock] ${cmd}`, args);
      return (cmd === "clear_hidden_issues" || cmd === "clear_manual_order"
        ? 0
        : undefined) as T;
    }

    case "get_run_snapshot": {
      await delay(40);
      console.log("[mock] get_run_snapshot", args);
      return null as T;
    }

    case "get_issue_detail": {
      await delay(200);
      const key = (args?.key as string) ?? "MIN-1243";
      return {
        key,
        url: `https://fnba.atlassian.net/browse/${key}`,
        summary: "Hot path fix for assumeIdentity",
        status: "Implement",
        statusGroup: "in_progress",
        priority: "Highest",
        dueDate: "2026-05-21",
        storyPoints: 5,
        issueType: "Bug",
        isBug: true,
        assignee: "Kevin Biesbrock",
        reporter: "QA Lead",
        labels: ["sql", "auth"],
        description:
          "Repro:\n  1. Open assume identity\n  2. Pick a connection\n  3. Submit twice quickly\n\nExpected: idempotent. Actual: second call fails with race on the login table.",
        spec:
          "Acceptance criteria:\n- A second submission while the first is in-flight must return the same result (no second proc call).\n- No new DB rows in the login table.\n- The UI shows a 'still running' indicator instead of becoming unresponsive.",
        checklist: [
          { text: "Setup", checked: false, isHeader: true },
          { text: "Reproduce locally with sample creds", checked: true, isHeader: false },
          { text: "Add integration test covering double-submit", checked: true, isHeader: false },
          { text: "Implementation", checked: false, isHeader: true },
          { text: "Idempotency token in proc call", checked: false, isHeader: false },
          { text: "Frontend guard while in-flight", checked: false, isHeader: false },
          { text: "Update release notes", checked: false, isHeader: false },
        ],
        checklistRaw:
          "> Setup\n- [x] Reproduce locally with sample creds\n- [x] Add integration test covering double-submit\n> Implementation\n- [ ] Idempotency token in proc call\n- [ ] Frontend guard while in-flight\n- [ ] Update release notes",
        created: new Date(Date.now() - 1000 * 60 * 60 * 24 * 3).toISOString(),
        updated: new Date(Date.now() - 1000 * 60 * 30).toISOString(),
      } as T;
    }

    case "start_new_claude_session": {
      await delay(400);
      console.log("[mock] start_new_claude_session", args);
      const cwd = (args?.cwd as string) ?? "/mock/cwd";
      const sid = `mock-${Math.random().toString(36).slice(2, 10)}`;
      // Fire system:init then assistant on a short delay so the chat panel
      // sees lifecycle events as if a real session started.
      (async () => {
        await delay(150);
        window.dispatchEvent(
          new CustomEvent("mock-claude-event", {
            detail: {
              sessionId: sid,
              event: { type: "system", subtype: "init", session_id: sid, model: "claude-sonnet-4-6" },
            },
          }),
        );
      })();
      return {
        sessionId: sid,
        pid: 50000 + Math.floor(Math.random() * 1000),
        jsonlPath: `${cwd}/.claude/projects/mock/${sid}.jsonl`,
        startedAt: Date.now(),
        cwd,
        worktreePath: args?.worktree ? `${cwd}/.worktrees/${Math.random().toString(36).slice(2, 10)}` : null,
      } as T;
    }

    case "interrupt_claude_session": {
      console.log("[mock] interrupt_claude_session", args);
      const sid = (args?.sessionId as string) ?? "mock-session";
      window.dispatchEvent(
        new CustomEvent("mock-claude-event", {
          detail: {
            sessionId: sid,
            event: { type: "result", subtype: "error_during_execution", duration_ms: 50 },
          },
        }),
      );
      return undefined as T;
    }

    case "update_session_label": {
      await delay(50);
      console.log("[mock] update_session_label", args);
      return undefined as T;
    }

    case "pick_directory": {
      await delay(150);
      console.log("[mock] pick_directory");
      return "/mnt/c/dev/mock-project" as T;
    }

    case "write_session_pty": {
      // Echo input back as a pty event so xterm can render it locally during dev.
      const sid = (args?.sessionId as string) ?? "mock-session";
      const data = (args?.data as string) ?? "";
      window.dispatchEvent(
        new CustomEvent("mock-claude-event", {
          detail: { sessionId: sid, event: { type: "pty", text: data } },
        }),
      );
      return undefined as T;
    }

    case "resize_session_pty": {
      console.log("[mock] resize_session_pty", args);
      return undefined as T;
    }

    case "list_projects": {
      await delay(50);
      return [
        {
          cwd: "/mnt/c/dev/fnba-utils",
          displayName: "fnba-utils",
          pinned: true,
          lastUsedAt: Date.now() - 600000,
          notes: null,
        },
        {
          cwd: "/mnt/c/dev/other-project",
          displayName: "other-project",
          pinned: false,
          lastUsedAt: Date.now() - 86_400_000,
          notes: null,
        },
      ] as T;
    }

    case "add_project":
    case "update_project": {
      await delay(30);
      console.log(`[mock] ${cmd}`, args);
      return true as T;
    }

    case "remove_project": {
      await delay(30);
      console.log("[mock] remove_project", args);
      return true as T;
    }

    case "record_project_used": {
      await delay(20);
      return undefined as T;
    }

    case "list_sql_groups": {
      await delay(40);
      return [
        { id: "grp-mock-loans", name: "Loan ops", orderIdx: 0, color: "#60a5fa", pinned: true },
        { id: "grp-mock-reports", name: "Reporting", orderIdx: 1, color: null, pinned: false },
      ] as T;
    }

    case "list_sql_queries": {
      await delay(40);
      const now = Date.now();
      return [
        {
          id: "q-mock-1",
          name: "Active loans (mock)",
          sql: "SELECT TOP 10 loan_id, status FROM loans WHERE status = 'active';",
          database: "loans",
          groupId: "grp-mock-loans",
          lastUsedAt: now - 5 * 60_000,
          createdAt: now - 7 * 86_400_000,
        },
        {
          id: "q-mock-2",
          name: "Closed loans (mock)",
          sql: "SELECT TOP 10 loan_id FROM loans WHERE status = 'closed';",
          database: "loans",
          groupId: "grp-mock-loans",
          lastUsedAt: now - 60 * 60_000,
          createdAt: now - 3 * 86_400_000,
        },
        {
          id: "q-mock-3",
          name: "Daily revenue (mock)",
          sql: "SELECT SUM(amount) FROM payments WHERE DATEDIFF(day, paid_at, GETDATE()) = 0;",
          database: "reporting",
          groupId: "grp-mock-reports",
          lastUsedAt: now - 24 * 60 * 60_000,
          createdAt: now - 14 * 86_400_000,
        },
        {
          id: "q-mock-4",
          name: "Unfiled (mock)",
          sql: "SELECT @@VERSION;",
          database: "",
          groupId: null,
          lastUsedAt: 0,
          createdAt: now - 1 * 86_400_000,
        },
      ] as T;
    }

    case "add_sql_group": {
      await delay(30);
      const name = (args?.name as string) ?? "New group";
      window.dispatchEvent(new CustomEvent("mock-sql-queries-changed"));
      return {
        id: `grp-mock-${Date.now()}`,
        name,
        orderIdx: 99,
        color: null,
        pinned: false,
      } as T;
    }

    case "add_sql_query": {
      await delay(30);
      const now = Date.now();
      window.dispatchEvent(new CustomEvent("mock-sql-queries-changed"));
      return {
        id: `q-mock-${now}`,
        name: (args?.name as string) ?? "",
        sql: (args?.sql as string) ?? "",
        database: (args?.database as string) ?? "",
        groupId: (args?.groupId as string | null) ?? null,
        lastUsedAt: 0,
        createdAt: now,
      } as T;
    }

    case "rename_sql_group":
    case "set_sql_group_color":
    case "set_sql_group_pinned":
    case "reorder_sql_groups":
    case "update_sql_query":
    case "move_sql_query_to_group":
    case "record_sql_query_used": {
      await delay(20);
      console.log(`[mock] ${cmd}`, args);
      window.dispatchEvent(new CustomEvent("mock-sql-queries-changed"));
      return undefined as T;
    }

    case "remove_sql_group":
    case "remove_sql_query": {
      await delay(20);
      console.log(`[mock] ${cmd}`, args);
      window.dispatchEvent(new CustomEvent("mock-sql-queries-changed"));
      return true as T;
    }

    case "migrate_legacy_sql_queries": {
      await delay(30);
      const entries = (args?.entries as unknown[]) ?? [];
      console.log(`[mock] migrate_legacy_sql_queries (${entries.length})`);
      if (entries.length > 0) {
        window.dispatchEvent(new CustomEvent("mock-sql-queries-changed"));
      }
      return entries.length as T;
    }

    case "open_path_in_editor": {
      console.log("[mock] open_path_in_editor", args);
      return undefined as T;
    }

    case "list_session_history": {
      await delay(60);
      const now = Date.now();
      return [
        {
          sessionId: "old-abc-123",
          cwd: "/mnt/c/dev/fnba-utils",
          pid: 0,
          startedAt: now - 86_400_000,
          endedAt: now - 3_600_000,
          label: "yesterday's refactor",
          claudeHome: "/home/user/.claude",
          worktreePath: null,
          tmuxSession: "claude-old-abc-123",
        },
      ] as T;
    }

    case "forget_session_history": {
      console.log("[mock] forget_session_history", args);
      return true as T;
    }

    case "resume_owned_session": {
      await delay(300);
      console.log("[mock] resume_owned_session", args);
      const sid = (args?.sessionId as string) ?? "mock";
      return {
        sessionId: sid,
        pid: 50000 + Math.floor(Math.random() * 1000),
        jsonlPath: `/mock/${sid}.jsonl`,
        startedAt: Date.now(),
        cwd: "/mnt/c/dev/mock-project",
        worktreePath: null,
      } as T;
    }

    case "list_clipboard_entries": {
      await delay(60);
      const q = (args?.query as string | undefined)?.trim().toLowerCase();
      const kind = args?.kind as string | undefined;
      const pinnedOnly = (args?.pinnedOnly as boolean | undefined) ?? false;
      let rows = mockClipboardEntries();
      if (kind) rows = rows.filter((r) => r.kind === kind);
      if (pinnedOnly) rows = rows.filter((r) => r.pinned);
      if (q) {
        rows = rows.filter(
          (r) => r.textPreview && r.textPreview.toLowerCase().includes(q),
        );
      }
      rows.sort(
        (a, b) =>
          Number(b.pinned) - Number(a.pinned) || b.capturedAt - a.capturedAt,
      );
      return rows as T;
    }
    case "get_clipboard_entry": {
      await delay(40);
      const id = args?.id as number;
      const found = mockClipboardEntries().find((e) => e.id === id);
      if (!found) return null as T;
      // Mock obfuscated text: for sensitive rows, expose a fake substituted
      // version + a fake "original" so the UI can demo the toggle.
      const original = found.sensitive
        ? `[mock original ${id}: SSN 123-45-6789]`
        : found.textPreview;
      return {
        ...found,
        textContent: original,
        htmlContent: found.kind === "html" ? `<p>${found.textPreview}</p>` : null,
        imageBase64: null,
        obfuscatedText: found.sensitive ? found.textPreview : null,
        testUserId: found.sensitive ? 1 : null,
        contentHash: `mock-${id}`,
      } as T;
    }
    case "paste_clipboard_entry": {
      await delay(40);
      console.log("[mock] paste_clipboard_entry", args);
      return undefined as T;
    }
    case "request_sensitive_reveal": {
      await delay(40);
      return {
        id: args?.id as number,
        token: `mock-token-${Date.now()}`,
        expiresInMs: 15000,
      } as T;
    }
    case "delete_clipboard_entry":
    case "pin_clipboard_entry":
    case "clear_clipboard_history":
    case "set_clipboard_settings":
    case "hide_clipboard_window":
    case "delete_test_user":
    case "set_test_user_enabled": {
      await delay(20);
      return undefined as T;
    }
    case "get_clipboard_settings": {
      await delay(20);
      return {
        textCap: 5000,
        imageCap: 500,
        captureEnabled: true,
        ignoredProcesses: [],
      } as T;
    }
    case "get_clipboard_max_captured_at": {
      await delay(10);
      return (mockClipboardEntries()[0]?.capturedAt ?? 0) as T;
    }
    case "list_test_users": {
      await delay(30);
      return mockTestUsers() as T;
    }
    case "upsert_test_user": {
      await delay(40);
      const user = args?.user as TestUser;
      return ((user?.id ?? Math.floor(Math.random() * 10_000) + 100) as number) as T;
    }

    default:
      throw new Error(`[mock] Unknown command: ${cmd}`);
  }
}

function mockClipboardEntries(): ClipboardEntrySummary[] {
  const now = Date.now();
  return [
    {
      id: 1,
      kind: "text",
      textPreview: "SELECT TOP 100 * FROM logincheck.dbo.AssumeIdentityLog ORDER BY CreatedAt DESC",
      thumbBase64: null,
      width: null,
      height: null,
      byteSize: 78,
      sensitive: false,
      piiKinds: [],
      sourceProcess: "ssms.exe",
      capturedAt: now - 1000 * 30,
      pinned: false,
    },
    {
      id: 2,
      kind: "text",
      textPreview: "git checkout -b feature/clipboard-manager",
      thumbBase64: null,
      width: null,
      height: null,
      byteSize: 42,
      sensitive: false,
      piiKinds: [],
      sourceProcess: "WindowsTerminal.exe",
      capturedAt: now - 1000 * 60 * 5,
      pinned: true,
    },
    {
      id: 3,
      kind: "html",
      textPreview: "The quick brown fox jumps over the lazy dog.",
      thumbBase64: null,
      width: null,
      height: null,
      byteSize: 180,
      sensitive: false,
      piiKinds: [],
      sourceProcess: "chrome.exe",
      capturedAt: now - 1000 * 60 * 12,
      pinned: false,
    },
    {
      id: 4,
      kind: "image",
      textPreview: null,
      thumbBase64:
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==",
      width: 1920,
      height: 1080,
      byteSize: 245_000,
      sensitive: false,
      piiKinds: [],
      sourceProcess: "SnippingTool.exe",
      capturedAt: now - 1000 * 60 * 25,
      pinned: false,
    },
    {
      id: 5,
      kind: "text",
      textPreview: "Customer SSN 900-11-1111 on file",
      thumbBase64: null,
      width: null,
      height: null,
      byteSize: 32,
      sensitive: true,
      piiKinds: ["ssn"],
      sourceProcess: "ssms.exe",
      capturedAt: now - 1000 * 60 * 47,
      pinned: false,
    },
    {
      id: 6,
      kind: "text",
      textPreview: "kevin.biesbrock@fnba.com",
      thumbBase64: null,
      width: null,
      height: null,
      byteSize: 26,
      sensitive: false,
      piiKinds: [],
      sourceProcess: "outlook.exe",
      capturedAt: now - 1000 * 60 * 60 * 2,
      pinned: false,
    },
    {
      id: 7,
      kind: "text",
      textPreview:
        "Error: Connection timeout to FNB-SQL-01.fnba.com after 30000ms (tcp/1433)",
      thumbBase64: null,
      width: null,
      height: null,
      byteSize: 84,
      sensitive: false,
      piiKinds: [],
      sourceProcess: "ssms.exe",
      capturedAt: now - 1000 * 60 * 60 * 4,
      pinned: false,
    },
    {
      id: 8,
      kind: "text",
      textPreview: "Card **** **** **** 4242, DOB 1990-01-15",
      thumbBase64: null,
      width: null,
      height: null,
      byteSize: 64,
      sensitive: true,
      piiKinds: ["card", "dob"],
      sourceProcess: "chrome.exe",
      capturedAt: now - 1000 * 60 * 90,
      pinned: false,
    },
  ];
}

function mockTestUsers(): TestUser[] {
  return [
    {
      id: 1,
      label: "Test Alice Tester",
      firstName: "Alice",
      lastName: "Tester",
      ssn: "900-11-1111",
      dob: "1990-01-15",
      email: "alice.tester@test.fnba.local",
      phone: "555-010-0001",
      address: "100 Test Lane, Springfield, IL 62701",
      accountNum: "100010000001",
      routingNum: "021000021",
      cards: [{ number: "4242424242424242", expiry: "12/29", cvv: "123" }],
      enabled: true,
    },
    {
      id: 2,
      label: "Test Bob Sample",
      firstName: "Bob",
      lastName: "Sample",
      ssn: "900-22-2222",
      dob: "1985-03-22",
      email: "bob.sample@test.fnba.local",
      phone: "555-010-0002",
      address: "110 Test Lane, Springfield, IL 62701",
      accountNum: "100010000002",
      routingNum: "021000021",
      cards: [{ number: "5555555555554444", expiry: "11/28", cvv: "234" }],
      enabled: true,
    },
  ];
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

export function openAppDataFolder(): Promise<void> {
  return invoke<void>("open_app_data_folder");
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

/** Resolve the bare Windows login for an associate (Right Lookup -> Assume). */
export function getAssumeLogin(server: string, assocId: number): Promise<string | null> {
  return invoke<string | null>("get_assume_login", { server, assocId });
}

/**
 * Pin a user to the distributable favorites list under `label`.
 * Resolves `true` if newly added, `false` if already a favorite.
 */
export function pinFavorite(username: string, label: string): Promise<boolean> {
  return invoke<boolean>("pin_favorite", { username, label });
}

/**
 * Remove a favorite from view. Deletes custom entries outright; hides
 * shipped-default entries so they stop appearing in getIdentityData().
 */
export function removeFavorite(label: string, username: string): Promise<void> {
  return invoke<void>("remove_favorite", { label, username });
}

/** Stamp a favorite's last-used time (drives the recency hot-pick order). */
export function markFavoriteUsed(label: string, username: string): Promise<void> {
  return invoke<void>("mark_favorite_used", { label, username });
}

export function getClaudeSessions(force = false): Promise<ClaudeSession[]> {
  return invoke<ClaudeSession[]>("get_claude_sessions", { forceRefresh: force });
}

export function getConnectionStatuses(): Promise<ConnectionStatus[]> {
  return invoke<ConnectionStatus[]>("get_connection_statuses");
}

export function executeSqlQuery(
  server: string,
  database: string,
  sql: string,
  queryId: string,
): Promise<QueryResult> {
  return invoke<QueryResult>("execute_sql_query", { server, database, sql, queryId });
}

export function killSqlQuery(queryId: string): Promise<void> {
  return invoke<void>("kill_sql_query", { queryId });
}

export function getSessionDetail(sessionId: string): Promise<SessionDetail> {
  return invoke<SessionDetail>("get_session_detail", { sessionId });
}

/**
 * Launch a brand-new Claude session in the chosen cwd. Returns immediately
 * with the assigned session_id; the chat panel subscribes to claude-event for
 * output as the JSONL fills in.
 */
export function startNewClaudeSession(
  cwd: string,
  initialPrompt: string | null,
  worktree: boolean,
): Promise<NewSessionInfo> {
  return invoke<NewSessionInfo>("start_new_claude_session", {
    cwd,
    initialPrompt,
    worktree,
  });
}

/** Send Ctrl-C to interrupt the current turn without killing the process (Feature #14). */
export function interruptClaudeSession(sessionId: string): Promise<void> {
  return invoke<void>("interrupt_claude_session", { sessionId });
}

/** Set or clear the user-assigned label for a session (Feature #20). */
export function updateSessionLabel(
  sessionId: string,
  label: string | null,
): Promise<void> {
  return invoke<void>("update_session_label", { sessionId, label });
}

/** Open the native directory picker; returns a WSL path or null if cancelled. */
export function pickDirectory(): Promise<string | null> {
  return invoke<string | null>("pick_directory");
}

/** Open a file path (WSL or Windows form) in IntelliJ if available, Explorer otherwise. */
export function openPathInEditor(path: string): Promise<void> {
  return invoke<void>("open_path_in_editor", { path });
}

/** Wave 2: project registry CRUD. */
export function listProjects(): Promise<Project[]> {
  return invoke<Project[]>("list_projects");
}

export function addProject(
  cwd: string,
  displayName: string | null,
  pinned: boolean | null,
  notes: string | null,
): Promise<boolean> {
  return invoke<boolean>("add_project", { cwd, displayName, pinned, notes });
}

export function updateProject(
  cwd: string,
  displayName: string | null,
  pinned: boolean | null,
  notes: string | null,
): Promise<void> {
  return invoke<void>("update_project", { cwd, displayName, pinned, notes });
}

export function removeProject(cwd: string): Promise<boolean> {
  return invoke<boolean>("remove_project", { cwd });
}

export function recordProjectUsed(cwd: string): Promise<void> {
  return invoke<void>("record_project_used", { cwd });
}

/** Saved SQL queries + groups (SQLite-backed). */
export function listSqlGroups(): Promise<SqlGroup[]> {
  return invoke<SqlGroup[]>("list_sql_groups");
}

export function addSqlGroup(name: string): Promise<SqlGroup> {
  return invoke<SqlGroup>("add_sql_group", { name });
}

export function renameSqlGroup(id: string, name: string): Promise<void> {
  return invoke<void>("rename_sql_group", { id, name });
}

export function setSqlGroupColor(id: string, color: string | null): Promise<void> {
  return invoke<void>("set_sql_group_color", { id, color });
}

export function setSqlGroupPinned(id: string, pinned: boolean): Promise<void> {
  return invoke<void>("set_sql_group_pinned", { id, pinned });
}

export function reorderSqlGroups(ids: string[]): Promise<void> {
  return invoke<void>("reorder_sql_groups", { ids });
}

export function removeSqlGroup(id: string): Promise<boolean> {
  return invoke<boolean>("remove_sql_group", { id });
}

export function listSqlQueries(): Promise<SavedSqlQuery[]> {
  return invoke<SavedSqlQuery[]>("list_sql_queries");
}

export function addSqlQuery(
  name: string,
  sql: string,
  database: string,
  groupId: string | null,
): Promise<SavedSqlQuery> {
  return invoke<SavedSqlQuery>("add_sql_query", { name, sql, database, groupId });
}

export function updateSqlQuery(
  id: string,
  name: string,
  sql: string,
  database: string,
): Promise<void> {
  return invoke<void>("update_sql_query", { id, name, sql, database });
}

export function moveSqlQueryToGroup(id: string, groupId: string | null): Promise<void> {
  return invoke<void>("move_sql_query_to_group", { id, groupId });
}

export function removeSqlQuery(id: string): Promise<boolean> {
  return invoke<boolean>("remove_sql_query", { id });
}

export function recordSqlQueryUsed(id: string): Promise<void> {
  return invoke<void>("record_sql_query_used", { id });
}

export function migrateLegacySqlQueries(
  entries: LegacySavedSqlQuery[],
): Promise<number> {
  return invoke<number>("migrate_legacy_sql_queries", { entries });
}

/**
 * Fires whenever any SQL panel mutates the saved-queries DB — every open panel
 * uses this to refresh its in-memory cache. The data is global, not scoped
 * per-server, so all SQL panels need to stay in sync.
 */
export async function onSqlQueriesChanged(
  handler: () => void,
): Promise<() => void> {
  if (isTauri) {
    const { listen } = await import("@tauri-apps/api/event");
    return listen<null>("sql-queries-changed", () => handler());
  }
  const listener = () => handler();
  window.addEventListener("mock-sql-queries-changed", listener);
  return () => window.removeEventListener("mock-sql-queries-changed", listener);
}

/** Wave 4 session history. */
export function listSessionHistory(limit?: number): Promise<HistoricalSession[]> {
  return invoke<HistoricalSession[]>("list_session_history", { limit: limit ?? null });
}

export function forgetSessionHistory(sessionId: string): Promise<boolean> {
  return invoke<boolean>("forget_session_history", { sessionId });
}

export function resumeOwnedSession(sessionId: string): Promise<NewSessionInfo> {
  return invoke<NewSessionInfo>("resume_owned_session", { sessionId });
}

export function killSession(pid: number): Promise<void> {
  return invoke<void>("kill_session", { pid });
}

/** Spawn a Claude SDK process for an existing session and start streaming events. */
export function startClaudeSession(sessionId: string, cwd: string): Promise<void> {
  return invoke<void>("start_claude_session", { sessionId, cwd });
}

/**
 * Attach to an external tmux session (not spawned by MC). Streams PTY bytes
 * over the same `claude-event` channel under the synthetic session id
 * `tmux:<name>`, so the existing xterm wiring routes without changes.
 */
export function attachTmuxSession(name: string, cwd: string | null): Promise<void> {
  return invoke<void>("attach_tmux_session", { name, cwd });
}

/** Synthetic prefix used for external tmux session ids in Mission Control. */
export const TMUX_SESSION_PREFIX = "tmux:";

/** True if a session id refers to an external tmux attach (vs. an MC session). */
export function isTmuxSessionId(sessionId: string): boolean {
  return sessionId.startsWith(TMUX_SESSION_PREFIX);
}

/** Extract the tmux session name from a synthetic `tmux:<name>` id. */
export function tmuxNameFromSessionId(sessionId: string): string {
  return sessionId.startsWith(TMUX_SESSION_PREFIX)
    ? sessionId.slice(TMUX_SESSION_PREFIX.length)
    : sessionId;
}

/** Send a user message to a running Claude SDK session (legacy; used for the initial-prompt path on spawn). */
export function sendClaudeMessage(sessionId: string, content: string): Promise<void> {
  return invoke<void>("send_claude_message", { sessionId, content });
}

/** Write raw bytes directly to a session's PTY (the path the xterm terminal uses for every keystroke). */
export function writeSessionPty(sessionId: string, data: string): Promise<void> {
  return invoke<void>("write_session_pty", { sessionId, data });
}

/** Resize the PTY to match the xterm.js viewport. */
export function resizeSessionPty(
  sessionId: string,
  cols: number,
  rows: number,
): Promise<void> {
  return invoke<void>("resize_session_pty", { sessionId, cols, rows });
}

/** Terminate a running Claude SDK session. The original interactive Claude is unaffected. */
export function stopClaudeSession(sessionId: string): Promise<void> {
  return invoke<void>("stop_claude_session", { sessionId });
}

/** Disconnect our PTY from a session without killing it (panel-close path). */
export function disconnectSession(sessionId: string): Promise<void> {
  return invoke<void>("disconnect_session", { sessionId });
}

/** Listen for stream-json events from any active Claude SDK session. Filter on sessionId yourself. */
export async function onClaudeEvent(
  handler: (event: ClaudeEventEnvelope) => void,
): Promise<() => void> {
  if (isTauri) {
    const { listen } = await import("@tauri-apps/api/event");
    return listen<ClaudeEventEnvelope>("claude-event", (e) => handler(e.payload));
  }
  const listener = (e: Event) => handler((e as CustomEvent).detail);
  window.addEventListener("mock-claude-event", listener);
  return () => window.removeEventListener("mock-claude-event", listener);
}

/** Listen for Claude SDK session exit. Returns an unlisten function. */
export async function onClaudeSessionClosed(
  handler: (event: ClaudeSessionClosedEvent) => void,
): Promise<() => void> {
  if (isTauri) {
    const { listen } = await import("@tauri-apps/api/event");
    return listen<ClaudeSessionClosedEvent>("claude-session-closed", (e) =>
      handler(e.payload),
    );
  }
  const listener = (e: Event) => handler((e as CustomEvent).detail);
  window.addEventListener("mock-claude-session-closed", listener);
  return () => window.removeEventListener("mock-claude-session-closed", listener);
}

export function openInExplorer(cwd: string): Promise<void> {
  return invoke<void>("open_in_explorer", { cwd });
}

export function getAppConfig(): Promise<AppConfigView> {
  return invoke<AppConfigView>("get_app_config");
}

export function getStandupLastRun(): Promise<StandupLastRun | null> {
  return invoke<StandupLastRun | null>("get_standup_last_run");
}

export function getStandupReport(): Promise<StandupReport> {
  return invoke<StandupReport>("get_standup_report");
}

export function runStandup(postToTeams: boolean): Promise<StandupRunResult> {
  return invoke<StandupRunResult>("run_standup", { postToTeamsFlag: postToTeams });
}

/**
 * Preview-first flow: fetch Jira, build the report, and persist as a preview
 * run. Does NOT post to Teams and does NOT touch the clipboard — the user
 * triggers `copyStandupReport` explicitly via the Copy button.
 */
export function previewStandup(): Promise<StandupRunResult> {
  return invoke<StandupRunResult>("preview_standup");
}

/**
 * Copy the report's plain-text rendition to the system clipboard. Resolves to
 * the text that was written (so the caller can show exact "Copied" feedback).
 */
export function copyStandupReport(report: StandupReport): Promise<string> {
  return invoke<string>("copy_standup_report", { report });
}

/**
 * Post the previewed report to Teams. The frontend echoes back the exact
 * StandupReport from the preview so the post matches what was on screen.
 */
export function postStandupToTeams(report: StandupReport): Promise<StandupRunResult> {
  return invoke<StandupRunResult>("post_standup_to_teams", { report });
}

export function getStandupPanelState(): Promise<StandupPanelState> {
  return invoke<StandupPanelState>("get_standup_panel_state");
}

export function setIssueHidden(key: string, hidden: boolean): Promise<void> {
  return invoke<void>("set_issue_hidden", { key, hidden });
}

export function clearHiddenIssues(): Promise<number> {
  return invoke<number>("clear_hidden_issues");
}

export function setIssueOrder(orderedKeys: string[]): Promise<void> {
  return invoke<void>("set_issue_order", { orderedKeys });
}

export function clearManualOrder(): Promise<number> {
  return invoke<number>("clear_manual_order");
}

export function getRunSnapshot(runId: number): Promise<StandupReport | null> {
  return invoke<StandupReport | null>("get_run_snapshot", { runId });
}

export function getIssueDetail(key: string): Promise<IssueDetail> {
  return invoke<IssueDetail>("get_issue_detail", { key });
}

/** Listen for standup-updated events (emitted after run_standup completes). */
export async function onStandupUpdated(handler: () => void): Promise<() => void> {
  if (isTauri) {
    const { listen } = await import("@tauri-apps/api/event");
    return listen("standup-updated", () => handler());
  }
  const listener = () => handler();
  window.addEventListener("mock-standup-updated", listener);
  return () => window.removeEventListener("mock-standup-updated", listener);
}

// --- Clipboard Manager public API ---

export function listClipboardEntries(
  query?: string,
  kind?: ClipboardKind,
  pinnedOnly?: boolean,
  limit?: number,
  offset?: number,
): Promise<ClipboardEntrySummary[]> {
  return invoke<ClipboardEntrySummary[]>("list_clipboard_entries", {
    query: query ?? null,
    kind: kind ?? null,
    pinnedOnly: pinnedOnly ?? false,
    limit: limit ?? 100,
    offset: offset ?? 0,
  });
}

export function getClipboardEntry(id: number): Promise<ClipboardEntryFull | null> {
  return invoke<ClipboardEntryFull | null>("get_clipboard_entry", { id });
}

export function pasteClipboardEntry(
  id: number,
  options: ClipboardPasteOptions,
): Promise<void> {
  return invoke<void>("paste_clipboard_entry", { id, options });
}

export function requestSensitiveReveal(id: number): Promise<ClipboardRevealToken> {
  return invoke<ClipboardRevealToken>("request_sensitive_reveal", { id });
}

export function deleteClipboardEntry(id: number): Promise<void> {
  return invoke<void>("delete_clipboard_entry", { id });
}

export function pinClipboardEntry(id: number, pinned: boolean): Promise<void> {
  return invoke<void>("pin_clipboard_entry", { id, pinned });
}

export function clearClipboardHistory(includePinned: boolean): Promise<number> {
  return invoke<number>("clear_clipboard_history", { includePinned });
}

export function getClipboardSettings(): Promise<ClipboardSettings> {
  return invoke<ClipboardSettings>("get_clipboard_settings");
}

export function setClipboardSettings(settings: ClipboardSettings): Promise<void> {
  return invoke<void>("set_clipboard_settings", { settings });
}

export function hideClipboardWindow(): Promise<void> {
  return invoke<void>("hide_clipboard_window");
}

/** Latest captured_at timestamp in the DB; used by the UI to poll for new
 *  entries inserted by the out-of-process capture daemon. */
export function getClipboardMaxCapturedAt(): Promise<number> {
  return invoke<number>("get_clipboard_max_captured_at");
}

// --- Test Users public API ---

export function listTestUsers(): Promise<TestUser[]> {
  return invoke<TestUser[]>("list_test_users");
}

export function upsertTestUser(user: TestUser): Promise<number> {
  return invoke<number>("upsert_test_user", { user });
}

export function deleteTestUser(id: number): Promise<void> {
  return invoke<void>("delete_test_user", { id });
}

export function setTestUserEnabled(id: number, enabled: boolean): Promise<void> {
  return invoke<void>("set_test_user_enabled", { id, enabled });
}

/** Fires whenever a new clipboard entry is captured + persisted. */
export async function onClipboardEntryAdded(
  handler: (id: number) => void,
): Promise<() => void> {
  if (isTauri) {
    const { listen } = await import("@tauri-apps/api/event");
    return listen<number>("clipboard-entry-added", (e) => handler(e.payload));
  }
  const listener = (e: Event) => handler((e as CustomEvent<number>).detail);
  window.addEventListener("mock-clipboard-entry-added", listener);
  return () => window.removeEventListener("mock-clipboard-entry-added", listener);
}

/** Fires when the clipboard window is shown via the global shortcut. */
export interface ClipboardWindowShownPayload {
  initialFilter?: string | null;
}

export async function onClipboardWindowShown(
  handler: (payload: ClipboardWindowShownPayload) => void,
): Promise<() => void> {
  if (isTauri) {
    const { listen } = await import("@tauri-apps/api/event");
    return listen<ClipboardWindowShownPayload>(
      "clipboard-window-shown",
      (e) => handler(e.payload ?? {}),
    );
  }
  const listener = (e: Event) =>
    handler((e as CustomEvent<ClipboardWindowShownPayload>).detail ?? {});
  window.addEventListener("mock-clipboard-window-shown", listener);
  return () => window.removeEventListener("mock-clipboard-window-shown", listener);
}

export { isTauri };
