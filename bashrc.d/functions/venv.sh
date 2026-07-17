# Auto-activate Python venv on cd
cd() {
    builtin cd "$@" || return

    # Check if we are inside a virtual environment
    if [[ -n "$VIRTUAL_ENV" ]]; then
        # Deactivate if the current directory no longer contains this venv
        if [[ "$PWD" != "$(dirname "$VIRTUAL_ENV")"* ]]; then
            deactivate
        fi
    fi

    # Activate venv if a common directory name exists (.venv or venv)
    if [[ -f ".venv/bin/activate" ]]; then
        source .venv/bin/activate
    elif [[ -f "venv/bin/activate" ]]; then
        source venv/bin/activate
    fi
}
