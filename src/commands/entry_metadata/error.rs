use crate::errors::{
    api_error::ApiResult,
    domain::{
        self, classify_io_hint_from_message, classify_message_by_patterns, DomainError, ErrorCode,
        IoErrorHint,
    },
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EntryMetadataErrorCode {
    PathNotAbsolute,
    InvalidPath,
    NotFound,
    PermissionDenied,
    MetadataReadFailed,
    UnknownError,
}

impl ErrorCode for EntryMetadataErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::PathNotAbsolute => "path_not_absolute",
            Self::InvalidPath => "invalid_path",
            Self::NotFound => "not_found",
            Self::PermissionDenied => "permission_denied",
            Self::MetadataReadFailed => "metadata_read_failed",
            Self::UnknownError => "unknown_error",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct EntryMetadataError {
    code: EntryMetadataErrorCode,
    message: String,
}

impl EntryMetadataError {
    pub(super) fn new(code: EntryMetadataErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub(super) fn from_external_message(message: impl Into<String>) -> Self {
        let message = message.into();
        if let Some(hint) = classify_io_hint_from_message(&message) {
            let code = match hint {
                IoErrorHint::NotFound => Some(EntryMetadataErrorCode::NotFound),
                IoErrorHint::PermissionDenied => Some(EntryMetadataErrorCode::PermissionDenied),
                _ => None,
            };
            if let Some(code) = code {
                return Self::new(code, message);
            }
        }
        let code = classify_message_by_patterns(
            &message,
            ENTRY_METADATA_CLASSIFICATION_RULES,
            EntryMetadataErrorCode::UnknownError,
        );
        Self::new(code, message)
    }
}

impl fmt::Display for EntryMetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for EntryMetadataError {}

impl DomainError for EntryMetadataError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

pub(super) type EntryMetadataResult<T> = Result<T, EntryMetadataError>;

pub(super) fn map_api_result<T>(result: EntryMetadataResult<T>) -> ApiResult<T> {
    domain::map_api_result(result)
}

const ENTRY_METADATA_CLASSIFICATION_RULES: &[(EntryMetadataErrorCode, &[&str])] = &[
    (
        EntryMetadataErrorCode::PathNotAbsolute,
        &["path must be absolute"],
    ),
    (
        EntryMetadataErrorCode::InvalidPath,
        &[
            "parent directory components are not allowed",
            "invalid path component (nul byte)",
            "path contains nul byte",
            "unsupported path prefix",
        ],
    ),
    (
        EntryMetadataErrorCode::NotFound,
        &["no such file or directory", "not found"],
    ),
    (
        EntryMetadataErrorCode::PermissionDenied,
        &[
            "permission denied",
            "operation not permitted",
            "access is denied",
        ],
    ),
    (
        EntryMetadataErrorCode::MetadataReadFailed,
        &["failed to read metadata"],
    ),
];
