use crate::errors::{
    api_error::ApiResult,
    domain::{self, DomainError, ErrorCode},
};
use std::fmt;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CloudCommandErrorCode {
    InvalidPath,
    NotFound,
    Timeout,
    NetworkError,
    TlsCertificateError,
    RateLimited,
    AuthRequired,
    PermissionDenied,
    DestinationExists,
    Unsupported,
    BinaryMissing,
    InvalidConfig,
    TaskFailed,
    UnknownError,
}

impl ErrorCode for CloudCommandErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::InvalidPath => "invalid_path",
            Self::NotFound => "not_found",
            Self::Timeout => "timeout",
            Self::NetworkError => "network_error",
            Self::TlsCertificateError => "tls_certificate_error",
            Self::RateLimited => "rate_limited",
            Self::AuthRequired => "auth_required",
            Self::PermissionDenied => "permission_denied",
            Self::DestinationExists => "destination_exists",
            Self::Unsupported => "unsupported",
            Self::BinaryMissing => "binary_missing",
            Self::InvalidConfig => "invalid_config",
            Self::TaskFailed => "task_failed",
            Self::UnknownError => "unknown_error",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CloudCommandError {
    code: CloudCommandErrorCode,
    message: String,
}

impl CloudCommandError {
    pub(crate) fn new(code: CloudCommandErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub(crate) fn code(&self) -> CloudCommandErrorCode {
        self.code
    }
}

impl fmt::Display for CloudCommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CloudCommandError {}

impl DomainError for CloudCommandError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

pub(crate) type CloudCommandResult<T> = Result<T, CloudCommandError>;

pub(super) fn map_api_result<T>(result: CloudCommandResult<T>) -> ApiResult<T> {
    domain::map_api_result(result)
}
