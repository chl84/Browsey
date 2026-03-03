use crate::errors::{
    api_error::ApiResult,
    domain::{self, DomainError, ErrorCode},
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

impl From<crate::tasks::TaskError> for StatusbarError {
    fn from(error: crate::tasks::TaskError) -> Self {
        let code = match error.code() {
            crate::tasks::TaskErrorCode::RegistryLockFailed => {
                StatusbarErrorCode::CancelRegistryFailed
            }
            crate::tasks::TaskErrorCode::TaskNotFound => StatusbarErrorCode::UnknownError,
        };
        Self::new(code, error.to_string())
    }
}

pub type StatusbarResult<T> = Result<T, StatusbarError>;

pub fn map_api_result<T>(result: StatusbarResult<T>) -> ApiResult<T> {
    domain::map_api_result(result)
}

#[cfg(test)]
mod tests {
    use super::StatusbarError;
    use crate::errors::domain::DomainError;
    use crate::tasks::{TaskError, TaskErrorCode};

    #[test]
    fn maps_task_registry_lock_failed_to_cancel_registry_failed() {
        let task_error = TaskError::new(TaskErrorCode::RegistryLockFailed, "lock failed");
        let error = StatusbarError::from(task_error);
        assert_eq!(error.code_str(), "cancel_registry_failed");
    }
}
