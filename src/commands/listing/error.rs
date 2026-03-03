use crate::errors::{
    api_error::ApiResult,
    domain::{
        self, classify_io_hint_from_message, classify_message_by_patterns, DomainError, ErrorCode,
        IoErrorHint, COMMON_INVALID_PATH_PATTERNS, COMMON_PATH_NOT_ABSOLUTE_PATTERNS,
        COMMON_PERMISSION_DENIED_PATTERNS,
    },
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ListingErrorCode {
    InvalidInput,
    PathNotAbsolute,
    InvalidPath,
    NotFound,
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

    pub(super) fn from_external_message(message: impl Into<String>) -> Self {
        let message = message.into();
        if let Some(hint) = classify_io_hint_from_message(&message) {
            let code = match hint {
                IoErrorHint::NotFound => Some(ListingErrorCode::NotFound),
                IoErrorHint::PermissionDenied => Some(ListingErrorCode::PermissionDenied),
                _ => None,
            };
            if let Some(code) = code {
                return Self::new(code, message);
            }
        }

        let code = classify_message_by_patterns(
            &message,
            LISTING_CLASSIFICATION_RULES,
            ListingErrorCode::UnknownError,
        );
        Self::new(code, message)
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
        let code = match error.code_str() {
            "invalid_input" => ListingErrorCode::InvalidInput,
            "path_not_absolute" => ListingErrorCode::PathNotAbsolute,
            "invalid_path" => ListingErrorCode::InvalidPath,
            "not_found" => ListingErrorCode::NotFound,
            "permission_denied" => ListingErrorCode::PermissionDenied,
            "task_failed" => ListingErrorCode::TaskFailed,
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

const LISTING_CLASSIFICATION_RULES: &[(ListingErrorCode, &[&str])] = &[
    (
        ListingErrorCode::TaskFailed,
        &["list_dir task panicked", "list_facets task panicked"],
    ),
    (
        ListingErrorCode::PathNotAbsolute,
        COMMON_PATH_NOT_ABSOLUTE_PATTERNS,
    ),
    (ListingErrorCode::InvalidPath, COMMON_INVALID_PATH_PATTERNS),
    (
        ListingErrorCode::WatchNotAllowed,
        &["watching this path is not allowed"],
    ),
    (
        ListingErrorCode::UnsupportedScope,
        &["unsupported facet scope"],
    ),
    (
        ListingErrorCode::PermissionDenied,
        COMMON_PERMISSION_DENIED_PATTERNS,
    ),
    (
        ListingErrorCode::NotFound,
        &["no such file or directory", "start directory not found"],
    ),
];

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
}
