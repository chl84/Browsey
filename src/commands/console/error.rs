use crate::errors::{
    api_error::ApiResult,
    domain::{
        self, classify_io_error, classify_io_hint_from_message, classify_message_by_patterns,
        DomainError, ErrorCode, IoErrorHint,
    },
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConsoleErrorCode {
    PathNotAbsolute,
    InvalidPath,
    RootForbidden,
    NotDirectory,
    NotFound,
    PermissionDenied,
    TerminalUnavailable,
    LaunchFailed,
    UnsupportedPlatform,
    UnknownError,
}

impl ErrorCode for ConsoleErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::PathNotAbsolute => "path_not_absolute",
            Self::InvalidPath => "invalid_path",
            Self::RootForbidden => "root_forbidden",
            Self::NotDirectory => "not_directory",
            Self::NotFound => "not_found",
            Self::PermissionDenied => "permission_denied",
            Self::TerminalUnavailable => "terminal_unavailable",
            Self::LaunchFailed => "launch_failed",
            Self::UnsupportedPlatform => "unsupported_platform",
            Self::UnknownError => "unknown_error",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct ConsoleError {
    code: ConsoleErrorCode,
    message: String,
}

impl ConsoleError {
    pub(super) fn new(code: ConsoleErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub(super) fn from_external_message(message: impl Into<String>) -> Self {
        let message = message.into();
        if let Some(hint) = classify_io_hint_from_message(&message) {
            let code = match hint {
                IoErrorHint::NotFound => Some(ConsoleErrorCode::NotFound),
                IoErrorHint::PermissionDenied => Some(ConsoleErrorCode::PermissionDenied),
                _ => None,
            };
            if let Some(code) = code {
                return Self::new(code, message);
            }
        }
        let code = classify_message_by_patterns(
            &message,
            CONSOLE_CLASSIFICATION_RULES,
            ConsoleErrorCode::UnknownError,
        );
        Self::new(code, message)
    }

    pub(super) fn from_io_error(
        fallback: ConsoleErrorCode,
        context: &str,
        error: std::io::Error,
    ) -> Self {
        let code = match classify_io_error(&error) {
            IoErrorHint::NotFound => ConsoleErrorCode::NotFound,
            IoErrorHint::PermissionDenied => ConsoleErrorCode::PermissionDenied,
            _ => fallback,
        };
        Self::new(code, format!("{context}: {error}"))
    }
}

impl fmt::Display for ConsoleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ConsoleError {}

impl DomainError for ConsoleError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

pub(super) type ConsoleResult<T> = Result<T, ConsoleError>;

pub(super) fn map_api_result<T>(result: ConsoleResult<T>) -> ApiResult<T> {
    domain::map_api_result(result)
}

const CONSOLE_CLASSIFICATION_RULES: &[(ConsoleErrorCode, &[&str])] = &[
    (
        ConsoleErrorCode::PathNotAbsolute,
        &["path must be absolute"],
    ),
    (
        ConsoleErrorCode::InvalidPath,
        &[
            "parent directory components are not allowed",
            "invalid path component (nul byte)",
            "path contains nul byte",
            "unsupported path prefix",
        ],
    ),
    (
        ConsoleErrorCode::RootForbidden,
        &["refusing to operate on filesystem root"],
    ),
    (
        ConsoleErrorCode::NotDirectory,
        &["can only open console in a directory"],
    ),
    (
        ConsoleErrorCode::PermissionDenied,
        &[
            "permission denied",
            "operation not permitted",
            "access is denied",
        ],
    ),
    (
        ConsoleErrorCode::TerminalUnavailable,
        &["could not find a supported terminal emulator"],
    ),
    (
        ConsoleErrorCode::UnsupportedPlatform,
        &["unsupported platform for opening console"],
    ),
];
