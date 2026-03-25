# Show available fnba-utils extensions
fnba-help() {
    local dir="$FNBA_UTILS_DIR"
    local file desc name

    echo "fnba-utils — available extensions"
    echo

    for subdir in "$dir"/bashrc.d/*/; do
        local section="${subdir%/}"
        section="${section##*/}"
        printf "  \033[1m%s\033[0m\n" "$section"

        for file in "$subdir"*.sh; do
            [ -f "$file" ] || continue
            name="$(basename "$file" .sh)"
            desc="$(head -1 "$file")"
            desc="${desc#\# }"
            printf "    %-20s %s\n" "$name" "$desc"
        done
        echo
    done

    # Non-shell utilities
    local has_scripts=false
    for item in "$dir"/*/; do
        [[ "$item" == */bashrc.d/ ]] && continue
        [[ "$item" == */.git/ ]] && continue
        [[ "$item" == */.claude/ ]] && continue
        if ! $has_scripts; then
            printf "  \033[1m%s\033[0m\n" "scripts"
            has_scripts=true
        fi
        name="$(basename "$item")"
        desc=""
        # Extract .SYNOPSIS from PowerShell scripts
        for ps1 in "$item"*.ps1; do
            [ -f "$ps1" ] || continue
            desc="$(sed -n '/\.SYNOPSIS/{n;s/^[[:space:]]*//;p;q}' "$ps1")"
            break
        done
        printf "    %-20s %s\n" "$name" "${desc:-utility}"
        done
    $has_scripts && echo
}
