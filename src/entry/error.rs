use crate::errors::domain::{classify_io_error, DomainError, ErrorCode, IoErrorHint};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryErrorCode {
    NotFound,
    PermissionDenied,
    MetadataReadFailed,
}

impl ErrorCode for EntryErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::PermissionDenied => "permission_denied",
            Self::MetadataReadFailed => "metadata_read_failed",
        }
    }
}

#[derive(Debug, Clone)]
pub struct EntryError {
    code: EntryErrorCode,
    message: String,
}

impl EntryError {
    pub fn new(code: EntryErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn from_io_error(context: &str, error: std::io::Error) -> Self {
        let code = match classify_io_error(&error) {
            IoErrorHint::NotFound => EntryErrorCode::NotFound,
            IoErrorHint::PermissionDenied => EntryErrorCode::PermissionDenied,
            _ => EntryErrorCode::MetadataReadFailed,
        };
        Self::new(code, format!("{context}: {error}"))
    }
}

impl fmt::Display for EntryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for EntryError {}

impl DomainError for EntryError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

pub type EntryResult<T> = Result<T, EntryError>;
