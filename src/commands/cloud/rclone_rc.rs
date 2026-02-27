use super::rclone_cli::{RcloneCliError, RcloneSubcommand};
use regex::Regex;
use serde_json::{json, Value};
use std::{
    env,
    ffi::OsString,
    fs, io,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{atomic::AtomicBool, Mutex, OnceLock},
    time::{Duration, Instant},
};
use tracing::{debug, info, warn};
use wait_timeout::ChildExt;

const RCLONE_RC_ENABLE_ENV: &str = "BROWSEY_RCLONE_RC";
const RCLONE_RC_READ_ENABLE_ENV: &str = "BROWSEY_RCLONE_RC_READ";
const RCLONE_RC_WRITE_ENABLE_ENV: &str = "BROWSEY_RCLONE_RC_WRITE";
const RCLONE_RC_STATE_DIR_NAME: &str = "browsey-rclone-rc";
const RCLONE_RC_STARTUP_TIMEOUT: Duration = Duration::from_secs(4);
const RCLONE_RC_READ_TIMEOUT: Duration = Duration::from_secs(25);
const RCLONE_RC_WRITE_TIMEOUT: Duration = Duration::from_secs(120);
const RCLONE_RC_MAX_RETRIES: usize = 1;
const RCLONE_RC_RETRY_BACKOFF: Duration = Duration::from_millis(120);
const RCLONE_RC_NOOP_TIMEOUT: Duration = Duration::from_secs(2);
const RCLONE_RC_STARTUP_POLL_SLICE: Duration = Duration::from_millis(80);
const RCLONE_RC_ASYNC_POLL_SLICE: Duration = Duration::from_millis(120);
#[cfg(not(test))]
const RCLONE_RC_START_FAILURE_COOLDOWN: Duration = Duration::from_secs(30);
#[cfg(test)]
const RCLONE_RC_START_FAILURE_COOLDOWN: Duration = Duration::from_millis(1);
const RCLONE_RC_ERROR_TEXT_MAX_CHARS: usize = 2048;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RcloneRcMethod {
    CoreNoop,
    ConfigListRemotes,
    ConfigDump,
    OperationsList,
    OperationsStat,
    OperationsMkdir,
    OperationsDeleteFile,
    OperationsPurge,
    OperationsRmdir,
    OperationsCopyFile,
    OperationsMoveFile,
    JobStatus,
    JobStop,
}

impl RcloneRcMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CoreNoop => "rc/noop",
            Self::ConfigListRemotes => "config/listremotes",
            Self::ConfigDump => "config/dump",
            Self::OperationsList => "operations/list",
            Self::OperationsStat => "operations/stat",
            Self::OperationsMkdir => "operations/mkdir",
            Self::OperationsDeleteFile => "operations/deletefile",
            Self::OperationsPurge => "operations/purge",
            Self::OperationsRmdir => "operations/rmdir",
            Self::OperationsCopyFile => "operations/copyfile",
            Self::OperationsMoveFile => "operations/movefile",
            Self::JobStatus => "job/status",
            Self::JobStop => "job/stop",
        }
    }
}

fn method_timeout(method: RcloneRcMethod) -> Duration {
    match method {
        RcloneRcMethod::CoreNoop
        | RcloneRcMethod::ConfigListRemotes
        | RcloneRcMethod::ConfigDump
        | RcloneRcMethod::OperationsList
        | RcloneRcMethod::OperationsStat
        | RcloneRcMethod::JobStatus => RCLONE_RC_READ_TIMEOUT,
        RcloneRcMethod::OperationsMkdir
        | RcloneRcMethod::OperationsDeleteFile
        | RcloneRcMethod::OperationsPurge
        | RcloneRcMethod::OperationsRmdir
        | RcloneRcMethod::OperationsCopyFile
        | RcloneRcMethod::OperationsMoveFile
        | RcloneRcMethod::JobStop => RCLONE_RC_WRITE_TIMEOUT,
    }
}

fn method_is_retry_safe(method: RcloneRcMethod) -> bool {
    matches!(
        method,
        RcloneRcMethod::CoreNoop
            | RcloneRcMethod::ConfigListRemotes
            | RcloneRcMethod::ConfigDump
            | RcloneRcMethod::OperationsList
            | RcloneRcMethod::OperationsStat
            | RcloneRcMethod::JobStatus
            | RcloneRcMethod::JobStop
    )
}

