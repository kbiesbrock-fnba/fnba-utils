#!/usr/bin/env bash
set -euo pipefail

# FNBA Utils - Development launcher
#
# Starts the Vite dev server in WSL, then launches cargo tauri dev on Windows.
# Requires: Rust + Tauri CLI installed on the Windows side.
#
# Usage: bash scripts/dev.sh

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
APP_DIR="$(dirname "$SCRIPT_DIR")"
CARGO_WIN="/mnt/c/Users/$USER/.cargo/bin/cargo.exe"

if [[ ! -f "$CARGO_WIN" ]]; then
  echo "ERROR: Windows cargo not found at $CARGO_WIN"
  echo "Install Rust on Windows: winget install Rustlang.Rustup"
  exit 1
fi

# Set the script dir so the Rust backend can find assumeIdentity-json.ps1
REPO_DIR="$(dirname "$APP_DIR")"
REPO_WIN="$(wslpath -w "$REPO_DIR")"
export FNBA_UTILS_SCRIPT_DIR="$REPO_WIN"

# Start Vite dev server in background
cd "$APP_DIR"
npm run dev &
VITE_PID=$!
trap "kill $VITE_PID 2>/dev/null" EXIT

echo "Waiting for Vite dev server on :5173..."
for i in $(seq 1 30); do
  if curl -s http://localhost:5173 > /dev/null 2>&1; then
    echo "Vite ready."
    break
  fi
  sleep 0.5
done

# Launch Tauri dev via Windows cargo
cd "$APP_DIR/src-tauri"
"$CARGO_WIN" tauri dev

wait $VITE_PID 2>/dev/null || true
