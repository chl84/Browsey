use crate::errors::{
    api_error::ApiResult,
    domain::{
        self, classify_io_hint_from_message, classify_message_by_patterns, DomainError, ErrorCode,
        IoErrorHint,
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
        &["path must be absolute"],
    ),
    (
        ListingErrorCode::InvalidPath,
        &[
            "parent directory components are not allowed",
            "invalid path component (nul byte)",
            "path contains nul byte",
            "unsupported path prefix",
        ],
    ),
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
        &[
            "permission denied",
            "operation not permitted",
            "access is denied",
        ],
    ),
    (
        ListingErrorCode::NotFound,
        &["no such file or directory", "start directory not found"],
    ),
];
