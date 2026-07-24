# Compare two files (add -a/--advanced for an AI-powered semantic diff)
compare() {
  local advanced=false
  local files=()

  while [ $# -gt 0 ]; do
    case "$1" in
      -a|--advanced)
        advanced=true
        shift
        ;;
      -h|--help)
        echo "Usage: compare [-a|--advanced] FILE1 FILE2"
        echo "  (default)        colorized diff via git (falls back to diff)"
        echo "  -a, --advanced   semantic comparison via claude"
        return 0
        ;;
      -*)
        echo "compare: unknown option '$1'" >&2
        return 1
        ;;
      *)
        files+=("$1")
        shift
        ;;
    esac
  done

  if [ "${#files[@]}" -ne 2 ]; then
    echo "Usage: compare [-a|--advanced] FILE1 FILE2" >&2
    return 1
  fi

  local f1="${files[0]}" f2="${files[1]}"
  local missing=false
  [ -f "$f1" ] || { echo "compare: no such file: $f1" >&2; missing=true; }
  [ -f "$f2" ] || { echo "compare: no such file: $f2" >&2; missing=true; }
  $missing && return 1

  if $advanced; then
    command -v claude >/dev/null 2>&1 || { echo "compare: claude not found on PATH" >&2; return 1; }
    echo "Comparing with claude..."
    claude -p "Compare these two files and summarize the meaningful differences. Focus on what changed semantically (logic, behavior, values, structure), not trivial formatting. Be concise.

--- FILE 1: $f1 ---
$(cat "$f1")

--- FILE 2: $f2 ---
$(cat "$f2")"
    return
  fi

  # Standard diff via git through less: -F prints inline if it fits one screen,
  # -R keeps color, -X leaves the last view in scrollback on exit (:q). Long
  # diffs stay scrollable in the pager. Plain diff fallback when git is absent.
  if command -v git >/dev/null 2>&1; then
    git -c core.pager='less -FRX' diff --no-index --color=auto -- "$f1" "$f2"
  else
    diff -u "$f1" "$f2"
  fi
}
