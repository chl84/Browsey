use crate::errors::{
    api_error::ApiResult,
    domain::{self, DomainError, ErrorCode},
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskErrorCode {
    RegistryLockFailed,
    TaskNotFound,
}

impl ErrorCode for TaskErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::RegistryLockFailed => "registry_lock_failed",
            Self::TaskNotFound => "task_not_found",
        }
    }
}

#[derive(Debug, Clone)]
pub struct TaskError {
    code: TaskErrorCode,
    message: String,
}

impl TaskError {
    pub fn new(code: TaskErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> TaskErrorCode {
        self.code
    }
}

impl fmt::Display for TaskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for TaskError {}

impl DomainError for TaskError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

pub type TaskResult<T> = Result<T, TaskError>;

pub fn map_api_result<T>(result: TaskResult<T>) -> ApiResult<T> {
    domain::map_api_result(result)
}
