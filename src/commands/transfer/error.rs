use crate::errors::{
    api_error::{ApiError, ApiResult},
    domain::{self, DomainError, ErrorCode},
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TransferErrorCode {
    InvalidInput,
    InvalidPath,
    NotFound,
    IoError,
    Timeout,
    NetworkError,
    TlsCertificateError,
    RateLimited,
    AuthRequired,
    PermissionDenied,
    InvalidConfig,
    DestinationExists,
    Unsupported,
    BinaryMissing,
    SymlinkUnsupported,
    Cancelled,
    TaskFailed,
    UnknownError,
}

impl ErrorCode for TransferErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::InvalidPath => "invalid_path",
            Self::NotFound => "not_found",
            Self::IoError => "io_error",
            Self::Timeout => "timeout",
            Self::NetworkError => "network_error",
            Self::TlsCertificateError => "tls_certificate_error",
            Self::RateLimited => "rate_limited",
            Self::AuthRequired => "auth_required",
            Self::PermissionDenied => "permission_denied",
            Self::InvalidConfig => "invalid_config",
            Self::DestinationExists => "destination_exists",
            Self::Unsupported => "unsupported",
            Self::BinaryMissing => "binary_missing",
            Self::SymlinkUnsupported => "symlink_unsupported",
            Self::Cancelled => "cancelled",
            Self::TaskFailed => "task_failed",
            Self::UnknownError => "unknown_error",
        }
    }
}

impl TransferErrorCode {
    fn from_code_str(code: &str) -> Self {
        match code {
            "invalid_input" => Self::InvalidInput,
            "invalid_path" => Self::InvalidPath,
            "not_found" => Self::NotFound,
            "io_error" => Self::IoError,
            "timeout" => Self::Timeout,
            "network_error" => Self::NetworkError,
            "tls_certificate_error" => Self::TlsCertificateError,
            "rate_limited" => Self::RateLimited,
            "auth_required" => Self::AuthRequired,
            "permission_denied" => Self::PermissionDenied,
            "invalid_config" => Self::InvalidConfig,
            "destination_exists" => Self::DestinationExists,
            "unsupported" => Self::Unsupported,
            "binary_missing" => Self::BinaryMissing,
            "symlink_unsupported" => Self::SymlinkUnsupported,
            "cancelled" => Self::Cancelled,
            "task_failed" => Self::TaskFailed,
            _ => Self::UnknownError,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct TransferError {
    code: TransferErrorCode,
    message: String,
}

impl TransferError {
    pub(super) fn new(code: TransferErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub(super) fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    pub(super) fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for TransferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for TransferError {}

impl DomainError for TransferError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

impl From<ApiError> for TransferError {
    fn from(error: ApiError) -> Self {
        Self::new(TransferErrorCode::from_code_str(&error.code), error.message)
    }
}

impl From<crate::commands::cloud::CloudCommandError> for TransferError {
    fn from(error: crate::commands::cloud::CloudCommandError) -> Self {
        let code = match error.code() {
            crate::commands::cloud::CloudCommandErrorCode::InvalidPath => {
                TransferErrorCode::InvalidPath
            }
            crate::commands::cloud::CloudCommandErrorCode::NotFound => TransferErrorCode::NotFound,
            crate::commands::cloud::CloudCommandErrorCode::Timeout => TransferErrorCode::Timeout,
            crate::commands::cloud::CloudCommandErrorCode::NetworkError => {
                TransferErrorCode::NetworkError
            }
            crate::commands::cloud::CloudCommandErrorCode::TlsCertificateError => {
                TransferErrorCode::TlsCertificateError
            }
            crate::commands::cloud::CloudCommandErrorCode::RateLimited => {
                TransferErrorCode::RateLimited
            }
            crate::commands::cloud::CloudCommandErrorCode::AuthRequired => {
                TransferErrorCode::AuthRequired
            }
            crate::commands::cloud::CloudCommandErrorCode::PermissionDenied => {
                TransferErrorCode::PermissionDenied
            }
            crate::commands::cloud::CloudCommandErrorCode::DestinationExists => {
                TransferErrorCode::DestinationExists
            }
            crate::commands::cloud::CloudCommandErrorCode::Unsupported => {
                TransferErrorCode::Unsupported
            }
            crate::commands::cloud::CloudCommandErrorCode::BinaryMissing => {
                TransferErrorCode::BinaryMissing
            }
            crate::commands::cloud::CloudCommandErrorCode::InvalidConfig => {
                TransferErrorCode::InvalidConfig
            }
            crate::commands::cloud::CloudCommandErrorCode::TaskFailed => {
                TransferErrorCode::TaskFailed
            }
            crate::commands::cloud::CloudCommandErrorCode::UnknownError => {
                TransferErrorCode::UnknownError
            }
        };
        Self::new(code, error.to_string())
    }
}

pub(super) type TransferResult<T> = Result<T, TransferError>;

pub(super) fn map_api_result<T>(result: TransferResult<T>) -> ApiResult<T> {
    domain::map_api_result(result)
}

pub(super) fn transfer_err(code: TransferErrorCode, message: impl Into<String>) -> TransferError {
    TransferError::new(code, message)
}

pub(super) fn transfer_err_code(code: &str, message: impl Into<String>) -> TransferError {
    TransferError::new(TransferErrorCode::from_code_str(code), message)
}

#[cfg(test)]
mod tests {
    use super::TransferError;
    use crate::commands::cloud::{CloudCommandError, CloudCommandErrorCode};

    #[test]
    fn maps_cloud_error_code_to_transfer_error_code() {
        let cloud = CloudCommandError::new(CloudCommandErrorCode::RateLimited, "slow down");
        let transfer: TransferError = cloud.into();
        assert_eq!(transfer.code_str(), "rate_limited");
        assert_eq!(transfer.message(), "slow down");
    }

    #[test]
    fn maps_cloud_unknown_to_transfer_unknown() {
        let cloud = CloudCommandError::new(CloudCommandErrorCode::UnknownError, "mystery");
        let transfer: TransferError = cloud.into();
        assert_eq!(transfer.code_str(), "unknown_error");
        assert_eq!(transfer.message(), "mystery");
    }
}
