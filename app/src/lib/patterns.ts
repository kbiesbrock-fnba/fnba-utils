// Shared content-detection patterns. No Tauri or UI dependencies.

export const URL_RE = /^(https?:\/\/|www\.)\S+$/i;
export const JIRA_KEY_RE = /^[A-Z][A-Z0-9]*-\d+$/;
export const JIRA_IN_URL_RE = /\/browse\/([A-Z][A-Z0-9]*-\d+)/i;

const GUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
// Single-line path: C:\... or /... or ~ / ~/...
// Tilde: bare "~", "~/", or "~/anything" — but NOT "~word" (no slash).
const PATH_RE = /^(?:[A-Za-z]:[/\\]\S+|\/\S+|~(?=$|[/\\])\S*)/;
const SQL_FIRST_WORD_RE = /^\s*(SELECT|INSERT|UPDATE|DELETE|CREATE|ALTER|DROP|WITH)\b/i;

export function isJsonText(q: string): boolean {
  if (q.length < 2 || (q[0] !== "{" && q[0] !== "[")) return false;
  try {
    JSON.parse(q);
    return true;
  } catch {
    return false;
  }
}

export function isGuid(q: string): boolean {
  return GUID_RE.test(q);
}

export function isEmail(q: string): boolean {
  return EMAIL_RE.test(q);
}

export function isUrl(q: string): boolean {
  return URL_RE.test(q);
}

export function isJiraKey(q: string): boolean {
  return JIRA_KEY_RE.test(q);
}

export function isPath(q: string): boolean {
  return PATH_RE.test(q);
}

export function isSql(q: string): boolean {
  return SQL_FIRST_WORD_RE.test(q);
}
