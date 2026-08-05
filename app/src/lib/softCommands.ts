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
import { copyText, runInTerminal, openInExplorer, openPathInEditor, resolvePath, openInNotepadpp, revealInExplorer, openWithDefault, readTextFile } from "@/lib/tauri";
import { openExternal } from "@/lib/external";
import { openNewJsonViewerWindow } from "@/lib/jsonViewerWindow";
import { openNewMarkdownViewerWindow } from "@/lib/markdownViewerWindow";
import { looksLikeMarkdown } from "@/lib/markdownDetect";
import { evaluate, formatResult, usesTrig } from "@/lib/calc";
import {
  getTrigUnit,
  setTrigUnit,
  getHistory,
  addHistory,
  clearHistory,
} from "@/lib/calcPrefs";
import type { TrigUnit } from "@/lib/calc";
import { buildTimeRows } from "@/lib/timeSoft";
import { URL_RE, JIRA_KEY_RE, JIRA_IN_URL_RE, isJsonText, isPath, stripPathQuotes } from "@/lib/patterns";

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

function row(
  c: Omit<PaletteCommand, "keywords" | "soft">,
): PaletteCommand {
  return { ...c, keywords: [], soft: true };
}

function markdownRows(content: string): PaletteCommand[] {
  return [
    row({
      id: "soft:markdown:view",
      name: "Open in Markdown Viewer",
      description: `${content.length} chars of Markdown`,
      icon: "📝",
      action: () => openNewMarkdownViewerWindow(content),
    }),
    row({
      id: "soft:markdown:copy",
      name: "Copy Markdown",
      description: `${content.length} chars`,
      icon: "📋",
      action: () => copyText(content),
    }),
  ];
}

// ─── Relative timestamp (for history rows) ────────────────────────────────────

function relativeTime(at: number): string {
  const diff = Date.now() - at;
  const s = Math.floor(diff / 1000);
  if (s < 60) return "just now";
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ago`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h ago`;
  return `${Math.floor(h / 24)}d ago`;
}

// ─── Calculator rows ──────────────────────────────────────────────────────────

const UNIT_LABELS: Record<TrigUnit, string> = {
  rad: "radians",
  deg: "degrees",
  grad: "gradians",
};

function buildCalcRows(expr: string): PaletteCommand[] {
  const unit = getTrigUnit();
  const value = evaluate(expr, unit);
  if (value === null) return [];

  const formatted = formatResult(value);
  const trigHint = usesTrig(expr) ? ` · ${unit}` : "";
  const desc = `${expr}${trigHint}`;

  const rows: PaletteCommand[] = [
    row({
      id: "soft:calc:copy",
      name: `= ${formatted}`,
      description: desc,
      icon: "🧮",
      action: () => {
        addHistory(expr, formatted);
        copyText(formatted);
      },
      chainQuery: `=${formatted}`,
    }),
  ];

  // Extra base-conversion rows for whole-number results.
  if (Number.isInteger(value) && Math.abs(value) <= Number.MAX_SAFE_INTEGER) {
    const sign = value < 0 ? "-" : "";
    const abs = Math.abs(value);
    rows.push(
      row({
        id: "soft:calc:hex",
        name: `${sign}0x${abs.toString(16).toUpperCase()}`,
        description: "Copy as hexadecimal",
        icon: "#️⃣",
        action: () => copyText(`${sign}0x${abs.toString(16).toUpperCase()}`),
      }),
      row({
        id: "soft:calc:bin",
        name: `${sign}0b${abs.toString(2)}`,
        description: "Copy as binary",
        icon: "#️⃣",
        action: () => copyText(`${sign}0b${abs.toString(2)}`),
      }),
      row({
        id: "soft:calc:oct",
        name: `${sign}0o${abs.toString(8)}`,
        description: "Copy as octal",
        icon: "#️⃣",
        action: () => copyText(`${sign}0o${abs.toString(8)}`),
      }),
    );
  }

  return rows;
}

// ─── Path soft commands ───────────────────────────────────────────────────────

