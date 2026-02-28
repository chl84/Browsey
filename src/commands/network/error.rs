use crate::errors::{
    api_error::ApiResult,
    domain::{self, DomainError, ErrorCode},
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NetworkErrorCode {
    InvalidUri,
    UnsupportedUri,
    UnsupportedScheme,
    OpenFailed,
    DiscoveryFailed,
    MountFailed,
    EjectFailed,
    TaskFailed,
}

impl ErrorCode for NetworkErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::InvalidUri => "invalid_uri",
            Self::UnsupportedUri => "unsupported_uri",
            Self::UnsupportedScheme => "unsupported_scheme",
            Self::OpenFailed => "open_failed",
            Self::DiscoveryFailed => "discovery_failed",
            Self::MountFailed => "mount_failed",
            Self::EjectFailed => "eject_failed",
            Self::TaskFailed => "task_failed",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct NetworkError {
    code: NetworkErrorCode,
    message: String,
}

impl NetworkError {
    pub(super) fn new(code: NetworkErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for NetworkError {}

impl DomainError for NetworkError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

pub(super) type NetworkResult<T> = Result<T, NetworkError>;

pub(super) fn map_api_result<T>(result: NetworkResult<T>) -> ApiResult<T> {
    domain::map_api_result(result)
}
