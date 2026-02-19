use crate::errors::{
    api_error::ApiResult,
    domain::{
        self, classify_io_hint_from_message, classify_message_by_patterns, DomainError, ErrorCode,
        IoErrorHint,
    },
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UndoErrorCode {
    InvalidInput,
    NotFound,
    PermissionDenied,
    ReadOnlyFilesystem,
    TargetExists,
    SymlinkUnsupported,
    SnapshotMismatch,
    UndoUnavailable,
    RedoUnavailable,
    LockFailed,
    IoError,
    UnknownError,
}

impl ErrorCode for UndoErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::NotFound => "not_found",
            Self::PermissionDenied => "permission_denied",
            Self::ReadOnlyFilesystem => "read_only_filesystem",
            Self::TargetExists => "target_exists",
            Self::SymlinkUnsupported => "symlink_unsupported",
            Self::SnapshotMismatch => "snapshot_mismatch",
            Self::UndoUnavailable => "undo_unavailable",
            Self::RedoUnavailable => "redo_unavailable",
            Self::LockFailed => "lock_failed",
            Self::IoError => "io_error",
            Self::UnknownError => "unknown_error",
        }
    }
}

#[derive(Debug, Clone)]
pub struct UndoError {
    code: UndoErrorCode,
    message: String,
}

impl UndoError {
    pub fn new(code: UndoErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn from_external_message(message: impl Into<String>) -> Self {
        let message = message.into();
        if let Some(hint) = classify_io_hint_from_message(&message) {
            let code = match hint {
                IoErrorHint::NotFound => Some(UndoErrorCode::NotFound),
                IoErrorHint::PermissionDenied => Some(UndoErrorCode::PermissionDenied),
                IoErrorHint::ReadOnlyFilesystem => Some(UndoErrorCode::ReadOnlyFilesystem),
                IoErrorHint::AlreadyExists => Some(UndoErrorCode::TargetExists),
                _ => None,
            };
            if let Some(code) = code {
                return Self::new(code, message);
            }
        }
        let code = classify_message_by_patterns(
            &message,
            UNDO_CLASSIFICATION_RULES,
            UndoErrorCode::UnknownError,
        );
        Self::new(code, message)
    }
}

impl fmt::Display for UndoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for UndoError {}

impl DomainError for UndoError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

impl From<String> for UndoError {
    fn from(message: String) -> Self {
        Self::from_external_message(message)
    }
}

impl From<&str> for UndoError {
    fn from(message: &str) -> Self {
        Self::from_external_message(message)
    }
}

impl From<crate::fs_utils::FsUtilsError> for UndoError {
    fn from(error: crate::fs_utils::FsUtilsError) -> Self {
        Self::from_external_message(error.to_string())
    }
}

impl From<UndoError> for String {
    fn from(error: UndoError) -> Self {
        error.to_string()
    }
}

pub type UndoResult<T> = Result<T, UndoError>;

pub fn map_api_result<T>(result: UndoResult<T>) -> ApiResult<T> {
    domain::map_api_result(result)
}

const UNDO_CLASSIFICATION_RULES: &[(UndoErrorCode, &[&str])] = &[
    (
        UndoErrorCode::UndoUnavailable,
        &["nothing to undo", "no actions to undo"],
    ),
    (
        UndoErrorCode::RedoUnavailable,
        &["nothing to redo", "no actions to redo"],
    ),
    (
        UndoErrorCode::LockFailed,
        &["undo manager poisoned", "failed to lock"],
    ),
    (
        UndoErrorCode::SymlinkUnsupported,
        &[
            "symlinks are not allowed",
            "refusing path with symlink target",
            "refusing to operate on symlink",
        ],
    ),
    (
        UndoErrorCode::SnapshotMismatch,
        &["path changed during operation"],
    ),
    (
        UndoErrorCode::InvalidInput,
        &[
            "invalid backup path",
            "invalid destination path",
            "path must be absolute",
            "parent directory components are not allowed",
            "path contains invalid nul byte",
            "path is missing parent",
            "path is missing file name",
            "undo directory must be an absolute path",
            "undo directory cannot be the filesystem root",
        ],
    ),
    (
        UndoErrorCode::TargetExists,
        &["destination already exists", "already exists"],
    ),
    (
        UndoErrorCode::IoError,
        &[
            "failed to read metadata",
            "failed to create",
            "failed to copy",
            "failed to delete",
            "failed to rename",
            "failed to open",
            "failed to update",
        ],
    ),
];
