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
          configPath: "~/.fnba-utils/config.yaml",
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
        copiedToClipboard: cmd !== "post_standup_to_teams",
        warnings: [],
        teamsConfigured: mockTeamsConfigured,
        teamsChannelUrl: mockTeamsConfigured ? mockChannelUrl : null,
      } as T;
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
 * Preview-first flow: fetch Jira, build the report, copy text to clipboard, and
 * persist as a preview run. Does NOT post to Teams.
 */
export function previewStandup(): Promise<StandupRunResult> {
  return invoke<StandupRunResult>("preview_standup");
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

export { isTauri };
