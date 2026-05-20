import type { PaletteCommand } from "./types";
import StandupCommand from "../components/standup/StandupCommand.vue";
import { getStandupLastRun } from "@/lib/tauri";

const BASE_DESCRIPTION = "Pull Jira tasks and post to Teams";

export function buildStandupCommand(description: string): PaletteCommand {
  return {
    id: "standup",
    name: "Standup",
    description,
    icon: "\u{1F4CB}", // 📋
    keywords: ["standup", "jira", "teams", "daily", "status"],
    component: StandupCommand,
  };
}

/** Format a "Last run: ..." suffix for the palette description. */
export async function buildStandupDescription(): Promise<string> {
  try {
    const last = await getStandupLastRun();
    if (!last) return `${BASE_DESCRIPTION} · Never run`;
    if (last.error) {
      return `${BASE_DESCRIPTION} · Last run failed (${humanAgo(last.at)})`;
    }
    const ago = humanAgo(last.at);
    const teamsBit = last.postedToTeams ? "posted" : "no Teams post";
    return `${BASE_DESCRIPTION} · Last run ${ago} (${last.issueCount} issues, ${teamsBit})`;
  } catch {
    return BASE_DESCRIPTION;
  }
}

function humanAgo(iso: string): string {
  const then = new Date(iso).getTime();
  if (Number.isNaN(then)) return iso;
  const diffMs = Date.now() - then;
  const mins = Math.floor(diffMs / 60_000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}h ago`;
  const days = Math.floor(hrs / 24);
  return `${days}d ago`;
}
