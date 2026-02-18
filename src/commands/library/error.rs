use crate::errors::{
    api_error::ApiResult,
    domain::{self, DomainError, ErrorCode},
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LibraryErrorCode {
    DatabaseOpenFailed,
    ToggleStarFailed,
    ListFailed,
    DeleteFailed,
}

impl ErrorCode for LibraryErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::DatabaseOpenFailed => "database_open_failed",
            Self::ToggleStarFailed => "toggle_star_failed",
            Self::ListFailed => "list_failed",
            Self::DeleteFailed => "delete_failed",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct LibraryError {
    code: LibraryErrorCode,
    message: String,
}

impl LibraryError {
    pub(super) fn new(code: LibraryErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for LibraryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for LibraryError {}

impl DomainError for LibraryError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

pub(super) type LibraryResult<T> = Result<T, LibraryError>;

pub(super) fn map_api_result<T>(result: LibraryResult<T>) -> ApiResult<T> {
    domain::map_api_result(result)
}
