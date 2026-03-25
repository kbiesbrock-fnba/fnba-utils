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
