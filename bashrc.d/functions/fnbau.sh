# Launch the fnba-utils desktop app, building it first if missing.
# The build step shells out to PowerShell because `cargo.exe` invoked
# directly from WSL inherits the WSL env -- no Windows Rust toolchain
# paths, no MSVC linker, no Tauri tooling -- and silently fails. A
# PowerShell child process gets the proper Windows env for free.
fnbau() {
    local exe="/mnt/c/dev/fnba-utils/app/src-tauri/target/release/fnba-utils.exe"
    local src_tauri="/mnt/c/dev/fnba-utils/app/src-tauri"

    case "${1:-}" in
        "")
            ;;
        stop|kill)
            if tasklist.exe /FI "IMAGENAME eq fnba-utils.exe" 2>/dev/null | grep -q fnba-utils.exe; then
                taskkill.exe /F /IM fnba-utils.exe >/dev/null 2>&1
                echo "fnbau: stopped."
            else
                echo "fnbau: not running."
            fi
            return 0
            ;;
        *)
            echo "fnbau: unknown argument '$1' (expected: stop|kill, or no args to launch)" >&2
            return 2
            ;;
    esac

    if [[ ! -x "$exe" ]]; then
        local win_src_tauri
        win_src_tauri=$(wslpath -w "$src_tauri") || {
            echo "fnbau: wslpath failed to translate $src_tauri" >&2
            return 1
        }
        echo "fnbau: binary not found, building via PowerShell (cargo tauri build --no-bundle)..."
        powershell.exe -NoProfile -Command "Set-Location '$win_src_tauri'; cargo tauri build --no-bundle; exit \$LASTEXITCODE" || {
            echo "fnbau: build failed" >&2
            return 1
        }
    fi

    taskkill.exe /F /IM fnba-utils.exe >/dev/null 2>&1

    "$exe" >/dev/null 2>&1 &
    disown

    echo "fnbau: launched"
}
