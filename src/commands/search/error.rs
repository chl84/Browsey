use crate::errors::{
    api_error::ApiResult,
    domain::{
        self, classify_io_hint_from_message, classify_message_by_patterns, DomainError, ErrorCode,
        IoErrorHint,
    },
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

    pub(super) fn from_external_message(message: impl Into<String>) -> Self {
        let message = message.into();
        if let Some(hint) = classify_io_hint_from_message(&message) {
            let code = match hint {
                IoErrorHint::NotFound => Some(SearchErrorCode::NotFound),
                _ => None,
            };
            if let Some(code) = code {
                return Self::new(code, message);
            }
        }
        let code = classify_message_by_patterns(
            &message,
            SEARCH_CLASSIFICATION_RULES,
            SearchErrorCode::UnknownError,
        );
        Self::new(code, message)
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

pub(super) type SearchResult<T> = Result<T, SearchError>;

pub(super) fn map_api_result<T>(result: SearchResult<T>) -> ApiResult<T> {
    domain::map_api_result(result)
}

const SEARCH_CLASSIFICATION_RULES: &[(SearchErrorCode, &[&str])] = &[
    (
        SearchErrorCode::InvalidInput,
        &["progress_event is required"],
    ),
    (
        SearchErrorCode::InvalidQuery,
        &[
            "invalid search query",
            "unclosed quote",
            "unclosed group",
            "unexpected token",
        ],
    ),
    (
        SearchErrorCode::InvalidPath,
        &["invalid path", "path must be absolute"],
    ),
    (
        SearchErrorCode::DatabaseOpenFailed,
        &[
            "failed to open db",
            "failed to open library database",
            "failed to open data dir",
        ],
    ),
    (
        SearchErrorCode::DatabaseReadFailed,
        &["failed to read", "failed to query", "failed to prepare"],
    ),
    (
        SearchErrorCode::TaskFailed,
        &["task failed", "channel closed", "start directory not found"],
    ),
];
