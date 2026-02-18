use crate::errors::{
    api_error::ApiResult,
    domain::{
        self, classify_io_hint_from_message, classify_message_by_patterns, DomainError, ErrorCode,
        IoErrorHint,
    },
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ThumbnailErrorCode {
    InvalidInput,
    NotFound,
    PermissionDenied,
    DecodeFailed,
    UnsupportedFormat,
    CacheFailed,
    Cancelled,
    UnknownError,
}

impl ErrorCode for ThumbnailErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::NotFound => "not_found",
            Self::PermissionDenied => "permission_denied",
            Self::DecodeFailed => "decode_failed",
            Self::UnsupportedFormat => "unsupported_format",
            Self::CacheFailed => "cache_failed",
            Self::Cancelled => "cancelled",
            Self::UnknownError => "unknown_error",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct ThumbnailError {
    code: ThumbnailErrorCode,
    message: String,
}

impl ThumbnailError {
    pub(super) fn new(code: ThumbnailErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub(super) fn from_external_message(message: impl Into<String>) -> Self {
        let message = message.into();
        if let Some(hint) = classify_io_hint_from_message(&message) {
            let code = match hint {
                IoErrorHint::NotFound => Some(ThumbnailErrorCode::NotFound),
                IoErrorHint::PermissionDenied => Some(ThumbnailErrorCode::PermissionDenied),
                _ => None,
            };
            if let Some(code) = code {
                return Self::new(code, message);
            }
        }
        let code = classify_message_by_patterns(
            &message,
            THUMBNAIL_CLASSIFICATION_RULES,
            ThumbnailErrorCode::UnknownError,
        );
        Self::new(code, message)
    }
}

impl fmt::Display for ThumbnailError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ThumbnailError {}

impl DomainError for ThumbnailError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

pub(super) fn map_api_result<T>(result: Result<T, ThumbnailError>) -> ApiResult<T> {
    domain::map_api_result(result)
}

const THUMBNAIL_CLASSIFICATION_RULES: &[(ThumbnailErrorCode, &[&str])] = &[
    (
        ThumbnailErrorCode::Cancelled,
        &["cancelled", "semaphore closed"],
    ),
    (
        ThumbnailErrorCode::PermissionDenied,
        &["cannot read file", "permission denied", "access is denied"],
    ),
    (
        ThumbnailErrorCode::InvalidInput,
        &[
            "target is not a file",
            "file too large for thumbnail",
            "video thumbnails disabled",
        ],
    ),
    (
        ThumbnailErrorCode::CacheFailed,
        &[
            "failed to create thumbnail cache dir",
            "failed to clear thumbnail cache",
            "failed to recreate thumbnail cache dir",
            "too many concurrent thumbnails",
        ],
    ),
    (
        ThumbnailErrorCode::DecodeFailed,
        &["decode", "failed to read metadata", "failed to render"],
    ),
    (
        ThumbnailErrorCode::UnsupportedFormat,
        &["unsupported image format", "unsupported thumbnail format"],
    ),
];
