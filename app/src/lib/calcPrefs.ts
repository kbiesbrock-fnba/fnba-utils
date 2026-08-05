// Persistence for the calculator soft command:
//   - active trig unit (rad / deg / grad)
//   - expression+result history, newest first, capped at 50
//
// Both use try/catch-wrapped localStorage, matching the pattern in
// src/lib/fileViewerRegistry.ts — failures are silently ignored so the
// calculator keeps working even in private-browsing or quota-exhausted states.

import type { TrigUnit } from "@/lib/calc";

const UNIT_KEY    = "fnba-utils:calc-trig-unit";
const HISTORY_KEY = "fnba-utils:calc-history";
const HISTORY_CAP = 50;

export interface CalcHistoryEntry {
  expr:   string;
  result: string;
  at:     number; // Date.now()
}

// ─── Trig unit ────────────────────────────────────────────────────────────────

export function getTrigUnit(): TrigUnit {
  try {
    const raw = localStorage.getItem(UNIT_KEY);
    if (raw === "deg" || raw === "rad" || raw === "grad") return raw;
  } catch {
    // ignore
  }
  return "rad";
}

export function setTrigUnit(unit: TrigUnit): void {
  try {
    localStorage.setItem(UNIT_KEY, unit);
  } catch {
    // ignore
  }
}

// ─── History ─────────────────────────────────────────────────────────────────

function readHistory(): CalcHistoryEntry[] {
  try {
    const raw = localStorage.getItem(HISTORY_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) return [];
    return parsed as CalcHistoryEntry[];
  } catch {
    return [];
  }
}

function writeHistory(entries: CalcHistoryEntry[]): void {
  try {
    localStorage.setItem(HISTORY_KEY, JSON.stringify(entries));
  } catch {
    // ignore
  }
}

export function getHistory(): CalcHistoryEntry[] {
  return readHistory();
}

/** Prepend an entry; skip if it duplicates the most-recent expr+result pair. */
export function addHistory(expr: string, result: string): void {
  const entries = readHistory();
  const newest = entries[0];
  if (newest && newest.expr === expr && newest.result === result) return;

  entries.unshift({ expr, result, at: Date.now() });
  writeHistory(entries.slice(0, HISTORY_CAP));
}

export function clearHistory(): void {
  try {
    localStorage.removeItem(HISTORY_KEY);
  } catch {
    // ignore
  }
}
