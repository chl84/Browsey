use super::rclone_cli::{RcloneCliError, RcloneSubcommand};
use regex::Regex;
use serde_json::{json, Value};
use std::{
    env,
    ffi::OsString,
    fs, io,
    io::{Read, Write},
    net::Shutdown,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
use tracing::{debug, info};
use wait_timeout::ChildExt;

const RCLONE_RC_ENABLE_ENV: &str = "BROWSEY_RCLONE_RC";
const RCLONE_RC_STATE_DIR_NAME: &str = "browsey-rclone-rc";
const RCLONE_RC_STARTUP_TIMEOUT: Duration = Duration::from_secs(4);
const RCLONE_RC_REQUEST_TIMEOUT: Duration = Duration::from_secs(45);
const RCLONE_RC_NOOP_TIMEOUT: Duration = Duration::from_secs(2);
const RCLONE_RC_STARTUP_POLL_SLICE: Duration = Duration::from_millis(80);
const RCLONE_RC_ERROR_TEXT_MAX_CHARS: usize = 2048;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RcloneRcMethod {
    CoreNoop,
    ConfigListRemotes,
    ConfigDump,
    OperationsList,
    OperationsStat,
}

impl RcloneRcMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CoreNoop => "core/noop",
            Self::ConfigListRemotes => "config/listremotes",
            Self::ConfigDump => "config/dump",
            Self::OperationsList => "operations/list",
            Self::OperationsStat => "operations/stat",
        }
    }
}

#[derive(Debug)]
struct RcloneRcDaemon {
    socket_path: PathBuf,
    child: Child,
}

#[derive(Debug, Default)]
struct RcloneRcState {
    daemon: Option<RcloneRcDaemon>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RcloneRcHealth {
    pub enabled: bool,
    pub daemon_running: bool,
    pub socket_path: Option<String>,
    pub socket_exists: bool,
}

#[derive(Debug, Clone)]
pub struct RcloneRcClient {
    binary: OsString,
}

impl Default for RcloneRcClient {
    fn default() -> Self {
        Self {
            binary: OsString::from("rclone"),
        }
    }
}

impl RcloneRcClient {
    pub fn new(binary: impl Into<OsString>) -> Self {
        Self {
            binary: binary.into(),
        }
    }

    pub fn is_enabled(&self) -> bool {
        rc_enabled()
    }

    pub fn list_remotes(&self) -> Result<Value, RcloneCliError> {
        self.run_method(RcloneRcMethod::ConfigListRemotes, json!({}))
    }

    pub fn config_dump(&self) -> Result<Value, RcloneCliError> {
        self.run_method(RcloneRcMethod::ConfigDump, json!({}))
    }

    pub fn operations_list(
        &self,
        fs_spec: &str,
        remote_path: &str,
    ) -> Result<Value, RcloneCliError> {
        self.run_method(
            RcloneRcMethod::OperationsList,
            json!({
                "fs": fs_spec,
                "remote": remote_path,
            }),
        )
    }

    pub fn operations_stat(
        &self,
        fs_spec: &str,
        remote_path: &str,
    ) -> Result<Value, RcloneCliError> {
        self.run_method(
            RcloneRcMethod::OperationsStat,
            json!({
                "fs": fs_spec,
                "remote": remote_path,
            }),
        )
    }

    fn run_method(&self, method: RcloneRcMethod, payload: Value) -> Result<Value, RcloneCliError> {
        let socket_path = self.ensure_daemon_ready()?;
        let started = Instant::now();
        let result =
            run_rc_command_via_socket(&socket_path, method, payload, RCLONE_RC_REQUEST_TIMEOUT);
        debug!(
            method = method.as_str(),
            elapsed_ms = started.elapsed().as_millis() as u64,
            success = result.is_ok(),
            "rclone rc method completed"
        );
        result
    }

