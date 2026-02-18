use crate::errors::{
    api_error::ApiResult,
    domain::{self, DomainError, ErrorCode},
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SearchErrorCode {
    InvalidInput,
}

impl ErrorCode for SearchErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
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

pub(super) fn map_api_result<T>(result: Result<T, SearchError>) -> ApiResult<T> {
    domain::map_api_result(result)
}
