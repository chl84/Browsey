use crate::errors::{
    api_error::ApiResult,
    domain::{self, classify_io_error, DomainError, ErrorCode, IoErrorHint},
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

impl From<crate::fs_utils::FsUtilsError> for ConsoleError {
    fn from(error: crate::fs_utils::FsUtilsError) -> Self {
        let code = match error.code() {
            crate::fs_utils::FsUtilsErrorCode::InvalidPath => {
                if error.to_string().contains("path must be absolute") {
                    ConsoleErrorCode::PathNotAbsolute
                } else {
                    ConsoleErrorCode::InvalidPath
                }
            }
            crate::fs_utils::FsUtilsErrorCode::NotFound => ConsoleErrorCode::NotFound,
            crate::fs_utils::FsUtilsErrorCode::PermissionDenied => {
                ConsoleErrorCode::PermissionDenied
            }
            crate::fs_utils::FsUtilsErrorCode::RootForbidden => ConsoleErrorCode::RootForbidden,
            crate::fs_utils::FsUtilsErrorCode::ReadOnlyFilesystem
            | crate::fs_utils::FsUtilsErrorCode::SymlinkUnsupported
            | crate::fs_utils::FsUtilsErrorCode::CanonicalizeFailed
            | crate::fs_utils::FsUtilsErrorCode::MetadataReadFailed => {
                ConsoleErrorCode::UnknownError
            }
        };
        Self::new(code, error.to_string())
    }
}

pub(super) type ConsoleResult<T> = Result<T, ConsoleError>;

pub(super) fn map_api_result<T>(result: ConsoleResult<T>) -> ApiResult<T> {
    domain::map_api_result(result)
}
