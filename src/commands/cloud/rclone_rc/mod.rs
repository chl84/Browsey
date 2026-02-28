mod daemon;

use super::rclone_cli::{RcloneCliError, RcloneSubcommand};
#[cfg(test)]
pub(crate) use daemon::reset_state_for_tests;
pub use daemon::{begin_shutdown_and_kill_daemon, health_snapshot, RcloneRcHealth};
use daemon::{
    daemon_is_running, kill_daemon, rc_read_enabled, rc_write_enabled, rclone_rc_state,
    should_recycle_daemon_after_error, spawn_daemon,
};
use regex::Regex;
use serde_json::{json, Value};
#[cfg(test)]
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::{
    ffi::OsString,
    io,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, OnceLock},
    time::{Duration, Instant},
};
use tracing::{info, warn};

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
        | RcloneCliError::AsyncJobStateUnknown { .. }
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

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ForcedAsyncStatusErrorMode {
    CopyFile,
    DeleteFile,
}

#[cfg(test)]
#[derive(Debug, Clone)]
struct ForcedAsyncStatusErrorState {
    mode: ForcedAsyncStatusErrorMode,
    job_id: u64,
    status_error_kind: io::ErrorKind,
    job_stop_calls: Arc<AtomicUsize>,
}

#[derive(Debug, Clone)]
pub struct RcloneRcClient {
    binary: OsString,
    read_enabled_override: Option<bool>,
    write_enabled_override: Option<bool>,
    #[cfg(test)]
    forced_async_status_error: Option<ForcedAsyncStatusErrorState>,
}

impl Default for RcloneRcClient {
    fn default() -> Self {
        Self {
            binary: std::ffi::OsString::from("rclone"),
            read_enabled_override: None,
            write_enabled_override: None,
            #[cfg(test)]
            forced_async_status_error: None,
        }
    }
}

