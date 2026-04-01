# alias for git push (commits first if there are uncommitted changes)
push() {
  if [ -n "$(git status --porcelain)" ]; then
    commit || return 1
  fi
  git push "$@"
}
