// Stage the release exe + README + release notes + example config into a
// portable folder and zip it via PowerShell's Compress-Archive. Run after
// `tauri build --no-bundle`.
//
// Sources:
//   app/README.md         -> bundled as README.md
//   app/RELEASE_NOTES.md  -> bundled as RELEASE_NOTES.md (verbatim)
//   exe                   -> bundled as fnba-utils.exe
//   inline example config -> bundled as example.assumeIdentity.json
//
// Output: <repo-root>/releases/fnba-utils-portable-<version>.zip
// Staging: app/dist-portable/<name>/  (gitignored)
import { readFileSync, writeFileSync, mkdirSync, rmSync, copyFileSync, existsSync, statSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(scriptDir, "..");
const appDir = join(repoRoot, "app");
const pkg = JSON.parse(readFileSync(join(appDir, "package.json"), "utf8"));
const version = pkg.version;

const exeSrc = join(appDir, "src-tauri", "target", "release", "fnba-utils.exe");
const readmeSrc = join(appDir, "README.md");
const releaseNotesSrc = join(appDir, "RELEASE_NOTES.md");

const stageRoot = join(appDir, "dist-portable");
const stageDir = join(stageRoot, `fnba-utils-portable-${version}`);

const releasesDir = join(repoRoot, "releases");
const zipPath = join(releasesDir, `fnba-utils-portable-${version}.zip`);

for (const [label, path] of [
  ["release binary", exeSrc],
  ["app/README.md", readmeSrc],
  ["app/RELEASE_NOTES.md", releaseNotesSrc],
]) {
  if (!existsSync(path)) {
    console.error(`ERROR: ${label} not found at ${path}`);
    if (label === "release binary") {
      console.error(`Run \`tauri build --no-bundle\` first (or use \`npm run package\` which does both).`);
    }
    process.exit(1);
  }
}

rmSync(stageDir, { recursive: true, force: true });
rmSync(zipPath, { force: true });
mkdirSync(stageDir, { recursive: true });
mkdirSync(releasesDir, { recursive: true });

copyFileSync(exeSrc, join(stageDir, "fnba-utils.exe"));
copyFileSync(readmeSrc, join(stageDir, "README.md"));
copyFileSync(releaseNotesSrc, join(stageDir, "RELEASE_NOTES.md"));

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
writeFileSync(join(stageDir, "example.assumeIdentity.json"), example);

const psCmd = `Compress-Archive -Force -Path '${stageDir}\\*' -DestinationPath '${zipPath}'`;
execFileSync("powershell.exe", ["-NoProfile", "-Command", psCmd], { stdio: "inherit" });

const exeBytes = statSync(join(stageDir, "fnba-utils.exe")).size;
const zipBytes = statSync(zipPath).size;
const mb = (n) => (n / 1024 / 1024).toFixed(1) + " MB";

console.log("");
console.log(`Staged:  ${stageDir}`);
console.log(`         fnba-utils.exe (${mb(exeBytes)})`);
console.log(`         README.md`);
console.log(`         RELEASE_NOTES.md`);
console.log(`         example.assumeIdentity.json`);
console.log(`Zipped:  ${zipPath} (${mb(zipBytes)})`);
