use crate::errors::{
    api_error::ApiResult,
    domain::{self, DomainError, ErrorCode},
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NetworkErrorCode {
    InvalidUri,
    UnsupportedUri,
    UnsupportedScheme,
    OpenFailed,
    DiscoveryFailed,
    MountFailed,
    HistoryFailed,
    EjectFailed,
    FormatNotAllowed,
    FormatFailed,
    FormatBusy,
    FormatStatusUnknown,
    TaskFailed,
}

impl ErrorCode for NetworkErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::InvalidUri => "invalid_uri",
            Self::UnsupportedUri => "unsupported_uri",
            Self::UnsupportedScheme => "unsupported_scheme",
            Self::OpenFailed => "open_failed",
            Self::DiscoveryFailed => "discovery_failed",
            Self::MountFailed => "mount_failed",
            Self::HistoryFailed => "network_history_failed",
            Self::EjectFailed => "eject_failed",
            Self::FormatNotAllowed => "format_not_allowed",
            Self::FormatFailed => "format_failed",
            Self::FormatBusy => "format_busy",
            Self::FormatStatusUnknown => "format_status_unknown",
            Self::TaskFailed => "task_failed",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct NetworkError {
    code: NetworkErrorCode,
    message: String,
}

impl NetworkError {
    pub(super) fn new(code: NetworkErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub(super) fn message(&self) -> &str {
        &self.message
    }

    #[cfg(not(target_os = "windows"))]
    pub(super) fn from_io_error(
        code: NetworkErrorCode,
        context: &str,
        error: std::io::Error,
    ) -> Self {
        // The operation determines the public code; I/O text is diagnostic only.
        Self::new(code, format!("{context}: {error}"))
    }
}

impl fmt::Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for NetworkError {}

impl From<tauri::Error> for NetworkError {
    fn from(error: tauri::Error) -> Self {
        // Tauri's task handle returns its own typed error at the runtime boundary.
        Self::new(NetworkErrorCode::TaskFailed, error.to_string())
    }
}

impl DomainError for NetworkError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

impl From<crate::watcher::WatcherError> for NetworkError {
    fn from(error: crate::watcher::WatcherError) -> Self {
        let code = match error.code() {
            crate::watcher::WatcherErrorCode::StateLock => NetworkErrorCode::TaskFailed,
            crate::watcher::WatcherErrorCode::Create
            | crate::watcher::WatcherErrorCode::WatchPath => NetworkErrorCode::EjectFailed,
        };
        Self::new(code, error.message())
    }
}

impl From<crate::commands::fs::FsError> for NetworkError {
    fn from(error: crate::commands::fs::FsError) -> Self {
        let code = match error.code() {
            crate::commands::fs::FsErrorCode::TaskFailed => NetworkErrorCode::TaskFailed,
            _ => NetworkErrorCode::EjectFailed,
        };
        Self::new(code, error.message())
    }
}

pub(super) type NetworkResult<T> = Result<T, NetworkError>;

pub(super) fn map_api_result<T>(result: NetworkResult<T>) -> ApiResult<T> {
    domain::map_api_result(result)
}

#[cfg(test)]
mod tests {
    use super::NetworkError;
    use crate::errors::domain::DomainError;
    use crate::watcher::{WatcherError, WatcherErrorCode};

    #[test]
    fn maps_watcher_error_to_task_failed() {
        let watcher_error = WatcherError::new(WatcherErrorCode::StateLock, "lock failed");
        let network_error = NetworkError::from(watcher_error);
        assert_eq!(network_error.code_str(), "task_failed");
        assert_eq!(network_error.message(), "lock failed");
    }

    #[test]
    fn maps_fs_error_to_eject_failed() {
        let fs_error: crate::commands::fs::FsError = "eject failed".into();
        let network_error = NetworkError::from(fs_error);
        assert_eq!(network_error.code_str(), "eject_failed");
        assert_eq!(network_error.message(), "eject failed");
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn device_inspection_io_errors_keep_the_operation_code_and_context() {
        for kind in [
            std::io::ErrorKind::NotFound,
            std::io::ErrorKind::PermissionDenied,
        ] {
            let error = NetworkError::from_io_error(
                super::NetworkErrorCode::FormatFailed,
                "Could not inspect the selected device",
                std::io::Error::new(kind, "fixture I/O failure"),
            );
            assert_eq!(error.code_str(), "format_failed");
            assert_eq!(
                error.message(),
                "Could not inspect the selected device: fixture I/O failure"
            );
        }
    }

    #[test]
    fn task_errors_keep_the_task_code_without_message_classification() {
        let error =
            NetworkError::from(tauri::Error::Io(std::io::Error::other("permission denied")));
        assert_eq!(error.code_str(), "task_failed");
        assert!(error.message().contains("permission denied"));
    }
}