    fn ensure_daemon_ready(&self) -> Result<PathBuf, RcloneCliError> {
        if !self.is_enabled() {
            return Err(RcloneCliError::Io(io::Error::new(
                io::ErrorKind::Unsupported,
                "rclone rc backend is disabled",
            )));
        }

        let mut state = match rclone_rc_state().lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        if let Some(existing) = state.daemon.as_mut() {
            if daemon_is_running(existing)? && existing.socket_path.exists() {
                return Ok(existing.socket_path.clone());
            }
            info!(
                socket = %existing.socket_path.display(),
                "restarting stale rclone rcd daemon"
            );
            let _ = kill_daemon(existing);
            state.daemon = None;
        }

        let daemon = spawn_daemon(&self.binary)?;
        let socket_path = daemon.socket_path.clone();
        info!(socket = %socket_path.display(), "started rclone rcd daemon");
        state.daemon = Some(daemon);
        Ok(socket_path)
    }
}

pub fn begin_shutdown_and_kill_daemon() -> io::Result<()> {
    let mut state = match rclone_rc_state().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(mut daemon) = state.daemon.take() {
        let socket_display = daemon.socket_path.display().to_string();
        kill_daemon(&mut daemon)?;
        info!(socket = %socket_display, "stopped rclone rcd daemon");
    }
    Ok(())
}

pub fn health_snapshot() -> RcloneRcHealth {
    let enabled = rc_enabled();
    let mut daemon_running = false;
    let mut socket_path = None;
    let mut socket_exists = false;

    if let Ok(mut state) = rclone_rc_state().lock() {
        if let Some(daemon) = state.daemon.as_mut() {
            socket_path = Some(daemon.socket_path.display().to_string());
            socket_exists = daemon.socket_path.exists();
            daemon_running = daemon
                .child
                .try_wait()
                .ok()
                .and_then(|status| status)
                .is_none();
        }
    }

    RcloneRcHealth {
        enabled,
        daemon_running,
        socket_path,
        socket_exists,
    }
}

fn rc_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        if !cfg!(target_os = "linux") {
            return false;
        }
        matches!(
            env::var(RCLONE_RC_ENABLE_ENV)
                .ok()
                .map(|v| v.trim().to_ascii_lowercase()),
            Some(ref v) if v == "1" || v == "true" || v == "yes" || v == "on"
        )
    })
}

fn rclone_rc_state() -> &'static Mutex<RcloneRcState> {
    static STATE: OnceLock<Mutex<RcloneRcState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(RcloneRcState::default()))
}

fn spawn_daemon(binary: &OsString) -> Result<RcloneRcDaemon, RcloneCliError> {
    let startup_started = Instant::now();
    let state_dir = rc_state_dir_path()?;
    prepare_state_dir(&state_dir).map_err(RcloneCliError::Io)?;
    let socket_path = state_dir.join(format!("rcd-{}.sock", std::process::id()));
    if socket_path.exists() {
        let _ = fs::remove_file(&socket_path);
    }

    let mut child = Command::new(binary)
        .arg("rcd")
        .arg("--rc")
        .arg("--rc-addr")
        .arg(format!("unix://{}", socket_path.display()))
        .arg("--rc-server-read-timeout")
        .arg("5m")
        .arg("--rc-server-write-timeout")
        .arg("5m")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(RcloneCliError::Io)?;

    let ready_deadline = Instant::now() + RCLONE_RC_STARTUP_TIMEOUT;
    loop {
        if let Some(status) = child.try_wait().map_err(RcloneCliError::Io)? {
            return Err(RcloneCliError::NonZero {
                status,
                stdout: String::new(),
                stderr: "rclone rcd exited during startup".to_string(),
            });
        }
        if socket_path.exists() {
            match run_rc_command_via_socket(
                &socket_path,
                RcloneRcMethod::CoreNoop,
                json!({}),
                RCLONE_RC_NOOP_TIMEOUT,
            ) {
                Ok(_) => {
                    info!(
                        socket = %socket_path.display(),
                        startup_ms = startup_started.elapsed().as_millis() as u64,
                        "rclone rcd daemon is ready"
                    );
                    break;
                }
                Err(error) => {
                    debug!(error = %error, "rclone rcd startup probe not ready yet");
                }
            }
        }
        if Instant::now() >= ready_deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(RcloneCliError::Timeout {
                subcommand: RcloneSubcommand::Rc,
                timeout: RCLONE_RC_STARTUP_TIMEOUT,
                stdout: String::new(),
                stderr: "timed out waiting for rclone rcd startup".to_string(),
            });
        }
        std::thread::sleep(RCLONE_RC_STARTUP_POLL_SLICE);
    }

    Ok(RcloneRcDaemon { socket_path, child })
}