fn async_method_total_timeout(method: RcloneRcMethod) -> Duration {
    match method {
        RcloneRcMethod::OperationsCopyFile | RcloneRcMethod::OperationsMoveFile => {
            Duration::from_secs(300)
        }
        RcloneRcMethod::OperationsPurge => Duration::from_secs(300),
        RcloneRcMethod::OperationsDeleteFile | RcloneRcMethod::OperationsRmdir => {
            Duration::from_secs(120)
        }
        RcloneRcMethod::OperationsMkdir => Duration::from_secs(45),
        _ => RCLONE_RC_WRITE_TIMEOUT,
    }
}

fn is_retryable_rc_error(error: &RcloneCliError) -> bool {
    match error {
        RcloneCliError::Timeout { .. } => true,
        RcloneCliError::Io(io) => matches!(
            io.kind(),
            io::ErrorKind::TimedOut
                | io::ErrorKind::WouldBlock
                | io::ErrorKind::ConnectionReset
                | io::ErrorKind::ConnectionAborted
                | io::ErrorKind::Interrupted
                | io::ErrorKind::BrokenPipe
                | io::ErrorKind::NotConnected
        ),
        RcloneCliError::Shutdown { .. }
        | RcloneCliError::Cancelled { .. }
        | RcloneCliError::NonZero { .. } => false,
    }
}

fn allowlisted_method_from_name(method_name: &str) -> Option<RcloneRcMethod> {
    match method_name {
        "rc/noop" => Some(RcloneRcMethod::CoreNoop),
        "config/listremotes" => Some(RcloneRcMethod::ConfigListRemotes),
        "config/dump" => Some(RcloneRcMethod::ConfigDump),
        "operations/list" => Some(RcloneRcMethod::OperationsList),
        "operations/stat" => Some(RcloneRcMethod::OperationsStat),
        "operations/mkdir" => Some(RcloneRcMethod::OperationsMkdir),
        "operations/deletefile" => Some(RcloneRcMethod::OperationsDeleteFile),
        "operations/purge" => Some(RcloneRcMethod::OperationsPurge),
        "operations/rmdir" => Some(RcloneRcMethod::OperationsRmdir),
        "operations/copyfile" => Some(RcloneRcMethod::OperationsCopyFile),
        "operations/movefile" => Some(RcloneRcMethod::OperationsMoveFile),
        "job/status" => Some(RcloneRcMethod::JobStatus),
        "job/stop" => Some(RcloneRcMethod::JobStop),
        _ => None,
    }
}

#[derive(Debug)]
struct RcloneRcDaemon {
    socket_path: PathBuf,
    binary: OsString,
    child: Child,
}

