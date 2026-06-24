//! Docker widget commands: container listing, start/stop/restart/logs, pinned
//! container persistence, widget window position, and the background poll
//! thread that emits `docker-status` events.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::Read as _;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tauri::{Emitter, Manager, State};

use crate::state::docker_widget::DockerWidgetState;

// =============================================================================
// Public data model (camelCase for the frontend)
// =============================================================================

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortMapping {
    pub host_ip: Option<String>,
    pub host_port: Option<u16>,
    pub container_port: u16,
    pub protocol: String,
}

#[derive(Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HealthState {
    Healthy,
    Unhealthy,
    Starting,
    None,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerContainer {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub status: String,
    pub health: HealthState,
    pub restart_loop: bool,
    pub ports: Vec<PortMapping>,
    pub compose_project: Option<String>,
    pub compose_service: Option<String>,
    pub pinned: bool,
}

#[derive(Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OverallHealth {
    Green,
    Amber,
    Red,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerStatusPayload {
    pub containers: Vec<DockerContainer>,
    pub running_count: u32,
    pub total_count: u32,
    pub overall: OverallHealth,
    pub engine_up: bool,
    pub error: Option<String>,
}

// =============================================================================
// Intermediate deserialization — docker ps emits PascalCase keys
// =============================================================================

#[derive(Deserialize)]
struct PsLine {
    #[serde(rename = "ID")]
    id: String,
    #[serde(rename = "Names")]
    names: String,
    #[serde(rename = "Image")]
    image: String,
    #[serde(rename = "State")]
    state: String,
    #[serde(rename = "Status")]
    status: String,
    #[serde(rename = "HealthStatus", default)]
    health_status: String,
    #[serde(rename = "Ports", default)]
    ports: String,
    #[serde(rename = "Labels", default)]
    labels: String,
}

// =============================================================================
// docker invocation helper
// =============================================================================

/// Run a docker command with a wall-clock timeout.
///
/// Tries `docker.exe` first; on `NotFound` falls back to
/// `wsl.exe -e docker <args>`. No tokio dependency — safe to call from a plain
/// `std::thread`.
///
/// Returns `(stdout, stderr, success)` on normal exit, or `Err(msg)` on spawn
/// failure / timeout / I/O error.
fn run_docker_blocking(
    args: &[&str],
    timeout: Duration,
) -> Result<(String, String, bool), String> {
    // --- build the command ---
    let (program, full_args): (&str, Vec<&str>) = {
        // We'll try docker.exe; if it's not found we retry with wsl.exe.
        // Build the arg list for the wsl fallback now so we can borrow cleanly.
        (
            "docker.exe",
            args.to_vec(),
        )
    };

    let try_spawn = |prog: &str, cmd_args: &[&str]| -> Result<std::process::Child, std::io::Error> {
        let mut cmd = std::process::Command::new(prog);
        cmd.args(cmd_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Suppress the console window on Windows.
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        cmd.spawn()
    };

    let mut child = match try_spawn(program, &full_args) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // docker.exe not on PATH — try through WSL.
            let mut wsl_args = vec!["-e", "docker"];
            wsl_args.extend_from_slice(args);
            try_spawn("wsl.exe", &wsl_args)
                .map_err(|e2| format!("docker not found (docker.exe: NotFound; wsl.exe: {e2})"))?
        }
        Err(e) => return Err(format!("Failed to spawn docker: {e}")),
    };

    // Drain stdout/stderr on dedicated threads so a child that fills the OS pipe
    // buffer (e.g. `docker logs --tail 200`) can't deadlock against our wait
    // loop. Reading only after exit would block the child on write forever.
    let stdout_handle = child.stdout.take();
    let stderr_handle = child.stderr.take();
    let out_thread = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(mut h) = stdout_handle {
            let _ = h.read_to_string(&mut s);
        }
        s
    });
    let err_thread = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(mut h) = stderr_handle {
            let _ = h.read_to_string(&mut s);
        }
        s
    });

    // Poll for completion with a deadline.
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(Option::None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    // Killing closes the pipes, so the drain threads hit EOF;
                    // join them so they don't leak, then report the timeout.
                    let _ = out_thread.join();
                    let _ = err_thread.join();
                    return Err(format!(
                        "docker command timed out after {}s",
                        timeout.as_secs()
                    ));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(format!("Error waiting for docker process: {e}")),
        }
    };

    // Collect drained output (threads finish once the pipes reach EOF).
    let stdout = out_thread.join().unwrap_or_default();
    let stderr = err_thread.join().unwrap_or_default();

    Ok((stdout, stderr, status.success()))
}

