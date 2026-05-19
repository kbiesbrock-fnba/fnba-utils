// Compute the app version string from package.json + git state.
// Format: <base>-<commit-count>-g<short-sha>[+dirty]
//   base       MAJOR.MINOR.PATCH from app/package.json
//   count      `git rev-list --count HEAD`
//   short-sha  `git rev-parse --short HEAD`
//   +dirty     appended when the working tree has uncommitted changes
//
// Used by app/vite.config.ts (frontend) and mirrored by app/src-tauri/build.rs (Rust).
// Both implementations must produce the same string so the tray About and the
// palette display agree.

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const appDir = join(scriptDir, "..");

function tryGit(args) {
  try {
    return execFileSync("git", args, { cwd: appDir, stdio: ["ignore", "pipe", "ignore"] })
      .toString()
      .trim();
  } catch {
    return null;
  }
}

function isDirty() {
  try {
    execFileSync("git", ["diff-index", "--quiet", "HEAD", "--"], {
      cwd: appDir,
      stdio: "ignore",
    });
    return false;
  } catch {
    return true;
  }
}

export function computeVersion() {
  const pkg = JSON.parse(readFileSync(join(appDir, "package.json"), "utf8"));
  const base = pkg.version || "0.0.0";
  const count = tryGit(["rev-list", "--count", "HEAD"]) ?? "0";
  const sha = tryGit(["rev-parse", "--short", "HEAD"]) ?? "unknown";
  const dirty = isDirty() ? "+dirty" : "";
  return `${base}-${count}-g${sha}${dirty}`;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  process.stdout.write(computeVersion());
}
