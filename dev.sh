#!/usr/bin/env bash
set -euo pipefail

# fnba-utils dev launcher.
#
# Manages a backgrounded `pwsh.exe -c "npm i; npm run tauri dev"` from WSL with
# start / stop / restart subcommands. The Windows PID is recorded by the
# pwsh.exe process itself so we can `taskkill /F /T` the whole tree — a plain
# WSL `kill` only severs the WSL-side wrapper and leaves cargo / tauri / node
# children running on the Windows side.
#
# Usage:
#   bash dev.sh start    # spawn (no-op if already running)
#   bash dev.sh stop     # taskkill /F /T the windows process tree
#   bash dev.sh kill     # alias for stop
#   bash dev.sh restart  # stop then start
#   bash dev.sh status   # show pid / running state
#   bash dev.sh logs     # tail the redirected stdout/stderr

ROOT="$(cd "$(dirname "$0")" && pwd)"
APP_DIR="$ROOT/app"
STATE_DIR="$HOME/.cache/fnba-utils-dev"
WIN_PID_FILE="$STATE_DIR/win.pid"
LOG_FILE="$STATE_DIR/dev.log"

mkdir -p "$STATE_DIR"

# pwsh.exe needs Windows-style paths when writing files.
win_path() {
  wslpath -w "$1"
}

is_running() {
  [[ -s "$WIN_PID_FILE" ]] || return 1
  local pid
  pid=$(cat "$WIN_PID_FILE")
  [[ -n "$pid" ]] || return 1
  # tasklist always exits 0; grep the output for the matching image to decide.
  tasklist.exe /FI "PID eq $pid" 2>/dev/null \
    | tr -d '\r' \
    | grep -qi '^pwsh\.exe'
}

cmd_start() {
  if is_running; then
    echo "Already running (windows pid $(cat "$WIN_PID_FILE")). Use 'restart' or 'stop'."
    return 0
  fi

  # Stale pid file from a previous run (process exited on its own). Clear it
  # so a future taskkill can't accidentally target a recycled PID.
  rm -f "$WIN_PID_FILE"

  local pidf_win
  pidf_win=$(win_path "$WIN_PID_FILE")

  echo "Starting 'npm i; npm run tauri dev' via pwsh.exe..."
  echo "Logs: $LOG_FILE"

  cd "$APP_DIR"
  # The pwsh script: record $PID first (so 'stop' works even if npm i fails),
  # then run npm install and the tauri dev loop. $LASTEXITCODE gate skips dev
  # when install fails so the log surfaces the real error.
  pwsh.exe -NoProfile -NoLogo -Command "
    Set-Content -LiteralPath '$pidf_win' -Value \$PID
    npm i
    if (\$LASTEXITCODE -eq 0) { npm run tauri dev }
  " > "$LOG_FILE" 2>&1 &
  disown $! 2>/dev/null || true

  # Wait briefly for pwsh to write its Windows PID. 2s is plenty; if it never
  # arrives, pwsh.exe failed to launch and the log will tell us why.
  local i
  for i in $(seq 1 10); do
    [[ -s "$WIN_PID_FILE" ]] && break
    sleep 0.2
  done

  if is_running; then
    echo "Started (windows pid $(cat "$WIN_PID_FILE")). Tail logs: bash dev.sh logs"
  else
    echo "WARN: pwsh.exe didn't report a PID. It probably failed early; check $LOG_FILE"
    return 1
  fi
}

cmd_stop() {
  if ! is_running; then
    echo "Not running."
    rm -f "$WIN_PID_FILE"
    return 0
  fi
  local pid
  pid=$(cat "$WIN_PID_FILE")
  echo "Stopping windows pid $pid and child tree..."
  # /T = tree (children, grandchildren), /F = force. Catches pwsh.exe + npm +
  # node + cargo + tauri + rustc + the running app window.
  taskkill.exe /F /T /PID "$pid" >/dev/null 2>&1 || true
  rm -f "$WIN_PID_FILE"
  echo "Stopped."
}

cmd_restart() {
  cmd_stop
  cmd_start
}

cmd_status() {
  if is_running; then
    echo "Running. Windows pid: $(cat "$WIN_PID_FILE")"
  else
    echo "Not running."
  fi
}

cmd_logs() {
  if [[ ! -f "$LOG_FILE" ]]; then
    echo "No log file yet: $LOG_FILE"
    return 0
  fi
  tail -F "$LOG_FILE"
}

usage() {
  cat <<EOF
Usage: bash dev.sh <command>

  start    Spawn 'pwsh.exe npm i; npm run tauri dev' in the background.
  stop     Kill the running tauri dev process tree (taskkill /F /T).
  kill     Alias for stop.
  restart  Stop then start.
  status   Show pid / running state.
  logs     Tail the redirected stdout/stderr (Ctrl-C to exit).
EOF
}

case "${1:-}" in
  start)   cmd_start ;;
  stop)    cmd_stop ;;
  kill)    cmd_stop ;;
  restart) cmd_restart ;;
  status)  cmd_status ;;
  logs)    cmd_logs ;;
  -h|--help|help|"") usage; [[ -z "${1:-}" ]] && exit 2 || exit 0 ;;
  *) echo "Unknown command: $1"; echo; usage; exit 2 ;;
esac
