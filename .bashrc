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