// =============================================================================
// Container listing / status computation
// =============================================================================

/// Query the local Docker engine and return a full status snapshot.
///
/// Called from both the `get_docker_status` Tauri command and the poll thread.
pub fn list_containers(pinned: &HashSet<String>) -> DockerStatusPayload {
    let result = run_docker_blocking(
        &["ps", "-a", "--format", "{{json .}}"],
        Duration::from_secs(5),
    );

    let (stdout, stderr, success) = match result {
        Err(msg) => {
            return DockerStatusPayload {
                containers: vec![],
                running_count: 0,
                total_count: 0,
                overall: OverallHealth::Red,
                engine_up: false,
                error: Some(msg),
            };
        }
        Ok(triple) => triple,
    };

    // Engine-down heuristics: even a success=true exit can carry these on some
    // Docker Desktop versions, so check stderr regardless.
    let engine_down = !success
        || stderr.contains("Cannot connect to the Docker daemon")
        || stderr.contains("error during connect")
        || stderr.contains("The system cannot find the file");

    if engine_down {
        let err_msg = if stderr.trim().is_empty() {
            "Docker engine is not running".to_string()
        } else {
            // Trim to a reasonable single-line summary.
            stderr.lines().next().unwrap_or("Docker error").trim().to_string()
        };
        return DockerStatusPayload {
            containers: vec![],
            running_count: 0,
            total_count: 0,
            overall: OverallHealth::Red,
            engine_up: false,
            error: Some(err_msg),
        };
    }

    // Parse each non-empty line as a JSON object.
    let mut containers: Vec<DockerContainer> = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let ps: PsLine = match serde_json::from_str(line) {
            Ok(p) => p,
            Err(_) => continue, // malformed line — skip it
        };

        let name = ps
            .names
            .split(',')
            .next()
            .unwrap_or(&ps.names)
            .trim()
            .to_string();

        // Health: prefer the explicit HealthStatus field; fall back to scanning
        // the human-readable Status string in case HealthStatus is absent.
        let health = {
            let h = ps.health_status.trim().to_lowercase();
            if h == "healthy" {
                HealthState::Healthy
            } else if h == "unhealthy" {
                HealthState::Unhealthy
            } else if h == "starting" {
                HealthState::Starting
            } else {
                // Fallback: scan the status string.
                let s = ps.status.to_lowercase();
                if s.contains("(unhealthy)") {
                    HealthState::Unhealthy
                } else if s.contains("(healthy)") {
                    HealthState::Healthy
                } else {
                    HealthState::None
                }
            }
        };

        let restart_loop = ps.state == "restarting" || ps.status.starts_with("Restarting");

        let ports = parse_ports(&ps.ports);

        let (compose_project, compose_service) = parse_compose_labels(&ps.labels);

        let is_pinned = pinned.contains(&name);

        containers.push(DockerContainer {
            id: ps.id,
            name,
            image: ps.image,
            state: ps.state,
            status: ps.status,
            health,
            restart_loop,
            ports,
            compose_project,
            compose_service,
            pinned: is_pinned,
        });
    }

    let running_count = containers.iter().filter(|c| c.state == "running").count() as u32;
    let total_count = containers.len() as u32;

    let overall = compute_overall(&containers, running_count);

    DockerStatusPayload {
        containers,
        running_count,
        total_count,
        overall,
        engine_up: true,
        error: None,
    }
}

/// Compute the traffic-light summary.
///
/// Red  — any container is unhealthy or in a restart loop.
/// Amber — there is at least one running container AND at least one
///         exited/dead container (partial outage).
/// Green — everything else (all stopped, all running, empty fleet, etc.).
fn compute_overall(containers: &[DockerContainer], running_count: u32) -> OverallHealth {
    if containers
        .iter()
        .any(|c| c.health == HealthState::Unhealthy || c.restart_loop)
    {
        return OverallHealth::Red;
    }
    let has_dead = containers
        .iter()
        .any(|c| c.state == "exited" || c.state == "dead");
    if running_count > 0 && has_dead {
        return OverallHealth::Amber;
    }
    OverallHealth::Green
}