impl RcloneRcClient {
    pub fn new(binary: impl Into<OsString>) -> Self {
        Self {
            binary: binary.into(),
            read_enabled_override: None,
            write_enabled_override: None,
            #[cfg(test)]
            forced_async_status_error: None,
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

    #[cfg(test)]
    pub fn with_forced_async_status_error_on_copy_for_tests(
        mut self,
        status_error_kind: io::ErrorKind,
    ) -> Self {
        self.forced_async_status_error = Some(ForcedAsyncStatusErrorState {
            mode: ForcedAsyncStatusErrorMode::CopyFile,
            job_id: 9101,
            status_error_kind,
            job_stop_calls: Arc::new(AtomicUsize::new(0)),
        });
        self
    }

    #[cfg(test)]
    pub fn with_forced_async_status_error_on_delete_for_tests(
        mut self,
        status_error_kind: io::ErrorKind,
    ) -> Self {
        self.forced_async_status_error = Some(ForcedAsyncStatusErrorState {
            mode: ForcedAsyncStatusErrorMode::DeleteFile,
            job_id: 9102,
            status_error_kind,
            job_stop_calls: Arc::new(AtomicUsize::new(0)),
        });
        self
    }

    #[cfg(test)]
    pub fn forced_job_stop_calls_for_tests(&self) -> usize {
        self.forced_async_status_error
            .as_ref()
            .map(|state| state.job_stop_calls.load(Ordering::SeqCst))
            .unwrap_or(0)
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

            let status = match self.job_status(job_id) {
                Ok(status) => status,
                Err(error) => {
                    if let Err(stop_error) = self.job_stop(job_id) {
                        warn!(
                            method = method.as_str(),
                            job_id,
                            status_error = %error,
                            stop_error = %stop_error,
                            "failed to stop async rclone rc job after status polling error"
                        );
                    }
                    return Err(RcloneCliError::AsyncJobStateUnknown {
                        subcommand: RcloneSubcommand::Rc,
                        operation: method.as_str().to_string(),
                        job_id,
                        reason: error.to_string(),
                    });
                }
            };
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
        #[cfg(test)]
        if let Some(forced) = self.run_method_forced_async_status_error_for_tests(method, &payload)
        {
            return forced;
        }
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
            if let Err(error) = &result {
                if should_recycle_daemon_after_error(method, error) {
                    self.recycle_daemon_after_error(method, error);
                }
            }
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

    #[cfg(test)]
    fn run_method_forced_async_status_error_for_tests(
        &self,
        method: RcloneRcMethod,
        payload: &Value,
    ) -> Option<Result<Value, RcloneCliError>> {
        let state = self.forced_async_status_error.as_ref()?;
        match method {
            RcloneRcMethod::OperationsCopyFile
                if state.mode == ForcedAsyncStatusErrorMode::CopyFile =>
            {
                if payload.get("_async").and_then(Value::as_bool) == Some(true) {
                    Some(Ok(json!({ "jobid": state.job_id })))
                } else {
                    Some(Err(RcloneCliError::Io(io::Error::other(
                        "forced copy async test expected `_async: true` payload",
                    ))))
                }
            }
            RcloneRcMethod::OperationsDeleteFile
                if state.mode == ForcedAsyncStatusErrorMode::DeleteFile =>
            {
                if payload.get("_async").and_then(Value::as_bool) == Some(true) {
                    Some(Ok(json!({ "jobid": state.job_id })))
                } else {
                    Some(Err(RcloneCliError::Io(io::Error::other(
                        "forced delete async test expected `_async: true` payload",
                    ))))
                }
            }
            RcloneRcMethod::JobStatus => Some(Err(RcloneCliError::Io(io::Error::new(
                state.status_error_kind,
                "forced job/status transport error for tests",
            )))),
            RcloneRcMethod::JobStop => {
                state.job_stop_calls.fetch_add(1, Ordering::SeqCst);
                Some(Ok(json!({ "stopped": true })))
            }
            _ => None,
        }
    }

    fn recycle_daemon_after_error(&self, method: RcloneRcMethod, error: &RcloneCliError) {
        let mut state = match rclone_rc_state().lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(mut daemon) = state.daemon.take() {
            let socket = daemon.socket_path.display().to_string();
            let _ = kill_daemon(&mut daemon);
            warn!(
                method = method.as_str(),
                error = %error,
                socket = %socket,
                "recycled rclone rcd daemon after transport-level rc failure"
            );
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
        allowlisted_method_from_name, is_retryable_rc_error, method_is_retry_safe, method_timeout,
        parse_http_response_body, run_rc_command_via_socket, scrub_rc_error_text, RcloneRcMethod,
        RCLONE_RC_READ_TIMEOUT, RCLONE_RC_WRITE_TIMEOUT,
    };
    use serde_json::{json, Value};
    use std::io;
    use std::time::Duration;

    #[cfg(unix)]
    use std::{
        fs,
        io::{Read, Write},
        os::unix::net::UnixListener,
        path::PathBuf,
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
        assert!(!method_is_retry_safe(RcloneRcMethod::OperationsStat));
        assert!(method_is_retry_safe(RcloneRcMethod::ConfigDump));
        assert!(method_is_retry_safe(RcloneRcMethod::JobStatus));
        assert!(method_is_retry_safe(RcloneRcMethod::JobStop));
        assert!(!method_is_retry_safe(RcloneRcMethod::OperationsList));
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
        assert!(!is_retryable_rc_error(
            &super::RcloneCliError::AsyncJobStateUnknown {
                subcommand: super::RcloneSubcommand::Rc,
                operation: "operations/copyfile".to_string(),
                job_id: 9,
                reason: "job status unavailable".to_string(),
            }
        ));
    }

    #[test]
    fn async_copy_job_status_error_returns_unknown_and_stops_job() {
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let client = super::RcloneRcClient::default()
            .with_enabled_override_for_tests(true)
            .with_forced_async_status_error_on_copy_for_tests(io::ErrorKind::ConnectionReset);

        let err = client
            .operations_copyfile(
                "work:",
                "src/file.txt",
                "work:",
                "dst/file.txt",
                Some(&cancel),
            )
            .expect_err("forced copy job/status error should fail");
        assert!(matches!(
            err,
            super::RcloneCliError::AsyncJobStateUnknown { .. }
        ));
        assert_eq!(client.forced_job_stop_calls_for_tests(), 1);
    }

    #[test]
    fn async_delete_job_status_error_returns_unknown_and_stops_job() {
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let client = super::RcloneRcClient::default()
            .with_enabled_override_for_tests(true)
            .with_forced_async_status_error_on_delete_for_tests(io::ErrorKind::NotConnected);

        let err = client
            .operations_deletefile("work:", "dst/file.txt", Some(&cancel))
            .expect_err("forced delete job/status error should fail");
        assert!(matches!(
            err,
            super::RcloneCliError::AsyncJobStateUnknown { .. }
        ));
        assert_eq!(client.forced_job_stop_calls_for_tests(), 1);
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
}
