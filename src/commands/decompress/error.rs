use crate::errors::{
    api_error::ApiResult,
    domain::{
        self, classify_io_hint_from_message, classify_message_by_patterns, DomainError, ErrorCode,
        IoErrorHint, COMMON_INVALID_PATH_PATTERNS, COMMON_PATH_NOT_ABSOLUTE_PATTERNS,
        COMMON_PERMISSION_DENIED_PATTERNS,
    },
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DecompressErrorCode {
    InvalidInput,
    PathNotAbsolute,
    InvalidPath,
    RootForbidden,
    SymlinkUnsupported,
    NotFound,
    PermissionDenied,
    ReadOnlyFilesystem,
    DiskSpaceExceeded,
    ArchiveTooLarge,
    UnsupportedArchive,
    Cancelled,
    ExtractFailed,
    TaskFailed,
    UnknownError,
}

impl ErrorCode for DecompressErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::PathNotAbsolute => "path_not_absolute",
            Self::InvalidPath => "invalid_path",
            Self::RootForbidden => "root_forbidden",
            Self::SymlinkUnsupported => "symlink_unsupported",
            Self::NotFound => "not_found",
            Self::PermissionDenied => "permission_denied",
            Self::ReadOnlyFilesystem => "read_only_filesystem",
            Self::DiskSpaceExceeded => "disk_space_exceeded",
            Self::ArchiveTooLarge => "archive_too_large",
            Self::UnsupportedArchive => "unsupported_archive",
            Self::Cancelled => "cancelled",
            Self::ExtractFailed => "extract_failed",
            Self::TaskFailed => "task_failed",
            Self::UnknownError => "unknown_error",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct DecompressError {
    code: DecompressErrorCode,
    message: String,
}

impl DecompressError {
    pub(super) fn new(code: DecompressErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub(super) fn from_external_message(message: impl Into<String>) -> Self {
        let message = message.into();
        let code = classify_external_code(&message);
        Self::new(code, message)
    }
}

impl fmt::Display for DecompressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for DecompressError {}

impl DomainError for DecompressError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

pub(super) type DecompressResult<T> = Result<T, DecompressError>;

pub(super) fn map_api_result<T>(result: DecompressResult<T>) -> ApiResult<T> {
    domain::map_api_result(result)
}

impl From<String> for DecompressError {
    fn from(message: String) -> Self {
        Self::from_external_message(message)
    }
}

impl From<&str> for DecompressError {
    fn from(message: &str) -> Self {
        Self::from_external_message(message)
    }
}

impl From<crate::fs_utils::FsUtilsError> for DecompressError {
    fn from(error: crate::fs_utils::FsUtilsError) -> Self {
        let code = match error.code() {
            crate::fs_utils::FsUtilsErrorCode::InvalidPath => DecompressErrorCode::InvalidPath,
            crate::fs_utils::FsUtilsErrorCode::NotFound => DecompressErrorCode::NotFound,
            crate::fs_utils::FsUtilsErrorCode::PermissionDenied => {
                DecompressErrorCode::PermissionDenied
            }
            crate::fs_utils::FsUtilsErrorCode::ReadOnlyFilesystem => {
                DecompressErrorCode::ReadOnlyFilesystem
            }
            crate::fs_utils::FsUtilsErrorCode::RootForbidden => DecompressErrorCode::RootForbidden,
            crate::fs_utils::FsUtilsErrorCode::SymlinkUnsupported => {
                DecompressErrorCode::SymlinkUnsupported
            }
            crate::fs_utils::FsUtilsErrorCode::CanonicalizeFailed
            | crate::fs_utils::FsUtilsErrorCode::MetadataReadFailed => {
                DecompressErrorCode::ExtractFailed
            }
        };
        Self::new(code, error.to_string())
    }
}

pub(super) fn is_cancelled_error(error: &DecompressError) -> bool {
    error.code == DecompressErrorCode::Cancelled
}

fn classify_external_code(message: &str) -> DecompressErrorCode {
    if let Some(hint) = classify_io_hint_from_message(message) {
        let code = match hint {
            IoErrorHint::NotFound => Some(DecompressErrorCode::NotFound),
            IoErrorHint::PermissionDenied => Some(DecompressErrorCode::PermissionDenied),
            IoErrorHint::ReadOnlyFilesystem => Some(DecompressErrorCode::ReadOnlyFilesystem),
            _ => None,
        };
        if let Some(code) = code {
            return code;
        }
    }

    classify_message_by_patterns(
        message,
        DECOMPRESS_CLASSIFICATION_RULES,
        DecompressErrorCode::UnknownError,
    )
}

const DECOMPRESS_CLASSIFICATION_RULES: &[(DecompressErrorCode, &[&str])] = &[
    (
        DecompressErrorCode::Cancelled,
        &["extraction cancelled", "cancelled"],
    ),
    (
        DecompressErrorCode::TaskFailed,
        &["extraction task failed", "batch extraction task failed"],
    ),
    (
        DecompressErrorCode::PathNotAbsolute,
        COMMON_PATH_NOT_ABSOLUTE_PATTERNS,
    ),
    (
        DecompressErrorCode::InvalidPath,
        COMMON_INVALID_PATH_PATTERNS,
    ),
    (
        DecompressErrorCode::InvalidInput,
        &[
            "only files can be extracted",
            "cannot detect archive kind",
            "gzip too small to contain size footer",
        ],
    ),
    (
        DecompressErrorCode::RootForbidden,
        &["cannot extract archive at filesystem root"],
    ),
    (
        DecompressErrorCode::SymlinkUnsupported,
        &[
            "symlink archives are not supported",
            "symlinks are not allowed",
        ],
    ),
    (
        DecompressErrorCode::DiskSpaceExceeded,
        &["insufficient free disk space"],
    ),
    (
        DecompressErrorCode::ArchiveTooLarge,
        &[
            "archive exceeds extraction size cap",
            "extraction entry cap exceeded",
        ],
    ),
    (
        DecompressErrorCode::UnsupportedArchive,
        &["unsupported archive format"],
    ),
    (
        DecompressErrorCode::PermissionDenied,
        COMMON_PERMISSION_DENIED_PATTERNS,
    ),
    (
        DecompressErrorCode::ExtractFailed,
        &[
            "failed to",
            "write decompressed file",
            "open compressed file",
            "read archive metadata",
        ],
    ),
];

#[cfg(test)]
mod tests {
    use super::{DecompressError, DecompressErrorCode};
    use crate::fs_utils::{FsUtilsError, FsUtilsErrorCode};

    #[test]
    fn maps_fs_utils_permission_denied_without_message_reclassification() {
        let fs_error = FsUtilsError::new(
            FsUtilsErrorCode::PermissionDenied,
            "unsupported archive format",
        );
        let decompress: DecompressError = fs_error.into();
        assert_eq!(decompress.code, DecompressErrorCode::PermissionDenied);
    }
}