/// Parse the docker `Ports` field (comma-separated, e.g.
/// `"0.0.0.0:5432->5432/tcp, 127.0.0.1:8080->80/tcp, 9000/tcp"`).
fn parse_ports(raw: &str) -> Vec<PortMapping> {
    let mut out = Vec::new();
    for segment in raw.split(',') {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        if let Some(pm) = parse_port_segment(segment) {
            out.push(pm);
        }
    }
    out
}

fn parse_port_segment(s: &str) -> Option<PortMapping> {
    if let Some(arrow) = s.find("->") {
        // Format: "[hostIp:]hostPort->containerPort/proto"
        let host_part = &s[..arrow];
        let container_part = &s[arrow + 2..];

        let (host_ip, host_port) = parse_host_part(host_part);
        let (container_port, protocol) = parse_container_part(container_part)?;

        Some(PortMapping {
            host_ip,
            host_port,
            container_port,
            protocol,
        })
    } else {
        // Format: "containerPort/proto" or just "containerPort"
        let (container_port, protocol) = parse_container_part(s)?;
        Some(PortMapping {
            host_ip: Option::None,
            host_port: Option::None,
            container_port,
            protocol,
        })
    }
}

/// Parse `[ip:]port` — the host side of a port mapping.
fn parse_host_part(s: &str) -> (Option<String>, Option<u16>) {
    // Could be "0.0.0.0:5432" or just "5432" (rare) or "[::]:5432"
    if let Some(colon) = s.rfind(':') {
        let ip = s[..colon].trim().to_string();
        let port = s[colon + 1..].trim().parse::<u16>().ok();
        let host_ip = if ip.is_empty() {
            Option::None
        } else {
            Some(ip)
        };
        (host_ip, port)
    } else {
        // No colon — treat entire string as the port.
        (Option::None, s.trim().parse::<u16>().ok())
    }
}

/// Parse `containerPort/proto` or just `containerPort`.
fn parse_container_part(s: &str) -> Option<(u16, String)> {
    if let Some(slash) = s.find('/') {
        let port = s[..slash].trim().parse::<u16>().ok()?;
        let proto = s[slash + 1..].trim().to_string();
        Some((port, if proto.is_empty() { "tcp".to_string() } else { proto }))
    } else {
        let port = s.trim().parse::<u16>().ok()?;
        Some((port, "tcp".to_string()))
    }
}

/// Extract `com.docker.compose.project` and `com.docker.compose.service`
/// from the comma-separated `key=value` labels string.
fn parse_compose_labels(labels: &str) -> (Option<String>, Option<String>) {
    let mut project: Option<String> = Option::None;
    let mut service: Option<String> = Option::None;

    for label in labels.split(',') {
        let label = label.trim();
        if let Some(eq) = label.find('=') {
            let key = label[..eq].trim();
            let val = label[eq + 1..].trim();
            if key == "com.docker.compose.project" && !val.is_empty() {
                project = Some(val.to_string());
            } else if key == "com.docker.compose.service" && !val.is_empty() {
                service = Some(val.to_string());
            }
        }
    }

    (project, service)
}

// =============================================================================
// Tauri command handlers
// =============================================================================

/// Query Docker and return a full status snapshot. Heavy work runs off the
/// async executor via `spawn_blocking`.
#[tauri::command]
pub async fn get_docker_status(
    state: State<'_, DockerWidgetState>,
) -> Result<DockerStatusPayload, String> {
    let pinned = state.pinned_set();
    tauri::async_runtime::spawn_blocking(move || list_containers(&pinned))
        .await
        .map_err(|e| format!("spawn_blocking failed: {e}"))
}

/// Start a container by ID.
#[tauri::command]
pub async fn docker_start(id: String) -> Result<(), String> {
    let id_clone = id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let (_, stderr, success) =
            run_docker_blocking(&["start", &id_clone], Duration::from_secs(30))?;
        if success {
            Ok(())
        } else {
            Err(stderr.trim().to_string())
        }
    })
    .await
    .map_err(|e| format!("spawn_blocking failed: {e}"))?
}

/// Stop a container by ID.
#[tauri::command]
pub async fn docker_stop(id: String) -> Result<(), String> {
    let id_clone = id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let (_, stderr, success) =
            run_docker_blocking(&["stop", &id_clone], Duration::from_secs(30))?;
        if success {
            Ok(())
        } else {
            Err(stderr.trim().to_string())
        }
    })
    .await
    .map_err(|e| format!("spawn_blocking failed: {e}"))?
}

