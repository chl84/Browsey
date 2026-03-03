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

    pub(super) fn from_external_message(message: impl Into<String>) -> Self {
        let message = message.into();
        if let Some(hint) = classify_io_hint_from_message(&message) {
            let code = match hint {
                IoErrorHint::NotFound => Some(DuplicatesErrorCode::NotFound),
                IoErrorHint::PermissionDenied => Some(DuplicatesErrorCode::PermissionDenied),
                _ => None,
            };
            if let Some(code) = code {
                return Self::new(code, message);
            }
        }
        let code = classify_message_by_patterns(
            &message,
            DUPLICATES_CLASSIFICATION_RULES,
            DuplicatesErrorCode::UnknownError,
        );
        Self::new(code, message)
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
        let code = match error.code_str() {
            "invalid_input" => DuplicatesErrorCode::InvalidInput,
            "path_not_absolute" => DuplicatesErrorCode::PathNotAbsolute,
            "invalid_path" => DuplicatesErrorCode::InvalidPath,
            "not_found" => DuplicatesErrorCode::NotFound,
            "permission_denied" => DuplicatesErrorCode::PermissionDenied,
            "task_failed" => DuplicatesErrorCode::TaskFailed,
            _ => DuplicatesErrorCode::UnknownError,
        };
        Self::new(code, error.to_string())
    }
}

pub(super) type DuplicatesResult<T> = Result<T, DuplicatesError>;

pub(super) fn map_api_result<T>(result: DuplicatesResult<T>) -> ApiResult<T> {
    domain::map_api_result(result)
}

const DUPLICATES_CLASSIFICATION_RULES: &[(DuplicatesErrorCode, &[&str])] = &[
    (
        DuplicatesErrorCode::TaskFailed,
        &["duplicate scan task panicked"],
    ),
    (
        DuplicatesErrorCode::PathNotAbsolute,
        COMMON_PATH_NOT_ABSOLUTE_PATTERNS,
    ),
    (
        DuplicatesErrorCode::InvalidPath,
        COMMON_INVALID_PATH_PATTERNS,
    ),
    (
        DuplicatesErrorCode::InvalidInput,
        &[
            "progress_event is required",
            "target must be a file",
            "target must be a regular file",
            "start path must be a directory",
        ],
    ),
    (
        DuplicatesErrorCode::PermissionDenied,
        COMMON_PERMISSION_DENIED_PATTERNS,
    ),
    (
        DuplicatesErrorCode::ScanLimitExceeded,
        &[
            "candidate file limit exceeded",
            "scanned file limit exceeded",
        ],
    ),
];

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
}
