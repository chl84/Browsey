use crate::errors::domain::{classify_io_error, DomainError, ErrorCode, IoErrorHint};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsUtilsErrorCode {
    InvalidPath,
    NotFound,
    PermissionDenied,
    ReadOnlyFilesystem,
    RootForbidden,
    SymlinkUnsupported,
    CanonicalizeFailed,
    MetadataReadFailed,
}

impl ErrorCode for FsUtilsErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::InvalidPath => "invalid_path",
            Self::NotFound => "not_found",
            Self::PermissionDenied => "permission_denied",
            Self::ReadOnlyFilesystem => "read_only_filesystem",
            Self::RootForbidden => "root_forbidden",
            Self::SymlinkUnsupported => "symlink_unsupported",
            Self::CanonicalizeFailed => "canonicalize_failed",
            Self::MetadataReadFailed => "metadata_read_failed",
        }
    }
}

#[derive(Debug, Clone)]
pub struct FsUtilsError {
    code: FsUtilsErrorCode,
    message: String,
}

impl FsUtilsError {
    pub fn new(code: FsUtilsErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn from_io_error(fallback: FsUtilsErrorCode, context: &str, error: std::io::Error) -> Self {
        let code = match classify_io_error(&error) {
            IoErrorHint::NotFound => FsUtilsErrorCode::NotFound,
            IoErrorHint::PermissionDenied => FsUtilsErrorCode::PermissionDenied,
            IoErrorHint::ReadOnlyFilesystem => FsUtilsErrorCode::ReadOnlyFilesystem,
            IoErrorHint::InvalidInput => FsUtilsErrorCode::InvalidPath,
            _ => fallback,
        };
        Self::new(code, format!("{context}: {error}"))
    }
}

impl fmt::Display for FsUtilsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for FsUtilsError {}

impl DomainError for FsUtilsError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

impl From<FsUtilsError> for String {
    fn from(error: FsUtilsError) -> Self {
        error.to_string()
    }
}

pub type FsUtilsResult<T> = Result<T, FsUtilsError>;
