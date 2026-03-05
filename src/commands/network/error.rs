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
    EjectFailed,
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
            Self::EjectFailed => "eject_failed",
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
}

impl fmt::Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for NetworkError {}

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
        let code = match error.code_str_value() {
            "task_failed" => NetworkErrorCode::TaskFailed,
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
}
