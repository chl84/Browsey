mod client;
mod daemon;
mod jobs;
mod methods;

use super::rclone_cli::{RcloneCliError, RcloneSubcommand};
use client::run_rc_command_via_socket;
#[cfg(test)]
pub(crate) use daemon::reset_state_for_tests;
pub use daemon::{begin_shutdown_and_kill_daemon, health_snapshot, RcloneRcHealth};
use daemon::{rc_read_enabled, rc_write_enabled, should_recycle_daemon_after_error};
#[cfg(test)]
use jobs::ForcedAsyncStatusErrorState;
pub(crate) use methods::{RcCopyFileFromLocalProgressSpec, RcCopyFileToLocalProgressSpec};
use serde_json::Value;
use std::{
    ffi::OsString,
    io,
    time::{Duration, Instant},
};
use tracing::debug;

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
    CoreStats,
    CoreStatsDelete,
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
            Self::CoreStats => "core/stats",
            Self::CoreStatsDelete => "core/stats-delete",
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
        | RcloneRcMethod::CoreStats
        | RcloneRcMethod::CoreStatsDelete
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
            | RcloneRcMethod::CoreStats
            | RcloneRcMethod::CoreStatsDelete
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
        "core/stats" => Some(RcloneRcMethod::CoreStats),
        "core/stats-delete" => Some(RcloneRcMethod::CoreStatsDelete),
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
    #[cfg(test)]
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
            RcloneRcMethod::CoreStats
            | RcloneRcMethod::CoreStatsDelete
            | RcloneRcMethod::ConfigListRemotes
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
            debug!(
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
}

#[cfg(test)]
mod tests {
    use super::{
        allowlisted_method_from_name, is_retryable_rc_error, method_is_retry_safe, method_timeout,
        RcloneRcMethod, RCLONE_RC_READ_TIMEOUT, RCLONE_RC_WRITE_TIMEOUT,
    };
    use std::io;
    use std::time::Duration;

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
}
