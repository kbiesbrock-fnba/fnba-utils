// "Soft commands" for time / date / epoch conversions.
// Triggered when the palette query starts with ")".
// Pure TypeScript — no Tauri calls here; callers pass in copyText.

import type { PaletteCommand } from "@/commands/types";
import { copyText } from "@/lib/tauri";

// ─── Helpers ─────────────────────────────────────────────────────────────────

function padZ(n: number, w = 2): string {
  return String(n).padStart(w, "0");
}

/** "2026-06-12 14:05:33" local time from a Date object. */
function localDatetime(d: Date): string {
  return (
    `${d.getFullYear()}-${padZ(d.getMonth() + 1)}-${padZ(d.getDate())}` +
    ` ${padZ(d.getHours())}:${padZ(d.getMinutes())}:${padZ(d.getSeconds())}`
  );
}

/** ISO 8601 UTC string truncated to seconds: "2026-06-12T12:05:33Z". */
function isoUtc(d: Date): string {
  return d.toISOString().replace(/\.\d{3}Z$/, "Z");
}

/** Epoch seconds (integer). */
function epochSec(d: Date): string {
  return String(Math.floor(d.getTime() / 1000));
}

/** Epoch milliseconds. */
function epochMs(d: Date): string {
  return String(d.getTime());
}

/** Day-of-week name. */
const DOW = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

/** Human-readable relative string ("3 days ago", "in 2 hours", "just now"). */
function relative(d: Date): string {
  const diffMs = d.getTime() - Date.now();
  const diffSec = Math.round(diffMs / 1000);
  const abs = Math.abs(diffSec);
  const suffix = diffSec < 0 ? " ago" : "";
  const prefix = diffSec >= 0 ? "in " : "";
  if (abs < 60) return "just now";
  const mins = Math.floor(abs / 60);
  if (mins < 60) return diffSec < 0 ? `${mins} minute${mins === 1 ? "" : "s"} ago` : `in ${mins} minute${mins === 1 ? "" : "s"}`;
  const hours = Math.floor(abs / 3600);
  if (hours < 24) return `${prefix}${hours} hour${hours === 1 ? "" : "s"}${suffix}`;
  const days = Math.floor(abs / 86400);
  if (days < 30) return `${prefix}${days} day${days === 1 ? "" : "s"}${suffix}`;
  const weeks = Math.floor(days / 7);
  if (weeks < 8) return `${prefix}${weeks} week${weeks === 1 ? "" : "s"}${suffix}`;
  const months = Math.floor(days / 30);
  return `${prefix}${months} month${months === 1 ? "" : "s"}${suffix}`;
}

// ─── Row builder ──────────────────────────────────────────────────────────────

function timeRow(
  id: string,
  name: string,
  description: string,
  value: string,
): PaletteCommand {
  return {
    id,
    name,
    description,
    icon: "🕐",
    keywords: [],
    soft: true,
    action: () => copyText(value),
  };
}

/** Rows for a known Date: local datetime, ISO UTC, relative. */
function dateRows(d: Date, idBase: string): PaletteCommand[] {
  return [
    timeRow(`${idBase}:local`, localDatetime(d), "Local datetime", localDatetime(d)),
    timeRow(`${idBase}:iso`, isoUtc(d), "ISO 8601 UTC", isoUtc(d)),
    timeRow(`${idBase}:rel`, relative(d), DOW[d.getDay()], relative(d)),
  ];
}

// ─── Date-math parser ─────────────────────────────────────────────────────────

const UNIT_MS: Record<string, number> = {
  m: 60_000,
  h: 3_600_000,
  d: 86_400_000,
  w: 604_800_000,
};

/**
 * Parse a date-math expression like "now+30d" or "1718200000-2h".
 * `base` may be "now", a 10/13-digit epoch number, or a parseable date string.
 * Returns null for anything that doesn't match or parses to NaN.
 */
