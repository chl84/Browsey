use crate::errors::{
    api_error::ApiResult,
    domain::{self, classify_io_error, DomainError, ErrorCode, IoErrorHint},
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ListingErrorCode {
    InvalidInput,
    PathNotAbsolute,
    InvalidPath,
    NotFound,
    Cancelled,
    PermissionDenied,
    WatchNotAllowed,
    UnsupportedScope,
    TaskFailed,
    UnknownError,
}

impl ErrorCode for ListingErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::PathNotAbsolute => "path_not_absolute",
            Self::InvalidPath => "invalid_path",
            Self::NotFound => "not_found",
            Self::Cancelled => "cancelled",
            Self::PermissionDenied => "permission_denied",
            Self::WatchNotAllowed => "watch_not_allowed",
            Self::UnsupportedScope => "unsupported_scope",
            Self::TaskFailed => "task_failed",
            Self::UnknownError => "unknown_error",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct ListingError {
    code: ListingErrorCode,
    message: String,
}

impl ListingError {
    pub(super) fn new(code: ListingErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub(super) fn from_io_error(
        fallback: ListingErrorCode,
        context: &str,
        error: std::io::Error,
    ) -> Self {
        let code = match classify_io_error(&error) {
            IoErrorHint::NotFound => ListingErrorCode::NotFound,
            IoErrorHint::PermissionDenied => ListingErrorCode::PermissionDenied,
            _ => fallback,
        };
        Self::new(code, format!("{context}: {error}"))
    }
}

impl fmt::Display for ListingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ListingError {}

impl DomainError for ListingError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

impl From<crate::fs_utils::FsUtilsError> for ListingError {
    fn from(error: crate::fs_utils::FsUtilsError) -> Self {
        let code = match error.code() {
            crate::fs_utils::FsUtilsErrorCode::InvalidPath => ListingErrorCode::InvalidPath,
            crate::fs_utils::FsUtilsErrorCode::NotFound => ListingErrorCode::NotFound,
            crate::fs_utils::FsUtilsErrorCode::PermissionDenied => {
                ListingErrorCode::PermissionDenied
            }
            crate::fs_utils::FsUtilsErrorCode::ReadOnlyFilesystem
            | crate::fs_utils::FsUtilsErrorCode::RootForbidden
            | crate::fs_utils::FsUtilsErrorCode::SymlinkUnsupported
            | crate::fs_utils::FsUtilsErrorCode::CanonicalizeFailed
            | crate::fs_utils::FsUtilsErrorCode::MetadataReadFailed => {
                ListingErrorCode::UnknownError
            }
        };
        Self::new(code, error.to_string())
    }
}

impl From<crate::commands::fs::FsError> for ListingError {
    fn from(error: crate::commands::fs::FsError) -> Self {
        let code = match error.code() {
            crate::commands::fs::FsErrorCode::InvalidInput => ListingErrorCode::InvalidInput,
            crate::commands::fs::FsErrorCode::PathNotAbsolute => ListingErrorCode::PathNotAbsolute,
            crate::commands::fs::FsErrorCode::InvalidPath => ListingErrorCode::InvalidPath,
            crate::commands::fs::FsErrorCode::NotFound => ListingErrorCode::NotFound,
            crate::commands::fs::FsErrorCode::PermissionDenied => {
                ListingErrorCode::PermissionDenied
            }
            crate::commands::fs::FsErrorCode::TaskFailed => ListingErrorCode::TaskFailed,
            _ => ListingErrorCode::UnknownError,
        };
        Self::new(code, error.to_string())
    }
}

impl From<crate::watcher::WatcherError> for ListingError {
    fn from(error: crate::watcher::WatcherError) -> Self {
        let code = match error.code() {
            crate::watcher::WatcherErrorCode::StateLock => ListingErrorCode::TaskFailed,
            crate::watcher::WatcherErrorCode::Create
            | crate::watcher::WatcherErrorCode::WatchPath => ListingErrorCode::UnknownError,
        };
        Self::new(code, error.to_string())
    }
}

pub(super) type ListingResult<T> = Result<T, ListingError>;

pub(super) fn map_api_result<T>(result: ListingResult<T>) -> ApiResult<T> {
    domain::map_api_result(result)
}

#[cfg(test)]
mod tests {
    use super::ListingError;
    use crate::errors::domain::DomainError;
    use crate::fs_utils::{FsUtilsError, FsUtilsErrorCode};
    use crate::watcher::{WatcherError, WatcherErrorCode};

    #[test]
    fn maps_fs_error_invalid_input_to_listing_invalid_input() {
        let fs_error: crate::commands::fs::FsError = "No paths provided".into();
        let error = ListingError::from(fs_error);
        assert_eq!(error.code_str(), "invalid_input");
    }

    #[test]
    fn maps_watcher_state_lock_to_listing_task_failed() {
        let watcher_error = WatcherError::new(WatcherErrorCode::StateLock, "lock failed");
        let error = ListingError::from(watcher_error);
        assert_eq!(error.code_str(), "task_failed");
    }

    #[test]
    fn maps_fs_utils_permission_denied_to_listing_permission_denied() {
        let fs_error = FsUtilsError::new(FsUtilsErrorCode::PermissionDenied, "permission denied");
        let error = ListingError::from(fs_error);
        assert_eq!(error.code_str(), "permission_denied");
    }

    #[test]
    fn maps_io_error_to_listing_permission_denied_without_message_reclassification() {
        let io_error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let error = ListingError::from_io_error(
            super::ListingErrorCode::UnknownError,
            "read_dir failed",
            io_error,
        );
        assert_eq!(error.code_str(), "permission_denied");
        assert_eq!(error.message(), "read_dir failed: denied");
    }
}
