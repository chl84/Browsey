use crate::errors::{
    api_error::ApiResult,
    domain::{
        self, classify_io_hint_from_message, classify_message_by_patterns, DomainError, ErrorCode,
        IoErrorHint,
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
        &["path must be absolute"],
    ),
    (
        DuplicatesErrorCode::InvalidPath,
        &[
            "parent directory components are not allowed",
            "invalid path component (nul byte)",
            "path contains nul byte",
            "unsupported path prefix",
        ],
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
        &[
            "permission denied",
            "operation not permitted",
            "access is denied",
        ],
    ),
    (
        DuplicatesErrorCode::ScanLimitExceeded,
        &[
            "candidate file limit exceeded",
            "scanned file limit exceeded",
        ],
    ),
];
