import type { ClipboardEntrySummary } from "@/lib/tauri";
import { isGuid, isEmail, isUrl, isJiraKey, isPath, isJsonText, isSql } from "@/lib/patterns";

export type DerivedLabel =
  | "image"
  | "sensitive"
  | "guid"
  | "email"
  | "url"
  | "jira"
  | "path"
  | "json"
  | "sql"
  | "html"
  | "text";

export function deriveLabel(entry: ClipboardEntrySummary): DerivedLabel {
  if (entry.kind === "image") return "image";
  if (entry.sensitive) return "sensitive";

  const raw = entry.textPreview;
  if (raw == null) return entry.kind as DerivedLabel;

  const t = raw.trim();

  if (isGuid(t)) return "guid";
  if (isEmail(t)) return "email";
  if (isUrl(t)) return "url";
  if (isJiraKey(t)) return "jira";
  if (isPath(t)) return "path";
  if (isJsonText(t)) return "json";
  if (isSql(t)) return "sql";

  return entry.kind === "html" ? "html" : "text";
}
