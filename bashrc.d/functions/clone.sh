# Clone or update FNBA-Software repos
clone() {
  local repo_name="${1:?Usage: gitc REPO}"
  local base_dir="$HOME/dev/FNBA-Software"
  local repo_dir="$base_dir/$repo_name"

  if [ -d "$repo_dir" ]; then
    cd "$repo_dir" && git checkout master && git fetch && git pull --prune
  else
    cd "$base_dir" && git clone "git@github.com:FNBA-Software/$repo_name.git" && cd "$repo_name"
  fi
}
