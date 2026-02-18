use crate::errors::domain::{classify_io_error, DomainError, ErrorCode, IoErrorHint};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathGuardErrorCode {
    NotFound,
    PermissionDenied,
    NotDirectory,
    SymlinkUnsupported,
    MetadataReadFailed,
}

impl ErrorCode for PathGuardErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::PermissionDenied => "permission_denied",
            Self::NotDirectory => "not_directory",
            Self::SymlinkUnsupported => "symlink_unsupported",
            Self::MetadataReadFailed => "metadata_read_failed",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PathGuardError {
    code: PathGuardErrorCode,
    message: String,
}

impl PathGuardError {
    pub fn new(code: PathGuardErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn from_io_error(context: &str, error: std::io::Error) -> Self {
        let code = match classify_io_error(&error) {
            IoErrorHint::NotFound => PathGuardErrorCode::NotFound,
            IoErrorHint::PermissionDenied => PathGuardErrorCode::PermissionDenied,
            _ => PathGuardErrorCode::MetadataReadFailed,
        };
        Self::new(code, format!("{context}: {error}"))
    }
}

impl fmt::Display for PathGuardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for PathGuardError {}

impl DomainError for PathGuardError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

impl From<PathGuardError> for String {
    fn from(error: PathGuardError) -> Self {
        error.to_string()
    }
}

pub type PathGuardResult<T> = Result<T, PathGuardError>;