#[derive(Debug, Default)]
struct RcloneRcState {
    daemon: Option<RcloneRcDaemon>,
    startup_blocked_until: Option<Instant>,
    startup_blocked_binary: Option<OsString>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RcloneRcHealth {
    pub enabled: bool,
    pub read_enabled: bool,
    pub write_enabled: bool,
    pub daemon_running: bool,
    pub socket_path: Option<String>,
    pub socket_exists: bool,
}

#[derive(Debug, Clone)]
pub struct RcloneRcClient {
    binary: OsString,
    read_enabled_override: Option<bool>,
    write_enabled_override: Option<bool>,
}

impl Default for RcloneRcClient {
    fn default() -> Self {
        Self {
            binary: std::ffi::OsString::from("rclone"),
            read_enabled_override: None,
            write_enabled_override: None,
        }
    }
}

impl RcloneRcClient {
    pub fn new(binary: impl Into<OsString>) -> Self {
        Self {
            binary: binary.into(),
            read_enabled_override: None,
            write_enabled_override: None,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.is_read_enabled() || self.is_write_enabled()
    }

    pub fn is_read_enabled(&self) -> bool {
        if let Some(enabled) = self.read_enabled_override {
            return enabled;
        }
        rc_read_enabled()
    }

    pub fn is_write_enabled(&self) -> bool {
        if let Some(enabled) = self.write_enabled_override {
            return enabled;
        }
        rc_write_enabled()
    }

    #[cfg(test)]
    pub fn with_enabled_override_for_tests(mut self, enabled: bool) -> Self {
        self.read_enabled_override = Some(enabled);
        self.write_enabled_override = Some(enabled);
        self
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

    pub fn operations_mkdir(
        &self,
        fs_spec: &str,
        remote_path: &str,
    ) -> Result<Value, RcloneCliError> {
        self.run_method(
            RcloneRcMethod::OperationsMkdir,
            json!({
                "fs": fs_spec,
                "remote": remote_path,
            }),
        )
    }

    pub fn operations_deletefile(
        &self,
        fs_spec: &str,
        remote_path: &str,
        cancel_token: Option<&AtomicBool>,
    ) -> Result<Value, RcloneCliError> {
        let payload = json!({
            "fs": fs_spec,
            "remote": remote_path,
        });
        self.run_method_async_if_cancelable(
            RcloneRcMethod::OperationsDeleteFile,
            payload,
            cancel_token,
        )
    }

    pub fn operations_purge(
        &self,
        fs_spec: &str,
        remote_path: &str,
    ) -> Result<Value, RcloneCliError> {
        self.run_method(
            RcloneRcMethod::OperationsPurge,
            json!({
                "fs": fs_spec,
                "remote": remote_path,
            }),
        )
    }

    pub fn operations_rmdir(
        &self,
        fs_spec: &str,
        remote_path: &str,
    ) -> Result<Value, RcloneCliError> {
        self.run_method(
            RcloneRcMethod::OperationsRmdir,
            json!({
                "fs": fs_spec,
                "remote": remote_path,
            }),
        )
    }

    pub fn operations_copyfile(
        &self,
        src_fs: &str,
        src_remote: &str,
        dst_fs: &str,
        dst_remote: &str,
        cancel_token: Option<&AtomicBool>,
    ) -> Result<Value, RcloneCliError> {
        let payload = json!({
            "srcFs": src_fs,
            "srcRemote": src_remote,
            "dstFs": dst_fs,
            "dstRemote": dst_remote,
        });
        self.run_method_async_if_cancelable(
            RcloneRcMethod::OperationsCopyFile,
            payload,
            cancel_token,
        )
    }

    pub fn operations_movefile(
        &self,
        src_fs: &str,
        src_remote: &str,
        dst_fs: &str,
        dst_remote: &str,
    ) -> Result<Value, RcloneCliError> {
        self.run_method(
            RcloneRcMethod::OperationsMoveFile,
            json!({
                "srcFs": src_fs,
                "srcRemote": src_remote,
                "dstFs": dst_fs,
                "dstRemote": dst_remote,
            }),
        )
    }

    fn job_status(&self, job_id: u64) -> Result<Value, RcloneCliError> {
        self.run_method(RcloneRcMethod::JobStatus, json!({ "jobid": job_id }))
    }

    fn job_stop(&self, job_id: u64) -> Result<Value, RcloneCliError> {
        self.run_method(RcloneRcMethod::JobStop, json!({ "jobid": job_id }))
    }

    fn run_method_async_if_cancelable(
        &self,
        method: RcloneRcMethod,
        payload: Value,
        cancel_token: Option<&AtomicBool>,
    ) -> Result<Value, RcloneCliError> {
        if cancel_token.is_none() {
            return self.run_method(method, payload);
        }
        self.run_method_async_with_job_control(method, payload, cancel_token)
    }

    fn run_method_async_with_job_control(
        &self,
        method: RcloneRcMethod,
        mut payload: Value,
        cancel_token: Option<&AtomicBool>,
    ) -> Result<Value, RcloneCliError> {
        let Some(payload_obj) = payload.as_object_mut() else {
            return Err(RcloneCliError::Io(io::Error::other(format!(
                "rclone rc {} async payload must be a JSON object",
                method.as_str()
            ))));
        };
        payload_obj.insert("_async".to_string(), Value::Bool(true));

        let kickoff = self.run_method(method, payload)?;
        let job_id = kickoff
            .get("jobid")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                RcloneCliError::Io(io::Error::other(format!(
                    "rclone rc {} async response missing numeric `jobid`",
                    method.as_str()
                )))
            })?;

        let total_timeout = async_method_total_timeout(method);
        let deadline = Instant::now() + total_timeout;

        loop {
            if is_cancelled(cancel_token) {
                if let Err(error) = self.job_stop(job_id) {
                    warn!(
                        method = method.as_str(),
                        job_id,
                        error = %error,
                        "failed to stop cancelled rclone rc job"
                    );
                }
                return Err(RcloneCliError::Cancelled {
                    subcommand: RcloneSubcommand::Rc,
                });
            }

            let status = self.job_status(job_id)?;
            let finished = status
                .get("finished")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if finished {
                let success = status
                    .get("success")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if success {
                    return Ok(status);
                }
                let message = status
                    .get("error")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|msg| !msg.is_empty())
                    .unwrap_or("rclone rc async job failed");
                return Err(RcloneCliError::Io(io::Error::other(format!(
                    "rclone rc {} async job {job_id} failed: {message}",
                    method.as_str()
                ))));
            }

            if Instant::now() >= deadline {
                if let Err(error) = self.job_stop(job_id) {
                    warn!(
                        method = method.as_str(),
                        job_id,
                        error = %error,
                        "failed to stop timed-out rclone rc job"
                    );
                }
                return Err(RcloneCliError::Timeout {
                    subcommand: RcloneSubcommand::Rc,
                    timeout: total_timeout,
                    stdout: String::new(),
                    stderr: format!("rclone rc {} async job {job_id} timed out", method.as_str()),
                });
            }

            std::thread::sleep(RCLONE_RC_ASYNC_POLL_SLICE);
        }
    }