fn daemon_is_running(daemon: &mut RcloneRcDaemon) -> Result<bool, RcloneCliError> {
    match daemon.child.try_wait().map_err(RcloneCliError::Io)? {
        Some(status) => {
            debug!(status = %status, "rclone rcd daemon has exited");
            Ok(false)
        }
        None => Ok(true),
    }
}

fn kill_daemon(daemon: &mut RcloneRcDaemon) -> io::Result<()> {
    let _ = daemon.child.kill();
    let _ = daemon.child.wait_timeout(Duration::from_secs(1));
    let _ = daemon.child.wait();
    if daemon.socket_path.exists() {
        let _ = fs::remove_file(&daemon.socket_path);
    }
    Ok(())
}

fn rc_state_dir_path() -> Result<PathBuf, RcloneCliError> {
    if let Some(xdg_runtime_dir) = env::var_os("XDG_RUNTIME_DIR") {
        return Ok(PathBuf::from(xdg_runtime_dir).join(RCLONE_RC_STATE_DIR_NAME));
    }
    Err(RcloneCliError::Io(io::Error::new(
        io::ErrorKind::NotFound,
        "XDG_RUNTIME_DIR is not set; cannot initialize secure rclone rc socket path",
    )))
}

fn prepare_state_dir(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn run_rc_command_via_socket(
    socket_path: &Path,
    method: RcloneRcMethod,
    payload: Value,
    timeout: Duration,
) -> Result<Value, RcloneCliError> {
    if !cfg!(unix) {
        return Err(RcloneCliError::Io(io::Error::new(
            io::ErrorKind::Unsupported,
            "rclone rc unix socket transport is only supported on unix targets",
        )));
    }
    let payload_text = serde_json::to_string(&payload).map_err(|error| {
        RcloneCliError::Io(io::Error::other(format!(
            "failed to encode rclone rc payload: {error}"
        )))
    })?;

    let response_text =
        send_rc_http_request_over_unix_socket(socket_path, method, &payload_text, timeout)?;
    let body = parse_http_response_body(&response_text, method)?;
    if body.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str::<Value>(body).map_err(|error| {
        RcloneCliError::Io(io::Error::other(format!(
            "invalid JSON from rclone rc {}: {error}",
            method.as_str()
        )))
    })
}

#[cfg(unix)]
fn send_rc_http_request_over_unix_socket(
    socket_path: &Path,
    method: RcloneRcMethod,
    payload_text: &str,
    timeout: Duration,
) -> Result<String, RcloneCliError> {
    use std::os::unix::net::UnixStream;

    let mut stream = UnixStream::connect(socket_path)
        .map_err(|error| map_rc_io_error(error, timeout, "connect"))?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(RcloneCliError::Io)?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(RcloneCliError::Io)?;

    let request = format!(
        "POST /{} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        method.as_str(),
        payload_text.len(),
        payload_text
    );

    stream
        .write_all(request.as_bytes())
        .map_err(|error| map_rc_io_error(error, timeout, "write"))?;
    stream
        .shutdown(Shutdown::Write)
        .map_err(RcloneCliError::Io)?;

    let mut response_bytes = Vec::new();
    stream
        .read_to_end(&mut response_bytes)
        .map_err(|error| map_rc_io_error(error, timeout, "read"))?;
    Ok(String::from_utf8_lossy(&response_bytes).to_string())
}

#[cfg(not(unix))]
fn send_rc_http_request_over_unix_socket(
    _socket_path: &Path,
    _method: RcloneRcMethod,
    _payload_text: &str,
    _timeout: Duration,
) -> Result<String, RcloneCliError> {
    Err(RcloneCliError::Io(io::Error::new(
        io::ErrorKind::Unsupported,
        "rclone rc unix socket transport is only supported on unix targets",
    )))
}

fn map_rc_io_error(error: io::Error, timeout: Duration, phase: &str) -> RcloneCliError {
    if error.kind() == io::ErrorKind::TimedOut || error.kind() == io::ErrorKind::WouldBlock {
        RcloneCliError::Timeout {
            subcommand: RcloneSubcommand::Rc,
            timeout,
            stdout: String::new(),
            stderr: format!("rclone rc socket {phase} timed out"),
        }
    } else {
        RcloneCliError::Io(error)
    }
}

fn parse_http_response_body<'a>(
    response_text: &'a str,
    method: RcloneRcMethod,
) -> Result<&'a str, RcloneCliError> {
    let Some((header, body)) = response_text.split_once("\r\n\r\n") else {
        return Err(RcloneCliError::Io(io::Error::other(format!(
            "invalid HTTP response from rclone rc {}",
            method.as_str()
        ))));
    };

    let mut lines = header.lines();
    let status_line = lines.next().unwrap_or_default();
    let status_code = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|n| n.parse::<u16>().ok())
        .unwrap_or(0);
    if !(200..300).contains(&status_code) {
        let body_scrubbed = scrub_rc_error_text(body.trim());
        return Err(RcloneCliError::Io(io::Error::other(format!(
            "rclone rc {} failed with HTTP {} ({})",
            method.as_str(),
            status_code,
            body_scrubbed
        ))));
    }
    Ok(body)
}

