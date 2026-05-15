// Stage the release exe + README + example config into a portable folder
// and zip it via PowerShell's Compress-Archive. Run after `tauri build --no-bundle`.
import { readFileSync, writeFileSync, mkdirSync, rmSync, copyFileSync, existsSync, statSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const appDir = join(scriptDir, "..", "app");
const pkg = JSON.parse(readFileSync(join(appDir, "package.json"), "utf8"));
const version = pkg.version;

const exeSrc = join(appDir, "src-tauri", "target", "release", "fnba-utils.exe");
const outRoot = join(appDir, "dist-portable");
const stageDir = join(outRoot, `fnba-utils-portable-${version}`);
const zipPath = join(outRoot, `fnba-utils-portable-${version}.zip`);

if (!existsSync(exeSrc)) {
  console.error(`ERROR: release binary not found at ${exeSrc}`);
  console.error(`Run \`tauri build --no-bundle\` first (or use \`npm run package\` which does both).`);
  process.exit(1);
}

rmSync(stageDir, { recursive: true, force: true });
rmSync(zipPath, { force: true });
mkdirSync(stageDir, { recursive: true });

copyFileSync(exeSrc, join(stageDir, "fnba-utils.exe"));

const readme = `FNBA Utils - portable build ${version}
==================================

A Raycast/Spotlight-style command palette for FNBA developers. The current
command is "Assume Identity" - pick a user + SQL Server, hit confirm, and
the app runs logincheck.fnba.assumeIdentity for you and shows before/after.


REQUIREMENTS
------------
- Windows 10 or 11 (Webview2 is built in)
- FNBA network access on TCP 1433 to the SQL Server hosts (VPN if remote)
- A Windows domain login - the app uses SSPI / integrated auth, no
  passwords are stored or asked for


HOW TO RUN
----------
1. Unzip this folder anywhere (Desktop, Documents, %LOCALAPPDATA%, etc.).
   No installer, no admin rights needed.
2. Double-click fnba-utils.exe. A window opens; you can close it - the
   app keeps running in the background.
3. Global hotkeys (registered automatically while the app is running):
     Win+Shift+F  -> open the command palette (Assume Identity)
     Win+Shift+C  -> open Mission Control (see "Optional" section below)
4. To launch on every login, drop a shortcut to fnba-utils.exe into:
        shell:startup
   (paste that into the Run dialog - Win+R - to open the startup folder.)


ASSUME IDENTITY - what's built in
---------------------------------
The three FNBA SQL servers and the default staff list are baked into the
exe, so it works out of the box:
  - Local        dsqlaleroy.fnba-dev.network
  - Development  meleagris.fnba.com
  - Staging      caster.fnba.com
Plus ~36 default users (developers / QA / etc.) selectable from the
picker. Your own Windows username is always available as an "imposter"
(the identity you authenticate as before assuming someone else's).


CUSTOM USERS / CONNECTIONS / IMPOSTERS  (optional)
--------------------------------------------------
Want extras beyond the defaults? Create this file:

    %USERPROFILE%\\.assumeIdentity.json

(That resolves to e.g. C:\\Users\\jsmith\\.assumeIdentity.json. In Notepad,
File > Save As with quotes around the filename so it doesn't get a .txt
extension.)

Schema - all three sections are optional:

    {
      "Imposters": ["username1", "username2"],
      "Users": [
        { "label": "QA Lead",   "username": "qauser1" },
        { "label": "DBA",       "username": "dba1"    }
      ],
      "Connections": [
        { "label": "My Sandbox", "server": "sandbox.fnba.com" },
        "another-server.fnba.com"
      ]
    }

A ready-to-edit template is included next to this README:
    example.assumeIdentity.json

Behavior:
  - Entries merge with the built-in defaults (they don't replace them).
  - Duplicates are skipped case-insensitively (by username / server).
  - Custom entries are flagged in the UI and can be deleted from the
    picker - that writes back to your .assumeIdentity.json.
  - Changes are picked up on next launch of the app.


MISSION CONTROL  (optional, advanced)
-------------------------------------
Win+Shift+C opens a separate window that monitors active Claude Code
sessions under ~/.claude/projects/ (both Windows and WSL homes). The
chat panel spawns "claude --resume" via WSL, so this feature only
works if you already have Claude Code installed inside WSL.

If you don't use Claude Code, just ignore this window - the Assume
Identity command palette works fully on its own.


TROUBLESHOOTING
---------------
Hotkey doesn't fire:
  Another app is holding Win+Shift+F. Close conflicts or open the visible
  window manually from the taskbar.

"Connection failed" in Assume Identity:
  Confirm VPN is up and you can reach the target SQL host on port 1433.
  From PowerShell:  Test-NetConnection meleagris.fnba.com -Port 1433

Anti-virus quarantine:
  The exe is unsigned. If endpoint security removes it, restore from
  quarantine or ask IT to whitelist com.fnba.utils.

App doesn't start / immediate crash:
  Run the exe from PowerShell so you can read any error output before
  the window exits.


VERSION
-------
fnba-utils ${version}
Identifier: com.fnba.utils
`;

const example = `{
  "Imposters": [
    "your.windows.username"
  ],
  "Users": [
    { "label": "QA Tester",  "username": "qa.tester" },
    { "label": "DBA",        "username": "dba.user"  }
  ],
  "Connections": [
    { "label": "My Sandbox", "server": "sandbox.fnba.com" },
    "another-server.fnba.com"
  ]
}
`;

writeFileSync(join(stageDir, "README.txt"), readme);
writeFileSync(join(stageDir, "example.assumeIdentity.json"), example);

const psCmd = `Compress-Archive -Force -Path '${stageDir}\\*' -DestinationPath '${zipPath}'`;
execFileSync("powershell.exe", ["-NoProfile", "-Command", psCmd], { stdio: "inherit" });

const exeBytes = statSync(join(stageDir, "fnba-utils.exe")).size;
const zipBytes = statSync(zipPath).size;
const mb = (n) => (n / 1024 / 1024).toFixed(1) + " MB";

console.log("");
console.log(`Staged:  ${stageDir}`);
console.log(`         fnba-utils.exe (${mb(exeBytes)})`);
console.log(`         README.txt`);
console.log(`         example.assumeIdentity.json`);
console.log(`Zipped:  ${zipPath} (${mb(zipBytes)})`);
