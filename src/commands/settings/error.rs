use crate::errors::{
    api_error::ApiResult,
    domain::{self, DomainError, ErrorCode},
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SettingsErrorCode {
    InvalidInput,
    PermissionDenied,
    ReadOnlyFilesystem,
    DbOpenFailed,
    DbReadFailed,
    DbWriteFailed,
    SerializeFailed,
    ParseFailed,
    UnknownError,
}

impl ErrorCode for SettingsErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::PermissionDenied => "permission_denied",
            Self::ReadOnlyFilesystem => "read_only_filesystem",
            Self::DbOpenFailed => "db_open_failed",
            Self::DbReadFailed => "db_read_failed",
            Self::DbWriteFailed => "db_write_failed",
            Self::SerializeFailed => "serialize_failed",
            Self::ParseFailed => "parse_failed",
            Self::UnknownError => "unknown_error",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct SettingsError {
    code: SettingsErrorCode,
    message: String,
}

impl SettingsError {
    pub(super) fn new(code: SettingsErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub(super) fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(SettingsErrorCode::InvalidInput, message)
    }
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for SettingsError {}

impl DomainError for SettingsError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

pub(super) type SettingsResult<T> = Result<T, SettingsError>;

pub(super) fn map_api_result<T>(result: SettingsResult<T>) -> ApiResult<T> {
    domain::map_api_result(result)
}
