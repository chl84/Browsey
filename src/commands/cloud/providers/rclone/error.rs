use super::{
    write::cloud_write_cancelled_error, CloudCommandError, CloudCommandErrorCode, CloudPath,
    CloudProviderKind, RcloneCliError,
};
use crate::commands::cloud::policy;

pub(super) fn map_rclone_error(error: RcloneCliError) -> CloudCommandError {
    map_rclone_error_for_provider(CloudProviderKind::Onedrive, error)
}

pub(super) fn map_rclone_error_for_remote(
    remote_id: &str,
    error: RcloneCliError,
) -> CloudCommandError {
    let providers = provider_kind_for_remote(remote_id)
        .into_iter()
        .collect::<Vec<_>>();
    map_rclone_error_for_providers(&providers, error)
}

pub(super) fn map_rclone_error_for_paths(
    paths: &[&CloudPath],
    error: RcloneCliError,
) -> CloudCommandError {
    let mut providers = Vec::new();
    for path in paths {
        if let Some(kind) = provider_kind_for_remote(path.remote()) {
            if !providers.contains(&kind) {
                providers.push(kind);
            }
        }
    }
    map_rclone_error_for_providers(&providers, error)
}

pub(super) fn map_rclone_error_for_provider(
    provider: CloudProviderKind,
    error: RcloneCliError,
) -> CloudCommandError {
    map_rclone_error_for_providers(&[provider], error)
}

#[cfg(test)]
pub(super) fn classify_rclone_message_code(
    provider: CloudProviderKind,
    message: &str,
) -> CloudCommandErrorCode {
    classify_rclone_message_code_for_providers(&[provider], message)
}

pub(super) fn classify_provider_rclone_message_code(
    provider: CloudProviderKind,
    message: &str,
) -> Option<CloudCommandErrorCode> {
    policy::classify_provider_rclone_message_code(provider, message)
}

pub(super) fn is_rclone_not_found_text(stderr: &str, stdout: &str) -> bool {
    let hay = if !stderr.trim().is_empty() {
        stderr
    } else {
        stdout
    };
    let lower = hay.to_ascii_lowercase();
    lower.contains("not found")
        || lower.contains("object not found")
        || lower.contains("directory not found")
        || lower.contains("file not found")
}

fn provider_kind_for_remote(remote_id: &str) -> Option<CloudProviderKind> {
    crate::commands::cloud::list_cloud_remotes_sync_best_effort(false)
        .into_iter()
        .find(|remote| remote.id == remote_id)
        .map(|remote| remote.provider)
}

fn map_rclone_error_for_providers(
    providers: &[CloudProviderKind],
    error: RcloneCliError,
) -> CloudCommandError {
    match error {
        RcloneCliError::WriteStateUnknown { operation, cause } => CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed, format!("Cloud write status is unknown after rclone rc {operation}; no automatic retry. Refresh and verify the destination. Cause: {cause}")),
        RcloneCliError::AsyncJobFailed { operation, job_id, message } => {
            let code = classify_rclone_message_code_for_providers(providers, &message);
            let code = if code == CloudCommandErrorCode::UnknownError { CloudCommandErrorCode::TaskFailed } else { code };
            CloudCommandError::new(code, crate::commands::cloud::rclone_cli::failed_job_message(&operation, job_id, &message))
        }
        RcloneCliError::OutputLimit { subcommand, stream, limit } => CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            crate::commands::cloud::rclone_cli::output_limit_message(subcommand, stream, limit)),
        RcloneCliError::Io(io) if io.kind() == std::io::ErrorKind::NotFound => {
            CloudCommandError::new(
                CloudCommandErrorCode::BinaryMissing,
                "rclone not found in PATH",
            )
        }
        RcloneCliError::Io(io) => CloudCommandError::new(
            CloudCommandErrorCode::NetworkError,
            crate::commands::cloud::rclone_cli::sanitize_failure_message(&format!("Failed to run rclone: {io}")),
        ),
        RcloneCliError::Shutdown { .. } => CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            "Application is shutting down; cloud operation was cancelled",
        ),
        RcloneCliError::Cancelled { .. } => cloud_write_cancelled_error(),
        RcloneCliError::AsyncJobStateUnknown {
            operation,
            job_id,
            reason,
            ..
        } => CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            format!(
                "Cloud operation status is unknown after rclone rc {operation} job {job_id}; Browsey did not retry automatically to avoid duplicate operations. Refresh and verify the destination before retrying. Cause: {}",
                crate::commands::cloud::rclone_cli::sanitize_failure_message(reason.trim())
            ),
        ),
        error @ RcloneCliError::Stalled { .. } => CloudCommandError::new(
            CloudCommandErrorCode::Timeout, error.to_string()),
        RcloneCliError::Timeout {
            subcommand,
            timeout,
            ..
        } => CloudCommandError::new(
            CloudCommandErrorCode::Timeout,
            format!(
                "rclone {} timed out after {}s",
                subcommand.as_str(),
                timeout.as_secs()
            ),
        ),
        RcloneCliError::NonZero { status, stderr, stdout } => {
            if status.code() == Some(9) {
                return CloudCommandError::new(CloudCommandErrorCode::TaskFailed,
                    crate::commands::cloud::rclone_cli::NO_TRANSFER_MESSAGE);
            }
            let msg = if !stderr.trim().is_empty() {
                stderr
            } else {
                stdout
            };
            let code = classify_rclone_message_code_for_providers(providers, &msg);
            CloudCommandError::new(code, crate::commands::cloud::rclone_cli::sanitize_failure_message(msg.trim()))
        }
    }
}