    fn run_method(&self, method: RcloneRcMethod, payload: Value) -> Result<Value, RcloneCliError> {
        if allowlisted_method_from_name(method.as_str()).is_none() {
            return Err(RcloneCliError::Io(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "rclone rc method is not allowlisted",
            )));
        }
        let method_enabled = match method {
            RcloneRcMethod::CoreNoop => self.is_enabled(),
            RcloneRcMethod::ConfigListRemotes
            | RcloneRcMethod::ConfigDump
            | RcloneRcMethod::OperationsList
            | RcloneRcMethod::OperationsStat => self.is_read_enabled(),
            RcloneRcMethod::OperationsMkdir
            | RcloneRcMethod::OperationsDeleteFile
            | RcloneRcMethod::OperationsPurge
            | RcloneRcMethod::OperationsRmdir
            | RcloneRcMethod::OperationsCopyFile
            | RcloneRcMethod::OperationsMoveFile
            | RcloneRcMethod::JobStatus
            | RcloneRcMethod::JobStop => self.is_write_enabled(),
        };
        if !method_enabled {
            return Err(RcloneCliError::Io(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("rclone rc method {} backend is disabled", method.as_str()),
            )));
        }
        let socket_path = self.ensure_daemon_ready()?;
        let timeout = method_timeout(method);
        let retry_safe = method_is_retry_safe(method);
        let mut attempt = 0usize;
        loop {
            attempt += 1;
            let started = Instant::now();
            let result = run_rc_command_via_socket(&socket_path, method, payload.clone(), timeout);
            let should_retry = result
                .as_ref()
                .err()
                .map(|error| {
                    retry_safe && attempt <= RCLONE_RC_MAX_RETRIES && is_retryable_rc_error(error)
                })
                .unwrap_or(false);
            info!(
                method = method.as_str(),
                elapsed_ms = started.elapsed().as_millis() as u64,
                timeout_ms = timeout.as_millis() as u64,
                attempt,
                success = result.is_ok(),
                will_retry = should_retry,
                "rclone rc method completed"
            );
            if should_retry {
                std::thread::sleep(RCLONE_RC_RETRY_BACKOFF);
                continue;
            }
            return result;
        }
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
        let now = Instant::now();
        if let Some(until) = state.startup_blocked_until {
            let binary_matches = state
                .startup_blocked_binary
                .as_ref()
                .map(|blocked| blocked == &self.binary)
                .unwrap_or(false);
            if until > now && binary_matches {
                return Err(RcloneCliError::Io(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    format!(
                        "rclone rc startup cooldown active ({}ms remaining)",
                        until.saturating_duration_since(now).as_millis() as u64
                    ),
                )));
            }
            state.startup_blocked_until = None;
            state.startup_blocked_binary = None;
        }

        if let Some(existing) = state.daemon.as_mut() {
            if existing.binary != self.binary {
                info!(
                    existing_binary = %existing.binary.to_string_lossy(),
                    requested_binary = %self.binary.to_string_lossy(),
                    "restarting rclone rcd daemon for different binary path"
                );
                let _ = kill_daemon(existing);
                state.daemon = None;
            }
        }

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

        let daemon = match spawn_daemon(&self.binary) {
            Ok(daemon) => daemon,
            Err(error) => {
                let cooldown_until = Instant::now() + RCLONE_RC_START_FAILURE_COOLDOWN;
                state.startup_blocked_until = Some(cooldown_until);
                state.startup_blocked_binary = Some(self.binary.clone());
                warn!(
                    cooldown_ms = RCLONE_RC_START_FAILURE_COOLDOWN.as_millis() as u64,
                    error = %error,
                    "rclone rcd startup failed; applying startup cooldown"
                );
                return Err(error);
            }
        };
        let socket_path = daemon.socket_path.clone();
        info!(socket = %socket_path.display(), "started rclone rcd daemon");
        state.startup_blocked_until = None;
        state.startup_blocked_binary = None;
        state.daemon = Some(daemon);
        Ok(socket_path)
    }
}