/// Restart a container by ID.
#[tauri::command]
pub async fn docker_restart(id: String) -> Result<(), String> {
    let id_clone = id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let (_, stderr, success) =
            run_docker_blocking(&["restart", &id_clone], Duration::from_secs(30))?;
        if success {
            Ok(())
        } else {
            Err(stderr.trim().to_string())
        }
    })
    .await
    .map_err(|e| format!("spawn_blocking failed: {e}"))?
}

/// Fetch the last `tail` lines of logs for a container.
///
/// Docker writes application logs to both stdout and stderr, so we return both
/// concatenated.
#[tauri::command]
pub async fn docker_logs(id: String, tail: u32) -> Result<String, String> {
    let id_clone = id.clone();
    let tail_str = tail.to_string();
    tauri::async_runtime::spawn_blocking(move || {
        let (stdout, stderr, _) = run_docker_blocking(
            &["logs", "--tail", &tail_str, &id_clone],
            Duration::from_secs(5),
        )?;
        let combined = format!("{stdout}\n{stderr}");
        Ok(combined)
    })
    .await
    .map_err(|e| format!("spawn_blocking failed: {e}"))?
}

/// List pinned container names in insertion order.
#[tauri::command]
pub async fn list_pinned_containers(
    state: State<'_, DockerWidgetState>,
) -> Result<Vec<String>, String> {
    Ok(state.pinned_list())
}

/// Add a container name to the pinned list.
#[tauri::command]
pub async fn pin_container(
    name: String,
    state: State<'_, DockerWidgetState>,
) -> Result<(), String> {
    state.pin(name)
}

/// Remove a container name from the pinned list.
#[tauri::command]
pub async fn unpin_container(
    name: String,
    state: State<'_, DockerWidgetState>,
) -> Result<(), String> {
    state.unpin(&name)
}

/// Persist the widget window's current position so it can be restored on the
/// next launch.
#[tauri::command]
pub async fn save_docker_widget_position(
    x: i32,
    y: i32,
    state: State<'_, DockerWidgetState>,
) -> Result<(), String> {
    state.set_position(x, y)
}

/// Return the last persisted widget position, or `null` if none was saved.
#[tauri::command]
pub async fn get_docker_widget_position(
    state: State<'_, DockerWidgetState>,
) -> Result<Option<(i32, i32)>, String> {
    Ok(state.position())
}

/// Bottom edge (physical px) of the PRIMARY monitor's work area — i.e. the top
/// of the taskbar. Used to pin the widget flush above the taskbar regardless of
/// taskbar height or display scaling (a fixed offset guesses wrong under DPI
/// scaling and leaves the widget floating).
#[cfg(windows)]
pub fn work_area_bottom() -> Option<i32> {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPI_GETWORKAREA, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    };
    let mut rect = RECT::default();
    let res = unsafe {
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut rect as *mut RECT as *mut core::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    res.ok().map(|_| rect.bottom)
}

#[cfg(not(windows))]
pub fn work_area_bottom() -> Option<i32> {
    None
}

/// The widget's pinned bottom edge (taskbar top) in physical px. The frontend
/// anchors every resize to this so the widget stays flush above the taskbar.
#[tauri::command]
pub fn docker_widget_anchor_bottom() -> Option<i32> {
    work_area_bottom()
}

// =============================================================================
// Background poll thread
// =============================================================================

/// Spawn a long-lived `std::thread` that polls Docker every few seconds and
/// emits a `docker-status` event with the latest `DockerStatusPayload`.
///
/// Sleep interval adapts to context:
/// - Engine down  → 10 s (avoid hammering a missing daemon)
/// - Widget hidden → 8 s (nobody is watching; conserve CPU)
/// - Normal        → 3 s (responsive ambient display)
///
/// The thread runs for the process lifetime; no cancellation is needed.
pub fn spawn_poll_thread(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        // Emit once immediately so the widget has data before the first sleep.
        loop {
            let visible = app
                .get_webview_window("docker-widget")
                .and_then(|w| w.is_visible().ok())
                .unwrap_or(true);

            let pinned: HashSet<String> = app
                .try_state::<DockerWidgetState>()
                .map(|s| s.pinned_set())
                .unwrap_or_default();

            let payload = list_containers(&pinned);
            let engine_up = payload.engine_up;
            let _ = app.emit("docker-status", &payload);

            let sleep_secs = if !engine_up {
                10
            } else if !visible {
                8
            } else {
                3
            };
            std::thread::sleep(Duration::from_secs(sleep_secs));
        }
    });
}
