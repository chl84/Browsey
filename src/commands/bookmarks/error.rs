use crate::errors::{
    api_error::ApiResult,
    domain::{self, DomainError, ErrorCode},
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BookmarkErrorCode {
    DatabaseOpenFailed,
    BookmarksReadFailed,
    BookmarksWriteFailed,
}

impl ErrorCode for BookmarkErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::DatabaseOpenFailed => "database_open_failed",
            Self::BookmarksReadFailed => "bookmarks_read_failed",
            Self::BookmarksWriteFailed => "bookmarks_write_failed",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct BookmarkError {
    code: BookmarkErrorCode,
    message: String,
}

impl BookmarkError {
    pub(super) fn new(code: BookmarkErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for BookmarkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for BookmarkError {}

impl DomainError for BookmarkError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

pub(super) type BookmarkResult<T> = Result<T, BookmarkError>;

pub(super) fn map_api_result<T>(result: BookmarkResult<T>) -> ApiResult<T> {
    domain::map_api_result(result)
}
