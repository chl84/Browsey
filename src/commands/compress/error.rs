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

impl From<crate::fs_utils::FsUtilsError> for CompressError {
    fn from(error: crate::fs_utils::FsUtilsError) -> Self {
        let code = match error.code() {
            crate::fs_utils::FsUtilsErrorCode::InvalidPath => CompressErrorCode::InvalidPath,
            crate::fs_utils::FsUtilsErrorCode::RootForbidden => CompressErrorCode::RootForbidden,
            crate::fs_utils::FsUtilsErrorCode::NotFound => CompressErrorCode::NotFound,
            crate::fs_utils::FsUtilsErrorCode::PermissionDenied => {
                CompressErrorCode::PermissionDenied
            }
            crate::fs_utils::FsUtilsErrorCode::ReadOnlyFilesystem => {
                CompressErrorCode::ReadOnlyFilesystem
            }
            crate::fs_utils::FsUtilsErrorCode::SymlinkUnsupported => CompressErrorCode::InvalidPath,
            crate::fs_utils::FsUtilsErrorCode::CanonicalizeFailed
            | crate::fs_utils::FsUtilsErrorCode::MetadataReadFailed => {
                CompressErrorCode::UnknownError
            }
        };
        Self::new(code, error.to_string())
    }
}

pub(super) type CompressResult<T> = Result<T, CompressError>;

pub(super) fn map_api_result<T>(result: CompressResult<T>) -> ApiResult<T> {
    domain::map_api_result(result)
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

#[cfg(test)]
mod tests {
    use super::CompressError;
    use crate::errors::domain::DomainError;
    use crate::fs_utils::{FsUtilsError, FsUtilsErrorCode};

    #[test]
    fn maps_fs_utils_read_only_to_compress_read_only_filesystem() {
        let fs_error = FsUtilsError::new(FsUtilsErrorCode::ReadOnlyFilesystem, "readonly");
        let error = CompressError::from(fs_error);
        assert_eq!(error.code_str(), "read_only_filesystem");
    }

    #[test]
    fn maps_fs_utils_root_forbidden_to_compress_root_forbidden() {
        let fs_error = FsUtilsError::new(FsUtilsErrorCode::RootForbidden, "root");
        let error = CompressError::from(fs_error);
        assert_eq!(error.code_str(), "root_forbidden");
    }
}
