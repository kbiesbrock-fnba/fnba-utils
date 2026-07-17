c() {
    local session_name
    session_name=$(basename "$PWD")

    # 1. Fetch git branch cleanly
    if git rev-parse --is-inside-work-tree &>/dev/null; then
        session_name="$session_name-$(git branch --show-current)"
    fi

    # 2. Check if the tmux session already exists
    if tmux has-session -t "$session_name" 2>/dev/null; then
        exec tmux attach-session -t "$session_name"
    else
        # 3. Determine if a Claude session with this name already exists
        local claude_cmd
        if claude --list 2>/dev/null | grep -qF "$session_name"; then
            # If it exists, resume it
            claude_cmd="bash -ic 'claude --resume \"$session_name\"'"
        else
            # If it is new, create it with this specific name
            claude_cmd="bash -ic 'claude --name \"$session_name\"'"
        fi

        # 4. Create the tmux session detached
        tmux new-session -d -s "$session_name" -n "$session_name"

        # 5. Split the pane horizontally using modern layout syntax
        tmux split-window -h -l 35% -t "$session_name:"

        # 6. FORCE FOCUS back to the left pane (pane index 0)
        tmux select-pane -t "$session_name:.0"

        # 7. Inject the Claude command into the focused left pane
        tmux send-keys -t "$session_name:.0" "$claude_cmd" C-m

        # 8. Attach to your freshly organized layout
        tmux attach-session -t "$session_name"
    fi
}
