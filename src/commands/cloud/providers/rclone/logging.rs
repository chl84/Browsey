use super::RcloneCliError;
use tracing::debug;

pub(super) fn log_backend_selected(
    op: &'static str,
    backend: &'static str,
    fallback: bool,
    reason: Option<&'static str>,
) {
    debug!(
        op,
        backend,
        fallback_from_rc = fallback,
        reason = reason.unwrap_or(""),
        "cloud provider backend selected"
    );
}

pub(super) fn classify_rc_fallback_reason(error: &RcloneCliError) -> &'static str {
    match error {
        RcloneCliError::Timeout { .. } => "rc_timeout",
        RcloneCliError::Shutdown { .. } => "rc_shutdown",
        RcloneCliError::Cancelled { .. } => "rc_cancelled",
        RcloneCliError::AsyncJobStateUnknown { .. } => "rc_async_job_unknown",
        RcloneCliError::NonZero { .. } => "rc_nonzero",
        RcloneCliError::Io(io) => match io.kind() {
            std::io::ErrorKind::WouldBlock => "rc_startup_cooldown",
            std::io::ErrorKind::TimedOut => "rc_io_timeout",
            std::io::ErrorKind::ConnectionRefused => "rc_connect_refused",
            std::io::ErrorKind::NotConnected => "rc_not_connected",
            std::io::ErrorKind::Unsupported => "rc_unsupported",
            std::io::ErrorKind::PermissionDenied => "rc_permission_denied",
            std::io::ErrorKind::NotFound => "rc_not_found",
            _ => "rc_io_error",
        },
    }
}
