use crate::errors::{
    api_error::ApiResult,
    domain::{
        self, classify_io_hint_from_message, classify_message_by_patterns, DomainError, ErrorCode,
        IoErrorHint,
    },
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CompressErrorCode {
    InvalidInput,
    PathNotAbsolute,
    InvalidPath,
    RootForbidden,
    NotFound,
    PermissionDenied,
    ReadOnlyFilesystem,
    TargetExists,
    Cancelled,
    CompressionFailed,
    TaskFailed,
    UnknownError,
}

impl ErrorCode for CompressErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::PathNotAbsolute => "path_not_absolute",
            Self::InvalidPath => "invalid_path",
            Self::RootForbidden => "root_forbidden",
            Self::NotFound => "not_found",
            Self::PermissionDenied => "permission_denied",
            Self::ReadOnlyFilesystem => "read_only_filesystem",
            Self::TargetExists => "target_exists",
            Self::Cancelled => "cancelled",
            Self::CompressionFailed => "compression_failed",
            Self::TaskFailed => "task_failed",
            Self::UnknownError => "unknown_error",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct CompressError {
    code: CompressErrorCode,
    message: String,
}

impl CompressError {
    pub(super) fn new(code: CompressErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub(super) fn from_external_message(message: impl Into<String>) -> Self {
        let message = message.into();
        if let Some(hint) = classify_io_hint_from_message(&message) {
            let code = match hint {
                IoErrorHint::NotFound => Some(CompressErrorCode::NotFound),
                IoErrorHint::PermissionDenied => Some(CompressErrorCode::PermissionDenied),
                IoErrorHint::ReadOnlyFilesystem => Some(CompressErrorCode::ReadOnlyFilesystem),
                IoErrorHint::AlreadyExists => Some(CompressErrorCode::TargetExists),
                _ => None,
            };
            if let Some(code) = code {
                return Self::new(code, message);
            }
        }

        let code = classify_message_by_patterns(
            &message,
            COMPRESS_CLASSIFICATION_RULES,
            CompressErrorCode::UnknownError,
        );
        Self::new(code, message)
    }
}

impl fmt::Display for CompressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CompressError {}

impl DomainError for CompressError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

pub(super) type CompressResult<T> = Result<T, CompressError>;

pub(super) fn map_api_result<T>(result: CompressResult<T>) -> ApiResult<T> {
    domain::map_api_result(result)
}

pub(super) fn map_external_result<T>(result: Result<T, String>) -> CompressResult<T> {
    result.map_err(CompressError::from_external_message)
}

const COMPRESS_CLASSIFICATION_RULES: &[(CompressErrorCode, &[&str])] = &[
    (
        CompressErrorCode::Cancelled,
        &["compression cancelled", "cancelled"],
    ),
    (CompressErrorCode::TaskFailed, &["compression task failed"]),
    (
        CompressErrorCode::PathNotAbsolute,
        &["path must be absolute"],
    ),
    (
        CompressErrorCode::InvalidPath,
        &[
            "parent directory components are not allowed",
            "invalid path component (nul byte)",
            "path contains nul byte",
            "unsupported path prefix",
            "name cannot contain path separators",
        ],
    ),
    (
        CompressErrorCode::InvalidInput,
        &[
            "nothing to compress",
            "name cannot be empty",
            "all items must be in the same folder",
        ],
    ),
    (
        CompressErrorCode::RootForbidden,
        &[
            "cannot compress filesystem root",
            "refusing to operate on filesystem root",
        ],
    ),
    (CompressErrorCode::NotFound, &["no such file or directory"]),
    (
        CompressErrorCode::PermissionDenied,
        &[
            "permission denied",
            "operation not permitted",
            "access is denied",
        ],
    ),
    (
        CompressErrorCode::ReadOnlyFilesystem,
        &["read-only file system"],
    ),
    (CompressErrorCode::TargetExists, &["target already exists"]),
    (
        CompressErrorCode::CompressionFailed,
        &[
            "failed to create destination",
            "failed to open file",
            "failed to write file to zip",
            "failed to finalize zip",
            "failed to start zip entry",
            "failed to add",
        ],
    ),
];
