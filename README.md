# fnba-utils

Shared shell extensions and utility scripts for FNBA development.

## Install

Add this line to `~/.bashrc` (before completions and after core shell setup):

```bash
source "$HOME/dev/kbiesbrock-fnba/fnba-utils/.bashrc"
```

Then reload:

```bash
source ~/.bashrc
```

### Secrets

Copy the env template and fill in your tokens:

```bash
cp bashrc.d/startup-files/.env.example bashrc.d/startup-files/.env
```

`.env` is gitignored and will be sourced automatically.

## What's included

- `bashrc.d/startup-files/env.sh` - PATH, JAVA_HOME, Node CA certs
- `bashrc.d/startup-files/.env` - Secret tokens (gitignored)
- `bashrc.d/functions/cd-shortcuts.sh` - `cdd`, `cdf`, `cdk` with tab completion
- `bashrc.d/functions/clone.sh` - Clone or update FNBA-Software repos
- `bashrc.d/functions/mkcd.sh` - Create a directory and cd into it
- `bashrc.d/functions/open.sh` - Open file manager from terminal
- `bashrc.d/functions/pwd.sh` - pwd that also copies path to clipboard
- `bashrc.d/functions/rebash.sh` - Reload ~/.bashrc
- `bashrc.d/functions/wwd.sh` - Print and copy Windows path of cwd
- `assumeIdentity/` - Assume SQL identities against FNBA servers
- `app/` - FNBA Utils desktop app (Tauri + Vue command palette)

## Desktop App

`app/` is a Tauri v2 + Vue 3 command palette that opens via `Win+Shift+F`. It provides a Raycast/Spotlight-style launcher for FNBA utilities.

### Dev (UI only — no Rust required)

```bash
cd app
docker compose up
```

Open http://localhost:5173 in a browser. The Tauri API is mocked with realistic sample data so the full UI is interactive. Source edits trigger instant HMR.

### Dev (native — requires Windows Rust toolchain)

```bash
cd app
bash scripts/dev.sh
```

Requires Rust + MSVC Build Tools + Tauri CLI installed on Windows. See `scripts/dev.sh` for details.

### Current commands

- **Assume Identity** — Switch SQL identity on a target server. Two-step picker (user → connection) with search filtering.

## Optional tools

These are not included in fnba-utils but are useful for FNBA development. Commented-out loader blocks are in `.bashrc` — uncomment them after installing.

### NVM (Node Version Manager)

Manage multiple Node.js versions per project.

```bash
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.1/install.sh | bash
```

Then uncomment the NVM block in `.bashrc` and reload. Usage: `nvm install 20`, `nvm use 20`.

See: https://github.com/nvm-sh/nvm

### fzf (fuzzy finder)

Interactive filtering for files, command history (`Ctrl+R`), and more.

```bash
sudo apt install fzf
```

Then uncomment the fzf block in `.bashrc` and reload.

See: `dpkg -L fzf` for installed files, `/usr/share/doc/fzf/` for docs.

### SDKMAN (Software Development Kit Manager)

Manage parallel versions of Java, Gradle, Maven, and other JVM tools.

```bash
curl -s "https://get.sdkman.io" | bash
```

Then uncomment the SDKMAN block in `.bashrc` and reload. Usage: `sdk install java 21.0.2-tem`, `sdk use gradle 8.5`.

**Note:** SDKMAN's init must run last in `.bashrc` — keep it at the bottom.

See: https://sdkman.io
