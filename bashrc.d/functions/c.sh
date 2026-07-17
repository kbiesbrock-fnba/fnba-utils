c() {
    local session_name
    session_name=$(basename "$PWD")

    # 1. Fetch git branch cleanly
    if git rev-parse --is-inside-work-tree &>/dev/null; then
        session_name="$session_name-$(git branch --show-current)"
    fi

    # 2. Check if session already exists
    if tmux has-session -t "$session_name" 2>/dev/null; then
        exec tmux attach-session -t "$session_name"
    else
        # 3. Create session detached
        tmux new-session -d -s "$session_name" -n "claude-env"

        # 4. Split using the modern -l flag with a explicit % sign
        tmux split-window -h -l 35% -t "$session_name:"

        # 5. Send command directly to the left pane (pane index 0)
        tmux send-keys -t "$session_name:.0" "bash -ic claude" C-m

        # 6. Attach safely with exec now that commands are built correctly
        exec tmux attach-session -t "$session_name"
    fi
}
