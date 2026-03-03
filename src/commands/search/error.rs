use crate::errors::{
    api_error::ApiResult,
    domain::{self, DomainError, ErrorCode},
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SearchErrorCode {
    InvalidInput,
    InvalidQuery,
    InvalidPath,
    NotFound,
    DatabaseOpenFailed,
    DatabaseReadFailed,
    TaskFailed,
    UnknownError,
}

impl ErrorCode for SearchErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::InvalidQuery => "invalid_query",
            Self::InvalidPath => "invalid_path",
            Self::NotFound => "not_found",
            Self::DatabaseOpenFailed => "database_open_failed",
            Self::DatabaseReadFailed => "database_read_failed",
            Self::TaskFailed => "task_failed",
            Self::UnknownError => "unknown_error",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct SearchError {
    code: SearchErrorCode,
    message: String,
}

impl SearchError {
    pub(super) fn new(code: SearchErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub(super) fn code_str_value(&self) -> &'static str {
        self.code.as_code_str()
    }
}

impl fmt::Display for SearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for SearchError {}

impl DomainError for SearchError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

impl From<crate::commands::fs::FsError> for SearchError {
    fn from(error: crate::commands::fs::FsError) -> Self {
        let code = match error.code_str() {
            "invalid_input" => SearchErrorCode::InvalidInput,
            "invalid_path" | "path_not_absolute" => SearchErrorCode::InvalidPath,
            "not_found" => SearchErrorCode::NotFound,
            "task_failed" => SearchErrorCode::TaskFailed,
            _ => SearchErrorCode::UnknownError,
        };
        Self::new(code, error.to_string())
    }
}

pub(super) type SearchResult<T> = Result<T, SearchError>;

pub(super) fn map_api_result<T>(result: SearchResult<T>) -> ApiResult<T> {
    domain::map_api_result(result)
}

#[cfg(test)]
mod tests {
    use super::SearchError;
    use crate::errors::domain::DomainError;

    #[test]
    fn maps_fs_error_invalid_input_to_search_invalid_input() {
        let fs_error: crate::commands::fs::FsError = "No paths provided".into();
        let error = SearchError::from(fs_error);
        assert_eq!(error.code_str(), "invalid_input");
    }

    #[test]
    fn maps_fs_error_path_not_absolute_to_search_invalid_path() {
        let fs_error: crate::commands::fs::FsError = "path must be absolute".into();
        let error = SearchError::from(fs_error);
        assert_eq!(error.code_str(), "invalid_path");
    }
}