function parseDateMath(raw: string): Date | null {
  const m = raw.match(/^(.+?)([+-])(\d+(?:\.\d+)?)([mhdw])$/i);
  if (!m) return null;
  const [, baseStr, sign, amtStr, unitStr] = m;
  const unit = unitStr.toLowerCase();
  const amt = parseFloat(amtStr);
  if (isNaN(amt) || !UNIT_MS[unit]) return null;

  let baseMs: number;
  const baseTrimmed = baseStr.trim().toLowerCase();
  if (baseTrimmed === "now") {
    baseMs = Date.now();
  } else if (/^\d{10}$/.test(baseTrimmed)) {
    baseMs = parseInt(baseTrimmed, 10) * 1000;
  } else if (/^\d{13}$/.test(baseTrimmed)) {
    baseMs = parseInt(baseTrimmed, 10);
  } else {
    const parsed = new Date(baseStr.trim());
    if (isNaN(parsed.getTime())) return null;
    baseMs = parsed.getTime();
  }

  const delta = (sign === "+" ? 1 : -1) * amt * UNIT_MS[unit];
  const result = new Date(baseMs + delta);
  return isNaN(result.getTime()) ? null : result;
}

// ─── Public entry point ───────────────────────────────────────────────────────

/**
 * Build soft-command rows for a query starting with ")".
 * `q` should be the full raw query (including the ")"); returns [] on no match.
 */
export function buildTimeRows(q: string): PaletteCommand[] {
  if (!q.startsWith(")")) return [];
  const remainder = q.slice(1).trim();
  const lower = remainder.toLowerCase();

  // --- "now" or empty → current time in multiple formats ---
  if (lower === "now" || lower === "") {
    const now = new Date();
    return [
      timeRow("soft:time:local", localDatetime(now), "Local datetime", localDatetime(now)),
      timeRow("soft:time:iso", isoUtc(now), "ISO 8601 UTC", isoUtc(now)),
      timeRow("soft:time:epoch", epochSec(now), "Epoch seconds", epochSec(now)),
      timeRow("soft:time:epochms", epochMs(now), "Epoch milliseconds", epochMs(now)),
    ];
  }

  // --- 10-digit epoch seconds ---
  if (/^\d{10}$/.test(remainder)) {
    const d = new Date(parseInt(remainder, 10) * 1000);
    if (!isNaN(d.getTime())) {
      return [
        timeRow("soft:time:ep:local", localDatetime(d), "Local datetime", localDatetime(d)),
        timeRow("soft:time:ep:iso", isoUtc(d), "ISO 8601 UTC", isoUtc(d)),
        timeRow("soft:time:ep:rel", relative(d), DOW[d.getDay()], relative(d)),
      ];
    }
  }

  // --- 13-digit epoch milliseconds ---
  if (/^\d{13}$/.test(remainder)) {
    const d = new Date(parseInt(remainder, 10));
    if (!isNaN(d.getTime())) {
      return [
        timeRow("soft:time:ms:local", localDatetime(d), "Local datetime", localDatetime(d)),
        timeRow("soft:time:ms:iso", isoUtc(d), "ISO 8601 UTC", isoUtc(d)),
        timeRow("soft:time:ms:rel", relative(d), DOW[d.getDay()], relative(d)),
      ];
    }
  }

  // --- Date math: "now+30d", "1718200000-2h", etc. ---
  const mathResult = parseDateMath(remainder);
  if (mathResult) {
    return [
      timeRow("soft:time:math:local", localDatetime(mathResult), "Local datetime", localDatetime(mathResult)),
      timeRow("soft:time:math:epoch", epochSec(mathResult), "Epoch seconds", epochSec(mathResult)),
      timeRow("soft:time:math:iso", isoUtc(mathResult), "ISO 8601 UTC", isoUtc(mathResult)),
    ];
  }

  // --- Parseable date string (guarded by !isNaN) ---
  if (remainder.length >= 4) {
    const d = new Date(remainder);
    if (!isNaN(d.getTime())) {
      return [
        timeRow("soft:time:ds:epoch", epochSec(d), "Epoch seconds", epochSec(d)),
        timeRow("soft:time:ds:iso", isoUtc(d), "ISO 8601 UTC", isoUtc(d)),
        timeRow(
          "soft:time:ds:dow",
          `${DOW[d.getDay()]} · ${relative(d)}`,
          localDatetime(d),
          `${DOW[d.getDay()]} · ${relative(d)}`,
        ),
      ];
    }
  }

  // No match
  return [];
}
