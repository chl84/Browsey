use crate::errors::{
    api_error::ApiResult,
    domain::{self, classify_message_by_patterns, DomainError, ErrorCode},
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusbarErrorCode {
    CancelRegistryFailed,
    TaskFailed,
    UnknownError,
}

impl ErrorCode for StatusbarErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::CancelRegistryFailed => "cancel_registry_failed",
            Self::TaskFailed => "task_failed",
            Self::UnknownError => "unknown_error",
        }
    }
}

#[derive(Debug, Clone)]
pub struct StatusbarError {
    code: StatusbarErrorCode,
    message: String,
}

impl StatusbarError {
    pub fn new(code: StatusbarErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn from_external_message(message: impl Into<String>) -> Self {
        let message = message.into();
        let code = classify_message_by_patterns(
            &message,
            STATUSBAR_CLASSIFICATION_RULES,
            StatusbarErrorCode::UnknownError,
        );
        Self::new(code, message)
    }
}

impl fmt::Display for StatusbarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for StatusbarError {}

impl DomainError for StatusbarError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

pub type StatusbarResult<T> = Result<T, StatusbarError>;

pub fn map_api_result<T>(result: StatusbarResult<T>) -> ApiResult<T> {
    domain::map_api_result(result)
}

const STATUSBAR_CLASSIFICATION_RULES: &[(StatusbarErrorCode, &[&str])] = &[
    (
        StatusbarErrorCode::CancelRegistryFailed,
        &["failed to lock cancel registry"],
    ),
    (
        StatusbarErrorCode::TaskFailed,
        &["failed to compute directory sizes"],
    ),
];
