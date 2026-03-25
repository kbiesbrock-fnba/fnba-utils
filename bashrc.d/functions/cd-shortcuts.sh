cdd() { cd "$HOME/dev/$1"; }
cdf() { cd "$HOME/dev/FNBA-Software/$1"; }
cdk() { cd "$HOME/dev/kbiesbrock-fnba/$1"; }

declare -A _cd_dirs=(
  [cdd]="$HOME/dev"
  [cdf]="$HOME/dev/FNBA-Software"
  [cdk]="$HOME/dev/kbiesbrock-fnba"
)

_cd_complete() {
  local base_dir="${_cd_dirs[$1]}"
  local cur="$2"
  local dirs=($(cd "$base_dir" 2>/dev/null && compgen -d -- "$cur"))
  COMPREPLY=("${dirs[@]/%//}")
}

complete -o nospace -F _cd_complete cdd cdf cdk