fn is_cancelled(cancel_token: Option<&AtomicBool>) -> bool {
    cancel_token
        .map(|token| token.load(std::sync::atomic::Ordering::SeqCst))
        .unwrap_or(false)
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

#[cfg(test)]
pub(crate) fn reset_state_for_tests() {
    let mut state = match rclone_rc_state().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(mut daemon) = state.daemon.take() {
        let _ = kill_daemon(&mut daemon);
    }
    state.startup_blocked_until = None;
    state.startup_blocked_binary = None;
}

pub fn health_snapshot() -> RcloneRcHealth {
    let read_enabled = rc_read_enabled();
    let write_enabled = rc_write_enabled();
    let enabled = read_enabled || write_enabled;
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
        read_enabled,
        write_enabled,
        daemon_running,
        socket_path,
        socket_exists,
    }
}

fn parse_rc_toggle_env(var_name: &str) -> Option<bool> {
    let value = env::var(var_name).ok()?;
    parse_rc_toggle_value(&value)
}

fn parse_rc_toggle_value(value: &str) -> Option<bool> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn rc_read_enabled() -> bool {
    static READ_ENABLED: OnceLock<bool> = OnceLock::new();
    *READ_ENABLED.get_or_init(|| {
        if !cfg!(target_os = "linux") {
            return false;
        }
        if let Some(all_enabled) = parse_rc_toggle_env(RCLONE_RC_ENABLE_ENV) {
            return all_enabled;
        }
        parse_rc_toggle_env(RCLONE_RC_READ_ENABLE_ENV).unwrap_or(true)
    })
}