// Extensions Notepad++ opens as text. Anything file-shaped that isn't here is
// treated as "opaque" (PDF, xlsx, image, archive, exe) — Notepad++ would just
// render mojibake, so it sinks to the bottom of the list.
const TEXT_EXTS = new Set([
  "txt", "log", "rst", "adoc",
  "sql", "xml", "xsd", "xsl", "yaml", "yml", "toml",
  "ini", "conf", "config", "cfg", "env", "properties", "csv", "tsv",
  "cs", "vb", "fs", "java", "kt", "kts", "scala", "groovy", "gradle",
  "rb", "php", "pl", "pm", "lua", "r", "go", "rs", "swift", "dart",
  "c", "h", "cpp", "cxx", "cc", "hpp", "hh", "m", "mm",
  "js", "mjs", "cjs", "jsx", "ts", "mts", "cts", "tsx", "vue", "svelte",
  "html", "htm", "cshtml", "razor", "css", "scss", "sass", "less",
  "sh", "bash", "zsh", "ps1", "psm1", "psd1", "bat", "cmd",
  "diff", "patch", "resx", "sln", "csproj", "vbproj", "props", "targets",
  "tf", "tfvars", "http", "graphql", "proto", "sqlproj", "dtsx", "lock",
]);

// Text files that carry no extension — otherwise indistinguishable from a
// folder by string shape alone.
const TEXT_STEMS = new Set([
  "makefile", "dockerfile", "jenkinsfile", "vagrantfile", "gemfile",
  "rakefile", "procfile", "brewfile", "license", "licence", "readme",
  "changelog", "notice", "authors", "contributing", "codeowners", "todo",
]);

const MARKDOWN_EXTS = new Set(["md", "markdown"]);
const JSON_EXTS = new Set(["json", "jsonc"]);

type PathKind = "dir" | "markdown" | "json" | "text" | "opaque";

/**
 * Classify a pasted path from its string shape alone. `buildSoftCommands` is
 * synchronous (it feeds a computed), so we can't await `resolve_path` here —
 * and this only decides row *order*, so a wrong guess costs an arrow key, not
 * a broken action.
 */
function classifyPath(rawPath: string): PathKind {
  const p = rawPath.replace(/[/\\]+$/, "");
  if (p !== rawPath) return "dir"; // trailing separator: explicitly a folder
  const seg = p.split(/[/\\]/).pop() ?? "";
  if (!seg) return "dir";

  // Dotfile with no further extension (.bashrc, .gitignore, .env) — text.
  if (seg.startsWith(".") && !seg.slice(1).includes(".")) return "text";
  if (TEXT_STEMS.has(seg.toLowerCase())) return "text";

  const dot = seg.lastIndexOf(".");
  if (dot <= 0) return "dir"; // no extension: assume folder
  const ext = seg.slice(dot + 1).toLowerCase();
  if (MARKDOWN_EXTS.has(ext)) return "markdown";
  if (JSON_EXTS.has(ext)) return "json";
  return TEXT_EXTS.has(ext) ? "text" : "opaque";
}

function buildPathRows(rawPath: string): PaletteCommand[] {
  const explorer = row({
    id: "soft:path:explorer",
    name: "Show in Explorer",
    description: rawPath,
    icon: "📂",
    action: async () => {
      const r = await resolvePath(rawPath);
      // File: reveal it highlighted in its folder. Dir/unknown: open directly.
      if (r.exists && r.isFile) {
        await revealInExplorer(r.windows);
      } else {
        await openInExplorer(r.windows);
      }
    },
  });

  const editor = row({
    id: "soft:path:editor",
    name: "Open in editor",
    description: "IntelliJ → Explorer fallback",
    icon: "✏️",
    action: async () => {
      const r = await resolvePath(rawPath);
      await openPathInEditor(r.windows);
    },
  });

  const terminal = row({
    id: "soft:path:terminal",
    name: "Open terminal here",
    description: rawPath,
    icon: "💻",
    action: async () => {
      const r = await resolvePath(rawPath);
      // cd to posix dir; strip filename segment if it's a file.
      const dir = r.exists && r.isFile
        ? r.posix.replace(/\/[^/]+$/, "") || r.posix
        : r.posix;
      await runInTerminal(`cd ${JSON.stringify(dir)}`);
    },
  });

  const defaultApp = row({
    id: "soft:path:default",
    name: "Open",
    description: "Default app for this file type",
    icon: "🚀",
    action: async () => {
      const r = await resolvePath(rawPath);
      await openWithDefault(r.windows);
    },
  });

  const markdownViewer = row({
    id: "soft:path:markdown-viewer",
    name: "Open in Markdown Viewer",
    description: rawPath,
    icon: "📝",
    action: async () => {
      const r = await resolvePath(rawPath);
      const content = await readTextFile(r.windows);
      await openNewMarkdownViewerWindow(content, r.windows);
    },
  });

  const jsonViewer = row({
    id: "soft:path:json-viewer",
    name: "Open in JSON Viewer",
    description: rawPath,
    icon: "🔍",
    action: async () => {
      const r = await resolvePath(rawPath);
      const content = await readTextFile(r.windows);
      await openNewJsonViewerWindow(content);
    },
  });

  const notepadpp = row({
    id: "soft:path:notepadpp",
    name: "Open in Notepad++",
    description: rawPath,
    icon: "📝",
    action: async () => {
      const r = await resolvePath(rawPath);
      await openInNotepadpp(r.windows);
    },
  });

  const copy = row({
    id: "soft:path:copy",
    name: "Copy path",
    description: rawPath,
    icon: "📋",
    action: async () => {
      const r = await resolvePath(rawPath);
      await copyText(r.windows);
    },
  });

  // First row = Enter with no arrow keys, so it has to be the obvious verb for
  // what was pasted: read a text file, browse a folder, and for anything else
  // (PDF/xlsx/image) hand it to the app that owns that file type.
  switch (classifyPath(rawPath)) {
    case "markdown":
      return [markdownViewer, notepadpp, editor, defaultApp, explorer, terminal, copy];
    case "json":
      return [jsonViewer, notepadpp, editor, defaultApp, explorer, terminal, copy];
    case "text":
      return [notepadpp, editor, defaultApp, explorer, terminal, copy];
    case "dir":
      // No Notepad++ (can't open a folder) and no "Open" — for a directory the
      // default handler IS Explorer, so it would duplicate the first row.
      return [explorer, terminal, editor, copy];
    case "opaque":
      return [defaultApp, explorer, copy, terminal, editor, notepadpp];
  }
}