fn classify_rclone_message_code_for_providers(
    providers: &[CloudProviderKind],
    message: &str,
) -> CloudCommandErrorCode {
    if let Some(code) = classify_common_rclone_message_code(message) {
        return code;
    }
    for provider in providers {
        if let Some(code) = classify_provider_rclone_message_code(*provider, message) {
            return code;
        }
    }
    CloudCommandErrorCode::UnknownError
}

pub(crate) fn classify_rclone_failure_code(
    provider: Option<CloudProviderKind>,
    message: &str,
) -> CloudCommandErrorCode {
    classify_common_rclone_message_code(message)
        .or_else(|| provider.and_then(|kind| classify_provider_rclone_message_code(kind, message)))
        .unwrap_or(CloudCommandErrorCode::UnknownError)
}

fn classify_common_rclone_message_code(message: &str) -> Option<CloudCommandErrorCode> {
    let lower = message.to_ascii_lowercase();
    if lower.contains("didn't find section") || lower.contains("not configured") {
        return Some(CloudCommandErrorCode::InvalidConfig);
    }
    if lower.contains("already exists")
        || lower.contains("file exists")
        || lower.contains("duplicate object")
        || lower.contains("destination exists")
        || lower.contains("immutable file modified")
    {
        return Some(CloudCommandErrorCode::DestinationExists);
    }
    if lower.contains("permission denied") || lower.contains("access denied") {
        return Some(CloudCommandErrorCode::PermissionDenied);
    }
    if lower.contains("x509:")
        || lower.contains("certificate verify failed")
        || lower.contains("self signed certificate")
        || lower.contains("unknown authority")
        || lower.contains("failed to verify certificate")
        || lower.contains("certificate has expired")
    {
        return Some(CloudCommandErrorCode::TlsCertificateError);
    }
    if lower.contains("too many requests")
        || lower.contains("quota exceeded")
        || lower.contains("rate_limit_exceeded")
        || lower.contains("rate limit")
        || lower.contains("rate-limited")
        || lower.contains("retry after")
        || lower.contains("status code 429")
        || lower.contains("http error 429")
    {
        return Some(CloudCommandErrorCode::RateLimited);
    }
    if lower.contains("unauthorized")
        || lower.contains("authentication failed")
        || lower.contains("login required")
        || lower.contains("invalid_grant")
        || lower.contains("token expired")
        || lower.contains("expired token")
        || lower.contains("status code 401")
        || lower.contains("http error 401")
    {
        return Some(CloudCommandErrorCode::AuthRequired);
    }
    if lower.contains("timed out") || lower.contains("timeout") {
        return Some(CloudCommandErrorCode::Timeout);
    }
    if lower.contains("connection")
        || lower.contains("network")
        || lower.contains("dial tcp")
        || lower.contains("read tcp")
        || lower.contains("write tcp")
        || lower.contains("proxyconnect tcp")
        || lower.contains("no such host")
        || lower.contains("name resolution")
        || lower.contains("tls handshake")
    {
        return Some(CloudCommandErrorCode::NetworkError);
    }
    if is_rclone_not_found_text(message, "")
        || lower.contains("status code 404")
        || lower.contains("http error 404")
    {
        return Some(CloudCommandErrorCode::NotFound);
    }
    None
}
