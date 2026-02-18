use crate::errors::{
    api_error::ApiResult,
    domain::{self, classify_message_by_patterns, DomainError, ErrorCode},
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskErrorCode {
    RegistryLockFailed,
    TaskNotFound,
    UnknownError,
}

impl ErrorCode for TaskErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::RegistryLockFailed => "registry_lock_failed",
            Self::TaskNotFound => "task_not_found",
            Self::UnknownError => "unknown_error",
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

    pub fn from_external_message(message: impl Into<String>) -> Self {
        let message = message.into();
        let code = classify_message_by_patterns(
            &message,
            TASK_CLASSIFICATION_RULES,
            TaskErrorCode::UnknownError,
        );
        Self::new(code, message)
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

impl From<TaskError> for String {
    fn from(error: TaskError) -> Self {
        error.to_string()
    }
}

pub type TaskResult<T> = Result<T, TaskError>;

pub fn map_api_result<T>(result: TaskResult<T>) -> ApiResult<T> {
    domain::map_api_result(result)
}

const TASK_CLASSIFICATION_RULES: &[(TaskErrorCode, &[&str])] = &[
    (
        TaskErrorCode::RegistryLockFailed,
        &["failed to lock cancel registry"],
    ),
    (
        TaskErrorCode::TaskNotFound,
        &["task not found or already finished"],
    ),
];
