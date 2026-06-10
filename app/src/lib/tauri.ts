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
  /** True if this issue should be included in the Teams post / clipboard copy.
   *  Defaults to true. Today only honored for the To Do group; other groups
   *  always post. */
  postToTeams: boolean;
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
  /** User-supplied label/rename. Metadata only — paste still uses content. */
  label: string | null;
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
  /** User-supplied label/rename. */
  label: string | null;
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

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke: tauriInvoke } = await import("@tauri-apps/api/core");
  return tauriInvoke<T>(cmd, args);
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
  const { listen } = await import("@tauri-apps/api/event");
  return listen<null>("sql-queries-changed", () => handler());
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
  const { listen } = await import("@tauri-apps/api/event");
  return listen<ClaudeEventEnvelope>("claude-event", (e) => handler(e.payload));
}

/** Listen for Claude SDK session exit. Returns an unlisten function. */
export async function onClaudeSessionClosed(
  handler: (event: ClaudeSessionClosedEvent) => void,
): Promise<() => void> {
  const { listen } = await import("@tauri-apps/api/event");
  return listen<ClaudeSessionClosedEvent>("claude-session-closed", (e) =>
    handler(e.payload),
  );
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

export function copyText(text: string): Promise<void> {
  return invoke<void>("copy_text", { text });
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

/** Toggle whether a single issue is included in the Teams post / clipboard copy.
 *  Only honored for the To Do group at format time; other groups always post. */
export function setStandupIssuePostToTeams(
  key: string,
  post: boolean,
): Promise<void> {
  return invoke<void>("set_standup_issue_post_to_teams", { key, post });
}

export function getRunSnapshot(runId: number): Promise<StandupReport | null> {
  return invoke<StandupReport | null>("get_run_snapshot", { runId });
}

export function getIssueDetail(key: string): Promise<IssueDetail> {
  return invoke<IssueDetail>("get_issue_detail", { key });
}

/** Listen for standup-updated events (emitted after run_standup completes). */
export async function onStandupUpdated(handler: () => void): Promise<() => void> {
  const { listen } = await import("@tauri-apps/api/event");
  return listen("standup-updated", () => handler());
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

export function setClipboardEntryLabel(
  id: number,
  label: string | null,
): Promise<void> {
  return invoke<void>("set_clipboard_entry_label", { id, label });
}

export function updateClipboardEntryContent(
  id: number,
  content: string,
): Promise<void> {
  return invoke<void>("update_clipboard_entry_content", { id, content });
}

export function setClipboardEntrySensitivity(
  id: number,
  sensitive: boolean,
): Promise<void> {
  return invoke<void>("set_clipboard_entry_sensitivity", { id, sensitive });
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
  const { listen } = await import("@tauri-apps/api/event");
  return listen<number>("clipboard-entry-added", (e) => handler(e.payload));
}

/** Fires when an entry's metadata or content was mutated (label, edit, sensitivity). */
export async function onClipboardEntryUpdated(
  handler: (id: number) => void,
): Promise<() => void> {
  const { listen } = await import("@tauri-apps/api/event");
  return listen<number>("clipboard-entry-updated", (e) => handler(e.payload));
}

/** Fires when the clipboard window is shown via the global shortcut. */
export interface ClipboardWindowShownPayload {
  initialFilter?: string | null;
}

export async function onClipboardWindowShown(
  handler: (payload: ClipboardWindowShownPayload) => void,
): Promise<() => void> {
  const { listen } = await import("@tauri-apps/api/event");
  return listen<ClipboardWindowShownPayload>(
    "clipboard-window-shown",
    (e) => handler(e.payload ?? {}),
  );
}
