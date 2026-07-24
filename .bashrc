# fnba-utils shell extensions
# Source this file from ~/.bashrc:
#   source "path/to/fnba-utils/.bashrc"

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
export NVM_DIR="$HOME/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"
[ -s "$NVM_DIR/bash_completion" ] && \. "$NVM_DIR/bash_completion"

# SDKMAN (Software Development Kit Manager) - manage Java, Gradle, Maven, etc.
# Install: https://sdkman.io/install
#   curl -s "https://get.sdkman.io" | bash
export SDKMAN_DIR="$HOME/.sdkman"
[[ -s "$HOME/.sdkman/bin/sdkman-init.sh" ]] && source "$HOME/.sdkman/bin/sdkman-init.sh"

# fzf (fuzzy finder) - interactive filtering for files, history, etc.
# Install:
#   sudo apt remove fzf                                    # remove old apt version
#   sudo apt install fd-find
#   mkdir -p ~/.local/bin
#   ln -sf $(command -v fdfind) ~/.local/bin/fd
#   git -c core.autocrlf=false clone --depth 1 https://github.com/junegunn/fzf.git ~/.fzf
#   ~/.fzf/install   (answered "no" to bashrc update — handled below)

_FD_OPTS='--hidden --follow --no-require-git --exclude .git'
export FZF_DEFAULT_OPTS='--ansi --height=40% --layout=reverse --border'
export FZF_DEFAULT_COMMAND="fd --type f --strip-cwd-prefix --color=always $_FD_OPTS"
export FZF_CTRL_T_COMMAND="$FZF_DEFAULT_COMMAND"

# put fzf binary on PATH
[[ ":$PATH:" != *":$HOME/.fzf/bin:"* ]] && export PATH="$HOME/.fzf/bin:$PATH"

# shell integration (key bindings + ** completion)
[ -f ~/.fzf/shell/key-bindings.bash ] && source ~/.fzf/shell/key-bindings.bash
[ -f ~/.fzf/shell/completion.bash ]   && source ~/.fzf/shell/completion.bash

_fzf_compgen_path() { fd $_FD_OPTS . "$1"; }
_fzf_compgen_dir()  { fd --type d $_FD_OPTS . "$1"; }
complete -F _fzf_path_completion -o default -o bashdefault nano
complete -F _fzf_dir_completion  -o nospace -o dirnames cd pushd rmdir
