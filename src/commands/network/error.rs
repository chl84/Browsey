use crate::errors::{
    api_error::ApiResult,
    domain::{
        self, classify_io_hint_from_message, classify_message_by_patterns, DomainError, ErrorCode,
        IoErrorHint,
    },
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NetworkErrorCode {
    InvalidUri,
    UnsupportedUri,
    UnsupportedScheme,
    NotFound,
    PermissionDenied,
    OpenFailed,
    DiscoveryFailed,
    MountFailed,
    EjectFailed,
    TaskFailed,
    UnknownError,
}

impl ErrorCode for NetworkErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::InvalidUri => "invalid_uri",
            Self::UnsupportedUri => "unsupported_uri",
            Self::UnsupportedScheme => "unsupported_scheme",
            Self::NotFound => "not_found",
            Self::PermissionDenied => "permission_denied",
            Self::OpenFailed => "open_failed",
            Self::DiscoveryFailed => "discovery_failed",
            Self::MountFailed => "mount_failed",
            Self::EjectFailed => "eject_failed",
            Self::TaskFailed => "task_failed",
            Self::UnknownError => "unknown_error",
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

    pub(super) fn from_external_message(message: impl Into<String>) -> Self {
        let message = message.into();
        if let Some(hint) = classify_io_hint_from_message(&message) {
            let code = match hint {
                IoErrorHint::NotFound => Some(NetworkErrorCode::NotFound),
                IoErrorHint::PermissionDenied => Some(NetworkErrorCode::PermissionDenied),
                _ => None,
            };
            if let Some(code) = code {
                return Self::new(code, message);
            }
        }
        let code = classify_message_by_patterns(
            &message,
            NETWORK_CLASSIFICATION_RULES,
            NetworkErrorCode::UnknownError,
        );
        Self::new(code, message)
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

const NETWORK_CLASSIFICATION_RULES: &[(NetworkErrorCode, &[&str])] = &[
    (
        NetworkErrorCode::TaskFailed,
        &["task failed", "task panicked"],
    ),
    (
        NetworkErrorCode::UnsupportedUri,
        &["unsupported uri", "network mounts are not supported"],
    ),
    (
        NetworkErrorCode::UnsupportedScheme,
        &["only http/https uris are supported"],
    ),
    (NetworkErrorCode::InvalidUri, &["invalid uri"]),
    (
        NetworkErrorCode::OpenFailed,
        &["failed to open uri", "failed to open"],
    ),
    (
        NetworkErrorCode::DiscoveryFailed,
        &["network discovery failed", "network listing failed"],
    ),
    (NetworkErrorCode::MountFailed, &["failed to mount"]),
    (
        NetworkErrorCode::EjectFailed,
        &["eject failed", "volume is in use"],
    ),
];
