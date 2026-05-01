# auto-generate a commit message via claude, then confirm/edit before committing
commit() {
  git add -A

  local diff
  diff=$(git diff --cached)
  if [ -z "$diff" ]; then
    echo "Nothing to commit."
    return 1
  fi

  echo "Generating commit message..."
  local msg
  msg=$(echo "$diff" | claude -p "Write a concise git commit message for this diff. Output ONLY the message, no quotes, no explanation. First line under 72 chars. If needed, add a blank line then bullet-point details.")
  if [ $? -ne 0 ] || [ -z "$msg" ]; then
    echo "Failed to generate commit message."
    return 1
  fi

  echo ""
  echo "--- Files to be committed ---"
  git diff --cached --name-status
  echo ""
  echo "--- Proposed commit message ---"
  echo "$msg"
  echo "-------------------------------"
  echo ""
  read -rp "(a)ccept, (e)dit, or (c)ancel? " choice

  case "$choice" in
    a|A)
      git commit -m "$msg"
      ;;
    e|E)
      local tmpfile
      tmpfile=$(mktemp)
      echo "$msg" > "$tmpfile"
      ${EDITOR:-vi} "$tmpfile"
      git commit -m "$(cat "$tmpfile")"
      rm -f "$tmpfile"
      ;;
    *)
      echo "Commit cancelled."
      git reset HEAD --quiet
      return 1
      ;;
  esac
}