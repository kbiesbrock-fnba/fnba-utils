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
        # 3. Resume the Claude session for this cwd+name if one exists.
        # claude keys sessions by UUID (--resume with a non-UUID just opens
        # the picker), so derive a stable v5 UUID from cwd+name and probe for
        # its transcript under ~/.claude/projects to decide resume vs create.
        local session_id project_dir claude_cmd
        session_id=$(uuidgen --sha1 --namespace @dns --name "$PWD/$session_name")
        project_dir="$HOME/.claude/projects/$(printf '%s' "$PWD" | tr -c 'a-zA-Z0-9' '-')"
        if [ -f "$project_dir/$session_id.jsonl" ]; then
            claude_cmd="bash -ic 'claude --resume $session_id'"
        else
            claude_cmd="bash -ic 'claude --session-id $session_id --name \"$session_name\"'"
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
