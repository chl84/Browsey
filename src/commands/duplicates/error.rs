use crate::errors::{
    api_error::ApiResult,
    domain::{self, classify_io_error, DomainError, ErrorCode, IoErrorHint},
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DuplicatesErrorCode {
    InvalidInput,
    PathNotAbsolute,
    InvalidPath,
    NotFound,
    PermissionDenied,
    ScanLimitExceeded,
    TaskFailed,
    UnknownError,
}

impl ErrorCode for DuplicatesErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::PathNotAbsolute => "path_not_absolute",
            Self::InvalidPath => "invalid_path",
            Self::NotFound => "not_found",
            Self::PermissionDenied => "permission_denied",
            Self::ScanLimitExceeded => "scan_limit_exceeded",
            Self::TaskFailed => "task_failed",
            Self::UnknownError => "unknown_error",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct DuplicatesError {
    code: DuplicatesErrorCode,
    message: String,
}

impl DuplicatesError {
    pub(super) fn new(code: DuplicatesErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub(super) fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(DuplicatesErrorCode::InvalidInput, message)
    }

    pub(super) fn cancelled() -> Self {
        Self::new(DuplicatesErrorCode::TaskFailed, "Duplicate scan cancelled")
    }

    pub(super) fn scan_limit_exceeded(message: impl Into<String>) -> Self {
        Self::new(DuplicatesErrorCode::ScanLimitExceeded, message)
    }

    pub(super) fn from_io_error(
        fallback: DuplicatesErrorCode,
        context: &str,
        error: std::io::Error,
    ) -> Self {
        let code = match classify_io_error(&error) {
            IoErrorHint::NotFound => DuplicatesErrorCode::NotFound,
            IoErrorHint::PermissionDenied => DuplicatesErrorCode::PermissionDenied,
            _ => fallback,
        };
        Self::new(code, format!("{context}: {error}"))
    }
}

impl fmt::Display for DuplicatesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for DuplicatesError {}

impl DomainError for DuplicatesError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

impl From<crate::fs_utils::FsUtilsError> for DuplicatesError {
    fn from(error: crate::fs_utils::FsUtilsError) -> Self {
        let code = match error.code() {
            crate::fs_utils::FsUtilsErrorCode::InvalidPath => DuplicatesErrorCode::InvalidPath,
            crate::fs_utils::FsUtilsErrorCode::NotFound => DuplicatesErrorCode::NotFound,
            crate::fs_utils::FsUtilsErrorCode::PermissionDenied => {
                DuplicatesErrorCode::PermissionDenied
            }
            crate::fs_utils::FsUtilsErrorCode::ReadOnlyFilesystem
            | crate::fs_utils::FsUtilsErrorCode::RootForbidden
            | crate::fs_utils::FsUtilsErrorCode::SymlinkUnsupported
            | crate::fs_utils::FsUtilsErrorCode::CanonicalizeFailed
            | crate::fs_utils::FsUtilsErrorCode::MetadataReadFailed => {
                DuplicatesErrorCode::UnknownError
            }
        };
        Self::new(code, error.to_string())
    }
}

impl From<crate::commands::fs::FsError> for DuplicatesError {
    fn from(error: crate::commands::fs::FsError) -> Self {
        let code = match error.code() {
            crate::commands::fs::FsErrorCode::InvalidInput => DuplicatesErrorCode::InvalidInput,
            crate::commands::fs::FsErrorCode::PathNotAbsolute => {
                DuplicatesErrorCode::PathNotAbsolute
            }
            crate::commands::fs::FsErrorCode::InvalidPath => DuplicatesErrorCode::InvalidPath,
            crate::commands::fs::FsErrorCode::NotFound => DuplicatesErrorCode::NotFound,
            crate::commands::fs::FsErrorCode::PermissionDenied => {
                DuplicatesErrorCode::PermissionDenied
            }
            crate::commands::fs::FsErrorCode::TaskFailed => DuplicatesErrorCode::TaskFailed,
            _ => DuplicatesErrorCode::UnknownError,
        };
        Self::new(code, error.to_string())
    }
}

pub(super) type DuplicatesResult<T> = Result<T, DuplicatesError>;

pub(super) fn map_api_result<T>(result: DuplicatesResult<T>) -> ApiResult<T> {
    domain::map_api_result(result)
}

#[cfg(test)]
mod tests {
    use super::DuplicatesError;
    use crate::errors::domain::DomainError;
    use crate::fs_utils::{FsUtilsError, FsUtilsErrorCode};

    #[test]
    fn maps_fs_error_invalid_input_to_duplicates_invalid_input() {
        let fs_error: crate::commands::fs::FsError = "No paths provided".into();
        let error = DuplicatesError::from(fs_error);
        assert_eq!(error.code_str(), "invalid_input");
    }

    #[test]
    fn maps_fs_utils_invalid_path_to_duplicates_invalid_path() {
        let fs_error = FsUtilsError::new(FsUtilsErrorCode::InvalidPath, "invalid path");
        let error = DuplicatesError::from(fs_error);
        assert_eq!(error.code_str(), "invalid_path");
    }

    #[test]
    fn cancelled_duplicate_scan_uses_typed_task_failed_code() {
        let error = DuplicatesError::cancelled();
        assert_eq!(error.code_str(), "task_failed");
        assert_eq!(error.message(), "Duplicate scan cancelled");
    }
}
