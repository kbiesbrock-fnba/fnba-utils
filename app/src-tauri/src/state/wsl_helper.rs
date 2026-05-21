//! Long-lived `wsl.exe bash` subprocess used to amortize the ~300-1000 ms
//! Windows-side WSL cold-start cost across every probe that previously did its
//! own one-shot fork (tmux list, ps args, etc.).
//!
//! Protocol: callers submit a bash script via [`run_script`]; the helper
//! appends a per-call sentinel `echo` and reads stdout until it observes the
//! sentinel line, then returns everything that came before it. The helper is
//! protected by a `Mutex`, so probes serialize — fine because the inner
//! commands (tmux/ps) typically complete in tens of ms once WSL is warm.
//!
//! If the subprocess dies (WSL shutdown, distro restart, broken pipe), the
//! next call respawns it transparently and retries once. A persistent failure
//! falls back to returning an error so callers can degrade gracefully.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

/// One run of `wsl.exe -e bash`. Stdin/stdout are piped; stderr is dropped to
/// keep the protocol parseable (callers redirect 2>&1 inside their script if
/// they want stderr).
struct WslHelper {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl WslHelper {
    fn spawn() -> std::io::Result<Self> {
        let mut child = Command::new("wsl.exe")
            .args(["-e", "bash"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let stdin = child.stdin.take().expect("stdin piped");
        let stdout = BufReader::new(child.stdout.take().expect("stdout piped"));
        let mut helper = Self { child, stdin, stdout };
        // Quiet the shell environment so callers see only their command output.
        // LANG=C makes ps/tmux emit ASCII; PS1/PS2 cleared so any interactive
        // prompt that slipped through can't pollute output.
        helper.write_raw("export LANG=C TERM=dumb PS1='' PS2=''\n")?;
        Ok(helper)
    }

    fn write_raw(&mut self, s: &str) -> std::io::Result<()> {
        self.stdin.write_all(s.as_bytes())?;
        self.stdin.flush()
    }

    fn is_dead(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(Some(_)) | Err(_))
    }

    fn run(&mut self, script: &str, sentinel: &str) -> std::io::Result<String> {
        // The trailing newline before sentinel guarantees the sentinel sits on
        // its own line even if `script` didn't end with one. `2>/dev/null` on
        // the sentinel `echo` is paranoia; bash echo doesn't normally write to
        // stderr.
        let payload = format!("{script}\nprintf '%s\\n' '{sentinel}'\n");
        self.write_raw(&payload)?;

        let mut out = String::new();
        let mut line = String::new();
        loop {
            line.clear();
            let n = self.stdout.read_line(&mut line)?;
            if n == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "wsl helper closed stdout",
                ));
            }
            // BufReader::read_line keeps the trailing \n. Strip it for the
            // sentinel comparison; preserve it for the captured output so
            // callers see line-accurate data.
            let trimmed = line.strip_suffix('\n').unwrap_or(&line);
            let trimmed = trimmed.strip_suffix('\r').unwrap_or(trimmed);
            if trimmed == sentinel {
                return Ok(out);
            }
            out.push_str(&line);
        }
    }
}

impl Drop for WslHelper {
    fn drop(&mut self) {
        // Closing stdin lets bash exit cleanly; if it doesn't, kill it.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn helper_slot() -> &'static Mutex<Option<WslHelper>> {
    static SLOT: OnceLock<Mutex<Option<WslHelper>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

fn next_sentinel() -> String {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    // The marker must not appear naturally in tmux/ps output. A long
    // hyphen-delimited tag is far outside any tmux session name (which can't
    // contain `:` or `\n` and is usually short) or `ps -o args=` output.
    format!("___FNBA_WSL_HELPER_END_{n}___")
}

/// Run `script` inside the persistent helper shell and return its stdout.
/// The script is appended to a shared bash session; assume previously-set
/// shell variables persist, but DO NOT rely on it — each call should be
/// self-contained so a respawn doesn't change behavior.
///
/// Returns `Err` only when both the initial attempt and a respawn-retry fail.
pub fn run_script(script: &str) -> Result<String, String> {
    let mut guard = helper_slot().lock().map_err(|e| e.to_string())?;

    // Drop a dead helper before attempting anything.
    if let Some(h) = guard.as_mut() {
        if h.is_dead() {
            *guard = None;
        }
    }
    if guard.is_none() {
        match WslHelper::spawn() {
            Ok(h) => *guard = Some(h),
            Err(e) => return Err(format!("spawn wsl helper failed: {e}")),
        }
    }

    let sentinel = next_sentinel();
    let helper = guard.as_mut().expect("just spawned");
    match helper.run(script, &sentinel) {
        Ok(out) => Ok(out),
        Err(first_err) => {
            // Probable causes: bash exited (WSL shutdown), broken pipe, or
            // stdout EOF. Respawn once and retry — masks transient distro
            // restarts without escalating.
            *guard = None;
            match WslHelper::spawn() {
                Ok(h) => {
                    *guard = Some(h);
                    let sentinel2 = next_sentinel();
                    guard
                        .as_mut()
                        .expect("just spawned")
                        .run(script, &sentinel2)
                        .map_err(|e| {
                            format!("wsl helper retry failed (first: {first_err}; retry: {e})")
                        })
                }
                Err(e) => Err(format!(
                    "wsl helper respawn failed after error '{first_err}': {e}"
                )),
            }
        }
    }
}