fn scrub_rc_error_text(raw: &str) -> String {
    let mut out = raw.to_string();
    for re in sensitive_json_regexes() {
        out = re.replace_all(&out, "$1\"***\"").to_string();
    }
    if out.chars().count() > RCLONE_RC_ERROR_TEXT_MAX_CHARS {
        out = out
            .chars()
            .take(RCLONE_RC_ERROR_TEXT_MAX_CHARS)
            .collect::<String>();
        out.push_str("… [truncated]");
    }
    out
}

fn sensitive_json_regexes() -> &'static Vec<Regex> {
    static REGEXES: OnceLock<Vec<Regex>> = OnceLock::new();
    REGEXES.get_or_init(|| {
        [
            r#"("access_token"\s*:\s*)"[^"]*""#,
            r#"("refresh_token"\s*:\s*)"[^"]*""#,
            r#"("token"\s*:\s*)"[^"]*""#,
            r#"("pass(word)?"\s*:\s*)"[^"]*""#,
        ]
        .iter()
        .map(|pattern| Regex::new(pattern).expect("valid sensitive JSON regex"))
        .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::{parse_http_response_body, scrub_rc_error_text, RcloneRcMethod};

    #[test]
    fn rc_method_allowlist_maps_to_expected_endpoint_names() {
        assert_eq!(RcloneRcMethod::CoreNoop.as_str(), "core/noop");
        assert_eq!(
            RcloneRcMethod::ConfigListRemotes.as_str(),
            "config/listremotes"
        );
        assert_eq!(RcloneRcMethod::ConfigDump.as_str(), "config/dump");
        assert_eq!(RcloneRcMethod::OperationsList.as_str(), "operations/list");
        assert_eq!(RcloneRcMethod::OperationsStat.as_str(), "operations/stat");
    }

    #[test]
    fn parses_success_http_response_body() {
        let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"ok\":true}";
        let body = parse_http_response_body(response, RcloneRcMethod::CoreNoop).expect("body");
        assert_eq!(body, "{\"ok\":true}");
    }

    #[test]
    fn rejects_non_2xx_http_response() {
        let response = "HTTP/1.1 500 Internal Server Error\r\nContent-Type: application/json\r\n\r\n{\"error\":\"boom\",\"access_token\":\"abc\",\"token\":\"def\"}";
        let err = parse_http_response_body(response, RcloneRcMethod::CoreNoop).expect_err("err");
        assert!(
            err.to_string().contains("HTTP 500"),
            "unexpected error text: {err}"
        );
        assert!(
            !err.to_string().contains("\"abc\"") && !err.to_string().contains("\"def\""),
            "sensitive values should be redacted: {err}"
        );
    }

    #[test]
    fn scrubs_and_truncates_sensitive_rc_error_text() {
        let raw = format!(
            "{{\"access_token\":\"a\",\"refresh_token\":\"b\",\"pass\":\"c\",\"token\":\"d\",\"pad\":\"{}\"}}",
            "x".repeat(3000)
        );
        let scrubbed = scrub_rc_error_text(&raw);
        assert!(scrubbed.contains("\"access_token\":\"***\""));
        assert!(scrubbed.contains("\"refresh_token\":\"***\""));
        assert!(scrubbed.contains("\"pass\":\"***\""));
        assert!(scrubbed.contains("\"token\":\"***\""));
        assert!(scrubbed.contains("[truncated]"));
    }
}
