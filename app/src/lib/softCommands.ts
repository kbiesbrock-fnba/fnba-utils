// "Soft commands" — contextual palette actions surfaced only when the typed
// query matches no real command but DOES match a recognizable pattern (a URL,
// a Jira key, a JSON blob, a math expression, …). They run a one-shot `action`
// and dismiss, rather than opening a component. See usePalette.filteredCommands.
//
// Stage 1 covers patterns that reuse existing infrastructure (no new Rust):
// URL, Jira issue key / Jira URL, JSON, and a calculator. Filesystem paths
// (open in Explorer / editor, file-vs-folder, terminal, Notepad++ workspace)
// land in Stage 2, which needs new path-aware Rust commands.

import type { PaletteCommand } from "@/commands/types";
import { copyText } from "@/lib/tauri";
import { openExternal } from "@/lib/external";
import { openNewJsonViewerWindow } from "@/lib/jsonViewerWindow";

const URL_RE = /^(https?:\/\/|www\.)\S+$/i;
const JIRA_KEY_RE = /^[A-Z][A-Z0-9]*-\d+$/;
const JIRA_IN_URL_RE = /\/browse\/([A-Z][A-Z0-9]*-\d+)/i;
const MATH_RE = /^[\d\s.+\-*/%()]+$/;

function isJsonText(q: string): boolean {
  if (q.length < 2 || (q[0] !== "{" && q[0] !== "[")) return false;
  try {
    JSON.parse(q);
    return true;
  } catch {
    return false;
  }
}

/** Evaluate a pure arithmetic expression. Returns null if it isn't safe/simple. */
function evalMath(expr: string): number | null {
  if (!MATH_RE.test(expr) || !/[+\-*/%]/.test(expr) || !/\d/.test(expr)) return null;
  try {
    // Input is constrained by MATH_RE to digits/whitespace/operators/parens.
    const value = Function(`"use strict"; return (${expr});`)() as unknown;
    return typeof value === "number" && Number.isFinite(value) ? value : null;
  } catch {
    return null;
  }
}

/** Surface a Jira issue in the in-app Issue panel (mirrors the standup flow). */
async function openIssuePanel(key: string): Promise<void> {
  // localStorage handoff guarantees the detail window picks up the key even if
  // it loaded before its event listener registered (first-open race).
  try {
    localStorage.setItem("fnba-utils:issue-detail-pending", key);
  } catch {
    // ignore
  }
  const { emit } = await import("@tauri-apps/api/event");
  const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
  const w = await WebviewWindow.getByLabel("issue-detail");
  if (w) {
    await w.show();
    await w.setFocus();
    await emit("issue-detail-open", { key });
  }
}

function row(c: Omit<PaletteCommand, "keywords" | "soft">): PaletteCommand {
  return { ...c, keywords: [], soft: true };
}

/**
 * Build the contextual soft-command rows for `query`, or `[]` if nothing
 * matches. Callers invoke this only when the normal command filter is empty.
 */
export function buildSoftCommands(query: string): PaletteCommand[] {
  const q = query.trim();
  if (!q) return [];

  // --- URL ---
  if (URL_RE.test(q)) {
    const url = /^www\./i.test(q) ? `https://${q}` : q;
    const out: PaletteCommand[] = [
      row({
        id: "soft:url:open",
        name: "Open in browser",
        description: url,
        icon: "🌐",
        action: () => openExternal(url),
      }),
    ];
    const jira = url.match(JIRA_IN_URL_RE);
    if (jira) {
      const key = jira[1].toUpperCase();
      out.push(
        row({
          id: "soft:url:issue",
          name: `Open ${key} in Issue panel`,
          description: "Jira issue",
          icon: "🐞",
          action: () => openIssuePanel(key),
        }),
      );
    }
    out.push(
      row({
        id: "soft:url:copy",
        name: "Copy URL",
        description: url,
        icon: "📋",
        action: () => copyText(url),
      }),
    );
    return out;
  }

  // --- Jira issue key (e.g. MIN-1234) ---
  if (JIRA_KEY_RE.test(q)) {
    const key = q.toUpperCase();
    return [
      row({
        id: "soft:jira:issue",
        name: `Open ${key} in Issue panel`,
        description: "Jira issue",
        icon: "🐞",
        action: () => openIssuePanel(key),
      }),
      row({
        id: "soft:jira:copy",
        name: "Copy key",
        description: key,
        icon: "📋",
        action: () => copyText(key),
      }),
    ];
  }

  // --- JSON blob ---
  if (isJsonText(q)) {
    return [
      row({
        id: "soft:json:view",
        name: "Open in JSON Viewer",
        description: `${q.length} chars of JSON`,
        icon: "🔍",
        action: () => openNewJsonViewerWindow(q),
      }),
      row({
        id: "soft:json:copy",
        name: "Copy JSON",
        description: `${q.length} chars`,
        icon: "📋",
        action: () => copyText(q),
      }),
    ];
  }

  // --- Calculator ---
  const result = evalMath(q);
  if (result !== null) {
    const text = String(result);
    return [
      row({
        id: "soft:calc",
        name: `= ${text}`,
        description: q,
        icon: "🧮",
        action: () => copyText(text),
      }),
    ];
  }

  return [];
}
