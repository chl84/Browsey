use crate::errors::{
    api_error::ApiResult,
    domain::{
        self, classify_io_hint_from_message, classify_message_by_patterns, DomainError, ErrorCode,
        IoErrorHint,
    },
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OpenWithErrorCode {
    InvalidInput,
    PathNotAbsolute,
    InvalidPath,
    NotFound,
    PermissionDenied,
    DatabaseOpenFailed,
    AppNotFound,
    LaunchFailed,
    UnknownError,
}

impl ErrorCode for OpenWithErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::PathNotAbsolute => "path_not_absolute",
            Self::InvalidPath => "invalid_path",
            Self::NotFound => "not_found",
            Self::PermissionDenied => "permission_denied",
            Self::DatabaseOpenFailed => "database_open_failed",
            Self::AppNotFound => "app_not_found",
            Self::LaunchFailed => "launch_failed",
            Self::UnknownError => "unknown_error",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct OpenWithError {
    code: OpenWithErrorCode,
    message: String,
}

impl OpenWithError {
    pub(super) fn new(code: OpenWithErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub(super) fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(OpenWithErrorCode::InvalidInput, message)
    }

    pub(super) fn from_external_message(message: impl Into<String>) -> Self {
        let message = message.into();
        if let Some(hint) = classify_io_hint_from_message(&message) {
            let code = match hint {
                IoErrorHint::NotFound => Some(OpenWithErrorCode::NotFound),
                IoErrorHint::PermissionDenied => Some(OpenWithErrorCode::PermissionDenied),
                _ => None,
            };
            if let Some(code) = code {
                return Self::new(code, message);
            }
        }
        let code = classify_message_by_patterns(
            &message,
            OPEN_WITH_CLASSIFICATION_RULES,
            OpenWithErrorCode::UnknownError,
        );
        Self::new(code, message)
    }
}

impl fmt::Display for OpenWithError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for OpenWithError {}

impl DomainError for OpenWithError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

pub(super) type OpenWithResult<T> = Result<T, OpenWithError>;

pub(super) fn map_api_result<T>(result: OpenWithResult<T>) -> ApiResult<T> {
    domain::map_api_result(result)
}

const OPEN_WITH_CLASSIFICATION_RULES: &[(OpenWithErrorCode, &[&str])] = &[
    (
        OpenWithErrorCode::PathNotAbsolute,
        &["path must be absolute"],
    ),
    (
        OpenWithErrorCode::InvalidPath,
        &[
            "parent directory components are not allowed",
            "invalid path component (nul byte)",
            "path contains nul byte",
            "unsupported path prefix",
        ],
    ),
    (
        OpenWithErrorCode::InvalidInput,
        &["no application selected"],
    ),
    (
        OpenWithErrorCode::AppNotFound,
        &[
            "desktop entry not found",
            "windows app handler not found",
            "application not found",
        ],
    ),
    (
        OpenWithErrorCode::LaunchFailed,
        &["failed to launch", "failed to open", "unable to execute"],
    ),
];
