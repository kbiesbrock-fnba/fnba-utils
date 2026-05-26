// Build the fnba-clipd daemon, first killing any running instance so cargo
// can overwrite the existing exe. Windows holds open files (including
// running .exe images) and `cargo build` will fail with "Access is denied"
// if the daemon is currently capturing.
//
// Cross-shell: runs under cmd (npm script default on Windows) or sh/bash via
// node, so we don't have to worry about taskkill-vs-pkill quoting.

import { spawnSync } from "node:child_process";
import { platform } from "node:os";

const isWindows = platform() === "win32";

function killDaemon() {
  if (!isWindows) return; // daemon is Windows-only
  // /F = force, /T = also kill child processes. Exit 128 if not running;
  // we don't care either way, so stdio:'ignore' swallows everything.
  const r = spawnSync("taskkill", ["/IM", "fnba-clipd.exe", "/F", "/T"], {
    stdio: "ignore",
    shell: false,
  });
  if (r.error) {
    // taskkill missing entirely (not Windows, somehow): nothing to do.
    return;
  }
  // Brief settle so Windows fully releases the file handle.
  if (r.status === 0) {
    const until = Date.now() + 500;
    while (Date.now() < until) {
      // busy-wait briefly — Node has no built-in sync sleep, and the cost
      // is bounded + only paid when we actually killed something.
    }
  }
}

function buildDaemon() {
  const r = spawnSync(
    "cargo",
    ["build", "--manifest-path", "src-tauri/Cargo.toml", "-p", "fnba-clipd"],
    { stdio: "inherit", shell: true },
  );
  process.exit(r.status ?? 1);
}

killDaemon();
buildDaemon();