fn rc_write_enabled() -> bool {
    static WRITE_ENABLED: OnceLock<bool> = OnceLock::new();
    *WRITE_ENABLED.get_or_init(|| {
        if !cfg!(target_os = "linux") {
            return false;
        }
        if let Some(all_enabled) = parse_rc_toggle_env(RCLONE_RC_ENABLE_ENV) {
            return all_enabled;
        }
        parse_rc_toggle_env(RCLONE_RC_WRITE_ENABLE_ENV).unwrap_or(true)
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
    cleanup_stale_socket(&socket_path).map_err(RcloneCliError::Io)?;

    let mut child = Command::new(binary)
        .arg("rcd")
        // We bind to a private unix socket in XDG_RUNTIME_DIR/browsey-rclone-rc (0700),
        // so we can keep auth disabled without exposing an HTTP listener.
        .arg("--rc-no-auth")
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

    Ok(RcloneRcDaemon {
        socket_path,
        binary: binary.clone(),
        child,
    })
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

fn cleanup_stale_socket(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_dir() {
                return Err(io::Error::other(
                    "rc socket path points to a directory; refusing to remove",
                ));
            }
            fs::remove_file(path)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
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
    serde_json::from_str::<Value>(&body).map_err(|error| {
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

fn parse_http_response_body(
    response_text: &str,
    method: RcloneRcMethod,
) -> Result<String, RcloneCliError> {
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
    let chunked = lines.any(|line| {
        let mut parts = line.splitn(2, ':');
        let name = parts.next().unwrap_or_default().trim();
        let value = parts.next().unwrap_or_default().trim();
        name.eq_ignore_ascii_case("transfer-encoding")
            && value
                .split(',')
                .any(|encoding| encoding.trim().eq_ignore_ascii_case("chunked"))
    });
    let decoded_body = if chunked {
        decode_chunked_http_body(body, method)?
    } else {
        body.to_string()
    };
    if !(200..300).contains(&status_code) {
        let body_scrubbed = scrub_rc_error_text(decoded_body.trim());
        return Err(RcloneCliError::Io(io::Error::other(format!(
            "rclone rc {} failed with HTTP {} ({})",
            method.as_str(),
            status_code,
            body_scrubbed
        ))));
    }
    Ok(decoded_body)
}

fn decode_chunked_http_body(body: &str, method: RcloneRcMethod) -> Result<String, RcloneCliError> {
    let bytes = body.as_bytes();
    let mut cursor = 0usize;
    let mut out = Vec::<u8>::new();

    loop {
        let Some(line_end_offset) = bytes[cursor..].windows(2).position(|w| w == b"\r\n") else {
            return Err(RcloneCliError::Io(io::Error::other(format!(
                "invalid chunked HTTP response from rclone rc {}: missing chunk size line terminator",
                method.as_str()
            ))));
        };
        let line_end = cursor + line_end_offset;
        let size_line = std::str::from_utf8(&bytes[cursor..line_end]).map_err(|error| {
            RcloneCliError::Io(io::Error::other(format!(
                "invalid chunked HTTP response from rclone rc {}: non-utf8 chunk size: {error}",
                method.as_str()
            )))
        })?;
        let size_hex = size_line.split(';').next().unwrap_or_default().trim();
        let chunk_size = usize::from_str_radix(size_hex, 16).map_err(|error| {
            RcloneCliError::Io(io::Error::other(format!(
                "invalid chunked HTTP response from rclone rc {}: bad chunk size `{size_hex}`: {error}",
                method.as_str()
            )))
        })?;
        cursor = line_end + 2;

        if chunk_size == 0 {
            break;
        }

        let chunk_end = cursor.saturating_add(chunk_size);
        if chunk_end + 2 > bytes.len() {
            return Err(RcloneCliError::Io(io::Error::other(format!(
                "invalid chunked HTTP response from rclone rc {}: truncated chunk payload",
                method.as_str()
            ))));
        }

        out.extend_from_slice(&bytes[cursor..chunk_end]);
        if &bytes[chunk_end..chunk_end + 2] != b"\r\n" {
            return Err(RcloneCliError::Io(io::Error::other(format!(
                "invalid chunked HTTP response from rclone rc {}: missing chunk terminator",
                method.as_str()
            ))));
        }
        cursor = chunk_end + 2;
    }

    String::from_utf8(out).map_err(|error| {
        RcloneCliError::Io(io::Error::other(format!(
            "invalid UTF-8 body from chunked rclone rc {} response: {error}",
            method.as_str()
        )))
    })
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
    use super::{
        allowlisted_method_from_name, cleanup_stale_socket, is_retryable_rc_error, kill_daemon,
        method_is_retry_safe, method_timeout, parse_http_response_body, parse_rc_toggle_value,
        prepare_state_dir, run_rc_command_via_socket, scrub_rc_error_text, RcloneRcDaemon,
        RcloneRcMethod, RCLONE_RC_READ_TIMEOUT, RCLONE_RC_WRITE_TIMEOUT,
    };
    use serde_json::{json, Value};
    use std::io;
    use std::time::Duration;

    #[cfg(unix)]
    use std::{
        fs,
        io::{Read, Write},
        os::unix::fs::symlink,
        os::unix::net::UnixListener,
        path::PathBuf,
        process::Command,
        sync::atomic::{AtomicU64, Ordering},
        thread,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[cfg(unix)]
    fn unique_test_dir(label: &str) -> PathBuf {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        let unique = format!(
            "browsey-rclone-rc-test-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
                + u128::from(NEXT_ID.fetch_add(1, Ordering::Relaxed))
        );
        let root = std::env::temp_dir().join(unique);
        fs::create_dir_all(&root).expect("create temp test dir");
        root
    }

    #[cfg(unix)]
    fn spawn_fake_rc_http_server(
        response: &'static str,
        delay: Duration,
    ) -> (PathBuf, PathBuf, thread::JoinHandle<()>) {
        let root = unique_test_dir("sock");
        let socket_path = root.join("rc.sock");
        let listener = UnixListener::bind(&socket_path).expect("bind unix socket");
        let handle = thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut request = [0u8; 4096];
                let _ = stream.read(&mut request);
                if !delay.is_zero() {
                    thread::sleep(delay);
                }
                let _ = stream.write_all(response.as_bytes());
            }
        });
        (root, socket_path, handle)
    }

    #[test]
    fn rc_method_allowlist_maps_to_expected_endpoint_names() {
        assert_eq!(RcloneRcMethod::CoreNoop.as_str(), "rc/noop");
        assert_eq!(
            RcloneRcMethod::ConfigListRemotes.as_str(),
            "config/listremotes"
        );
        assert_eq!(RcloneRcMethod::ConfigDump.as_str(), "config/dump");
        assert_eq!(RcloneRcMethod::OperationsList.as_str(), "operations/list");
        assert_eq!(RcloneRcMethod::OperationsStat.as_str(), "operations/stat");
        assert_eq!(RcloneRcMethod::OperationsMkdir.as_str(), "operations/mkdir");
        assert_eq!(
            RcloneRcMethod::OperationsDeleteFile.as_str(),
            "operations/deletefile"
        );
        assert_eq!(RcloneRcMethod::OperationsPurge.as_str(), "operations/purge");
        assert_eq!(RcloneRcMethod::OperationsRmdir.as_str(), "operations/rmdir");
        assert_eq!(
            RcloneRcMethod::OperationsCopyFile.as_str(),
            "operations/copyfile"
        );
        assert_eq!(
            RcloneRcMethod::OperationsMoveFile.as_str(),
            "operations/movefile"
        );
        assert_eq!(RcloneRcMethod::JobStatus.as_str(), "job/status");
        assert_eq!(RcloneRcMethod::JobStop.as_str(), "job/stop");
    }

    #[test]
    fn rc_method_allowlist_rejects_unsafe_or_unknown_method_names() {
        assert_eq!(
            allowlisted_method_from_name("rc/noop"),
            Some(RcloneRcMethod::CoreNoop)
        );
        assert_eq!(
            allowlisted_method_from_name("operations/list"),
            Some(RcloneRcMethod::OperationsList)
        );
        assert_eq!(
            allowlisted_method_from_name("job/status"),
            Some(RcloneRcMethod::JobStatus)
        );
        assert_eq!(
            allowlisted_method_from_name("job/stop"),
            Some(RcloneRcMethod::JobStop)
        );
        assert_eq!(allowlisted_method_from_name("../rc/noop"), None);
        assert_eq!(allowlisted_method_from_name("rc/noop?x=1"), None);
        assert_eq!(allowlisted_method_from_name("sync/copy"), None);
    }

    #[test]
    fn parses_rc_toggle_values() {
        assert_eq!(parse_rc_toggle_value("1"), Some(true));
        assert_eq!(parse_rc_toggle_value("true"), Some(true));
        assert_eq!(parse_rc_toggle_value("ON"), Some(true));
        assert_eq!(parse_rc_toggle_value("0"), Some(false));
        assert_eq!(parse_rc_toggle_value("false"), Some(false));
        assert_eq!(parse_rc_toggle_value("Off"), Some(false));
        assert_eq!(parse_rc_toggle_value("maybe"), None);
    }

    #[test]
    fn rc_method_timeouts_are_classified_by_endpoint_class() {
        assert_eq!(
            method_timeout(RcloneRcMethod::OperationsList),
            RCLONE_RC_READ_TIMEOUT
        );
        assert_eq!(
            method_timeout(RcloneRcMethod::ConfigListRemotes),
            RCLONE_RC_READ_TIMEOUT
        );
        assert_eq!(
            method_timeout(RcloneRcMethod::OperationsMkdir),
            RCLONE_RC_WRITE_TIMEOUT
        );
        assert_eq!(
            method_timeout(RcloneRcMethod::OperationsDeleteFile),
            RCLONE_RC_WRITE_TIMEOUT
        );
        assert_eq!(
            method_timeout(RcloneRcMethod::OperationsCopyFile),
            RCLONE_RC_WRITE_TIMEOUT
        );
        assert_eq!(
            method_timeout(RcloneRcMethod::JobStatus),
            RCLONE_RC_READ_TIMEOUT
        );
        assert_eq!(
            method_timeout(RcloneRcMethod::JobStop),
            RCLONE_RC_WRITE_TIMEOUT
        );
    }

    #[test]
    fn retry_policy_only_allows_retry_safe_methods() {
        assert!(method_is_retry_safe(RcloneRcMethod::OperationsList));
        assert!(method_is_retry_safe(RcloneRcMethod::ConfigDump));
        assert!(method_is_retry_safe(RcloneRcMethod::JobStatus));
        assert!(method_is_retry_safe(RcloneRcMethod::JobStop));
        assert!(!method_is_retry_safe(RcloneRcMethod::OperationsMkdir));
        assert!(!method_is_retry_safe(RcloneRcMethod::OperationsDeleteFile));
        assert!(!method_is_retry_safe(RcloneRcMethod::OperationsMoveFile));
    }

    #[test]
    fn retry_policy_recognizes_transient_rc_errors() {
        assert!(is_retryable_rc_error(&super::RcloneCliError::Timeout {
            subcommand: super::RcloneSubcommand::Rc,
            timeout: Duration::from_secs(1),
            stdout: String::new(),
            stderr: String::new(),
        }));
        assert!(is_retryable_rc_error(&super::RcloneCliError::Io(
            io::Error::new(io::ErrorKind::ConnectionReset, "reset")
        )));
        assert!(!is_retryable_rc_error(&super::RcloneCliError::Io(
            io::Error::other("http 403")
        )));
    }

    #[test]
    fn parses_success_http_response_body() {
        let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"ok\":true}";
        let body = parse_http_response_body(response, RcloneRcMethod::CoreNoop).expect("body");
        assert_eq!(body, "{\"ok\":true}");
    }

    #[test]
    fn parses_chunked_success_http_response_body() {
        let response = concat!(
            "HTTP/1.1 200 OK\r\n",
            "Transfer-Encoding: chunked\r\n",
            "\r\n",
            "4\r\n",
            "{\"ok\r\n",
            "7\r\n",
            "\":true}\r\n",
            "0\r\n",
            "\r\n"
        );
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

    #[cfg(unix)]
    #[test]
    fn rc_socket_request_success_path_parses_json_body() {
        let response =
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"ok\":true,\"n\":1}";
        let (root, socket_path, handle) =
            spawn_fake_rc_http_server(response, Duration::from_millis(0));
        let value = run_rc_command_via_socket(
            &socket_path,
            RcloneRcMethod::CoreNoop,
            json!({"ping":"pong"}),
            Duration::from_secs(1),
        )
        .expect("rc success response");
        assert_eq!(value.get("ok"), Some(&Value::Bool(true)));
        assert_eq!(value.get("n"), Some(&Value::from(1)));
        handle.join().expect("join fake server");
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn rc_socket_request_returns_timeout_when_server_stalls() {
        let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"ok\":true}";
        let (root, socket_path, handle) =
            spawn_fake_rc_http_server(response, Duration::from_millis(220));
        let err = run_rc_command_via_socket(
            &socket_path,
            RcloneRcMethod::CoreNoop,
            json!({}),
            Duration::from_millis(40),
        )
        .expect_err("expected timeout");
        assert!(
            matches!(err, super::RcloneCliError::Timeout { .. }),
            "unexpected error: {err}"
        );
        handle.join().expect("join fake server");
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn rc_socket_request_rejects_malformed_json_payload() {
        let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\nnot-json";
        let (root, socket_path, handle) =
            spawn_fake_rc_http_server(response, Duration::from_millis(0));
        let err = run_rc_command_via_socket(
            &socket_path,
            RcloneRcMethod::CoreNoop,
            json!({}),
            Duration::from_secs(1),
        )
        .expect_err("expected malformed payload failure");
        assert!(
            err.to_string().contains("invalid JSON"),
            "unexpected error: {err}"
        );
        handle.join().expect("join fake server");
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn rc_socket_request_fails_cleanly_when_socket_is_unavailable() {
        let root = unique_test_dir("missing");
        let socket_path = root.join("missing.sock");
        let err = run_rc_command_via_socket(
            &socket_path,
            RcloneRcMethod::CoreNoop,
            json!({}),
            Duration::from_millis(60),
        )
        .expect_err("expected unavailable socket error");
        assert!(
            matches!(err, super::RcloneCliError::Io(_)),
            "unexpected error: {err}"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn lifecycle_kill_daemon_terminates_process_and_cleans_socket_path() {
        let root = unique_test_dir("daemon");
        let socket_path = root.join("daemon.sock");
        fs::write(&socket_path, "placeholder").expect("create socket placeholder");
        let child = Command::new("sh")
            .arg("-c")
            .arg("sleep 30")
            .spawn()
            .expect("spawn sleep child");
        let mut daemon = RcloneRcDaemon {
            socket_path,
            binary: std::ffi::OsString::from("rclone"),
            child,
        };
        kill_daemon(&mut daemon).expect("kill daemon");
        assert!(
            daemon
                .child
                .try_wait()
                .expect("query child status")
                .is_some(),
            "child process should be terminated"
        );
        assert!(
            !daemon.socket_path.exists(),
            "socket path should be removed by shutdown"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn lifecycle_prepare_state_dir_enforces_user_only_permissions() {
        let root = unique_test_dir("state-dir-perms");
        let state_dir = root.join("rc-state");
        prepare_state_dir(&state_dir).expect("prepare state dir");
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&state_dir)
            .expect("state dir metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o700);
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn stale_socket_cleanup_removes_symlink_without_touching_target() {
        let root = unique_test_dir("stale-symlink");
        let target_file = root.join("target.txt");
        fs::write(&target_file, "keep-me").expect("write target file");
        let stale_socket = root.join("rcd.sock");
        symlink(&target_file, &stale_socket).expect("create stale symlink");

        cleanup_stale_socket(&stale_socket).expect("cleanup stale socket");
        assert!(!stale_socket.exists(), "stale symlink should be removed");
        let target = fs::read_to_string(&target_file).expect("read target");
        assert_eq!(target, "keep-me");
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn stale_socket_cleanup_removes_regular_file() {
        let root = unique_test_dir("stale-file");
        let stale_socket = root.join("rcd.sock");
        fs::write(&stale_socket, "stale").expect("write stale socket file");
        cleanup_stale_socket(&stale_socket).expect("cleanup stale regular file");
        assert!(!stale_socket.exists());
        let _ = fs::remove_dir_all(root);
    }
}
