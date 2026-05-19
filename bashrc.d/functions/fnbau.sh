# Launch the fnba-utils desktop app, building it first if missing
fnbau() {
    local exe="/mnt/c/dev/fnba-utils/app/src-tauri/target/release/fnba-utils.exe"
    local src_tauri="/mnt/c/dev/fnba-utils/app/src-tauri"
    local cargo_win="/mnt/c/Users/$USER/.cargo/bin/cargo.exe"

    if [[ ! -x "$exe" ]]; then
        if [[ ! -x "$cargo_win" ]]; then
            echo "fnbau: $exe missing and Windows cargo not found at $cargo_win" >&2
            return 1
        fi
        echo "fnbau: binary not found, building (cargo tauri build --no-bundle)..."
        ( cd "$src_tauri" && "$cargo_win" tauri build --no-bundle ) || {
            echo "fnbau: build failed" >&2
            return 1
        }
    fi

    taskkill.exe /F /IM fnba-utils.exe >/dev/null 2>&1

    "$exe" >/dev/null 2>&1 &
    disown

    echo "fnbau: launched. Kill with: taskkill.exe /F /IM fnba-utils.exe"
}