/**
 * Build the contextual soft-command rows for `query`, or `[]` if nothing
 * matches. Callers invoke this only when the normal command filter is empty.
 */
export function buildSoftCommands(query: string): PaletteCommand[] {
  const q = query.trim();
  if (!q) return [];

  // --- ">" run-in-terminal prefix ---
  if (q.startsWith(">")) {
    const cmd = q.slice(1).trim();
    if (!cmd) return [];
    return [
      row({
        id: "soft:terminal:run",
        name: "Run in terminal",
        description: cmd,
        icon: "💻",
        action: () => runInTerminal(cmd),
      }),
      row({
        id: "soft:terminal:copy",
        name: "Copy command",
        description: cmd,
        icon: "📋",
        action: () => copyText(cmd),
      }),
    ];
  }

  // --- ")" time/date prefix ---
  if (q.startsWith(")")) {
    return buildTimeRows(q);
  }

  // --- "md " markdown prefix ---
  if (q.startsWith("md ")) {
    const body = q.slice(3).trim();
    if (!body) return [];
    return markdownRows(body);
  }

  // --- Filesystem path (C:\…, /mnt/c/…, /path/…, "C:\quoted\path") ---
  if (isPath(q)) {
    return buildPathRows(stripPathQuotes(q));
  }

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
  // Triggered by a leading "=" (e.g. "=2*(3+4)"). The "=" is required because
  // the palette's 1-9 launch hotkeys swallow a leading digit while the search
  // is empty — typing "=" first makes the search non-empty so digits type
  // normally. It also avoids false positives (a bare "2024" isn't a sum).
  if (q.startsWith("=")) {
    const raw = q.slice(1).trim();

    // "=deg" / "=rad" / "=grad" — trig unit switcher
    if (/^(deg|rad|grad)$/i.test(raw)) {
      const next = raw.toLowerCase() as TrigUnit;
      const current = getTrigUnit();
      return [
        row({
          id: "soft:calc:unit",
          name: `Set trig unit to ${UNIT_LABELS[next]}`,
          description: `Currently: ${UNIT_LABELS[current]}`,
          icon: "📐",
          action: () => setTrigUnit(next),
        }),
      ];
    }

    // Bare "=" — show history
    if (raw === "") {
      const history = getHistory();
      const out: PaletteCommand[] = history.slice(0, 9).map((entry, i) =>
        row({
          id: `soft:calc:hist:${i}`,
          name: `= ${entry.result}`,
          description: `${entry.expr} · ${relativeTime(entry.at)}`,
          icon: "🧮",
          action: () => copyText(entry.result),
          chainQuery: `=${entry.result}`,
        }),
      );
      if (history.length > 0) {
        out.push(
          row({
            id: "soft:calc:hist:clear",
            name: "Clear calculator history",
            description: `${history.length} saved ${history.length === 1 ? "entry" : "entries"}`,
            icon: "🗑️",
            action: () => clearHistory(),
          }),
        );
      }
      return out;
    }

    // Expression — evaluate and build result rows
    return buildCalcRows(raw);
  }

  // --- Markdown content sniff (LAST: all other patterns have priority) ---
  if (looksLikeMarkdown(q)) {
    return markdownRows(q);
  }

  return [];
}
