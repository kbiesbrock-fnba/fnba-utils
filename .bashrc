# fnba-utils shell extensions
# Source this file from ~/.bashrc:
#   source "$HOME/dev/kbiesbrock-fnba/fnba-utils/.bashrc"

export FNBA_UTILS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Environment and startup config first
for f in "$FNBA_UTILS_DIR/bashrc.d/startup-files"/*.sh; do
    [ -r "$f" ] && source "$f"
done

# Shell functions and completions
for f in "$FNBA_UTILS_DIR/bashrc.d/functions"/*.sh; do
    [ -r "$f" ] && source "$f"
done

# --- Optional tools (uncomment after installing) ---

# NVM (Node Version Manager) - manage multiple Node.js versions
# Install: https://github.com/nvm-sh/nvm#installing-and-updating
#   curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.1/install.sh | bash
# export NVM_DIR="$HOME/.nvm"
# [ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"
# [ -s "$NVM_DIR/bash_completion" ] && \. "$NVM_DIR/bash_completion"

# fzf (fuzzy finder) - interactive filtering for files, history, etc.
# Install: sudo apt install fzf
# source /usr/share/doc/fzf/examples/key-bindings.bash
# [ -f /usr/share/bash-completion/completions/fzf ] && source /usr/share/bash-completion/completions/fzf

# SDKMAN (Software Development Kit Manager) - manage Java, Gradle, Maven, etc.
# Install: https://sdkman.io/install
#   curl -s "https://get.sdkman.io" | bash
# export SDKMAN_DIR="$HOME/.sdkman"
# [[ -s "$HOME/.sdkman/bin/sdkman-init.sh" ]] && source "$HOME/.sdkman/bin/sdkman-init.sh"
