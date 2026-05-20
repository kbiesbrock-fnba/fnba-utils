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
          entrypoint: "cli",
          isAlive: true,
          subagentCount: 3,
          subagents: [
            { agentType: "Explore", description: "Explore codebase structure" },
            { agentType: "Explore", description: "Explore test patterns" },
            { agentType: "Plan", description: "Design auth feature" },
          ],
          status: "busy",
          lastMessageAt: new Date(now - 30000).toISOString(),
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
          status: "idle",
          lastMessageAt: new Date(now - 120000).toISOString(),
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
                status: "idle",
                lastMessageAt: new Date(now - 600000).toISOString(),
              } satisfies ClaudeSession,
            ]
          : []),
      ];
      return sessions as T;
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
        pid: (args as Record<string, unknown>)?.pid ?? 12345,
        sessionId: (args as Record<string, unknown>)?.sessionId ?? "abc-123",
        cwd: (args as Record<string, unknown>)?.cwd ?? "/mnt/c/dev/my-project",
        startedAt: now - 3600000,
        kind: "interactive",
        name: "refactor-auth",
        entrypoint: "cli",
        isAlive: true,
        gitBranch: "feature/auth-rework",
        status: "idle",
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
    case "run_standup": {
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
      return {
        report: sample,
        postedToTeams: !!(args && (args.postToTeamsFlag as boolean)),
        copiedToClipboard: true,
        warnings: [],
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

export function getSessionDetail(
  sessionId: string,
  cwd: string,
  pid: number,
): Promise<SessionDetail> {
  return invoke<SessionDetail>("get_session_detail", { sessionId, cwd, pid });
}

export function killSession(pid: number): Promise<void> {
  return invoke<void>("kill_session", { pid });
}

/** Spawn a Claude SDK process for an existing session and start streaming events. */
export function startClaudeSession(sessionId: string, cwd: string): Promise<void> {
  return invoke<void>("start_claude_session", { sessionId, cwd });
}

/** Send a user message to a running Claude SDK session. */
export function sendClaudeMessage(sessionId: string, content: string): Promise<void> {
  return invoke<void>("send_claude_message", { sessionId, content });
}

/** Terminate a running Claude SDK session. The original interactive Claude is unaffected. */
export function stopClaudeSession(sessionId: string): Promise<void> {
  return invoke<void>("stop_claude_session", { sessionId });
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
