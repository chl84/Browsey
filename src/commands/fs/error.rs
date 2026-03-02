#[cfg(target_os = "windows")]
use crate::errors::domain::classify_io_error;
use crate::errors::{
    api_error::{ApiError, ApiResult},
    domain::{
        classify_io_hint_from_message, classify_message_by_patterns, DomainError, ErrorCode,
        IoErrorHint, COMMON_PATH_NOT_ABSOLUTE_PATTERNS, COMMON_PERMISSION_DENIED_PATTERNS,
    },
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SetHiddenErrorCode {
    PathNotAbsolute,
    InvalidPath,
    InvalidInput,
    RootForbidden,
    SymlinkUnsupported,
    TargetExists,
    NotFound,
    PermissionDenied,
    HiddenUpdateFailed,
    UnknownError,
}

impl ErrorCode for SetHiddenErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::PathNotAbsolute => "path_not_absolute",
            Self::InvalidPath => "invalid_path",
            Self::InvalidInput => "invalid_input",
            Self::RootForbidden => "root_forbidden",
            Self::SymlinkUnsupported => "symlink_unsupported",
            Self::TargetExists => "target_exists",
            Self::NotFound => "not_found",
            Self::PermissionDenied => "permission_denied",
            Self::HiddenUpdateFailed => "hidden_update_failed",
            Self::UnknownError => "unknown_error",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct SetHiddenError {
    code: SetHiddenErrorCode,
    message: String,
}

impl SetHiddenError {
    pub(super) fn new(code: SetHiddenErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub(super) fn from_external_message(message: impl Into<String>) -> Self {
        let message = message.into();
        if let Some(hint) = classify_io_hint_from_message(&message) {
            let code = match hint {
                IoErrorHint::NotFound => Some(SetHiddenErrorCode::NotFound),
                IoErrorHint::PermissionDenied => Some(SetHiddenErrorCode::PermissionDenied),
                IoErrorHint::AlreadyExists => Some(SetHiddenErrorCode::TargetExists),
                _ => None,
            };
            if let Some(code) = code {
                return Self::new(code, message);
            }
        }
        let code = classify_message_by_patterns(
            &message,
            SET_HIDDEN_CLASSIFICATION_RULES,
            SetHiddenErrorCode::UnknownError,
        );
        Self::new(code, message)
    }

    #[cfg(target_os = "windows")]
    pub(super) fn from_io_error(
        fallback: SetHiddenErrorCode,
        context: &str,
        error: std::io::Error,
    ) -> Self {
        let code = match classify_io_error(&error) {
            IoErrorHint::NotFound => SetHiddenErrorCode::NotFound,
            IoErrorHint::PermissionDenied => SetHiddenErrorCode::PermissionDenied,
            IoErrorHint::AlreadyExists => SetHiddenErrorCode::TargetExists,
            _ => fallback,
        };
        Self::new(code, format!("{context}: {error}"))
    }

    pub(super) fn code(&self) -> SetHiddenErrorCode {
        self.code
    }

    pub(super) fn to_api_error(&self) -> ApiError {
        <Self as DomainError>::to_api_error(self)
    }
}

impl fmt::Display for SetHiddenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for SetHiddenError {}

impl DomainError for SetHiddenError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

impl From<String> for SetHiddenError {
    fn from(message: String) -> Self {
        Self::from_external_message(message)
    }
}

impl From<&str> for SetHiddenError {
    fn from(message: &str) -> Self {
        Self::from_external_message(message)
    }
}

impl From<crate::fs_utils::FsUtilsError> for SetHiddenError {
    fn from(error: crate::fs_utils::FsUtilsError) -> Self {
        let code = match error.code() {
            crate::fs_utils::FsUtilsErrorCode::InvalidPath => SetHiddenErrorCode::InvalidPath,
            crate::fs_utils::FsUtilsErrorCode::NotFound => SetHiddenErrorCode::NotFound,
            crate::fs_utils::FsUtilsErrorCode::PermissionDenied => {
                SetHiddenErrorCode::PermissionDenied
            }
            crate::fs_utils::FsUtilsErrorCode::RootForbidden => SetHiddenErrorCode::RootForbidden,
            crate::fs_utils::FsUtilsErrorCode::SymlinkUnsupported => {
                SetHiddenErrorCode::SymlinkUnsupported
            }
            crate::fs_utils::FsUtilsErrorCode::ReadOnlyFilesystem
            | crate::fs_utils::FsUtilsErrorCode::CanonicalizeFailed
            | crate::fs_utils::FsUtilsErrorCode::MetadataReadFailed => {
                SetHiddenErrorCode::UnknownError
            }
        };
        Self::new(code, error.to_string())
    }
}

pub(super) type SetHiddenResult<T> = Result<T, SetHiddenError>;

pub(super) fn is_expected_set_hidden_error(error: &SetHiddenError) -> bool {
    matches!(
        error.code(),
        SetHiddenErrorCode::SymlinkUnsupported
            | SetHiddenErrorCode::NotFound
            | SetHiddenErrorCode::PermissionDenied
            | SetHiddenErrorCode::TargetExists
    )
}

const SET_HIDDEN_CLASSIFICATION_RULES: &[(SetHiddenErrorCode, &[&str])] = &[
    (
        SetHiddenErrorCode::PathNotAbsolute,
        COMMON_PATH_NOT_ABSOLUTE_PATTERNS,
    ),
    (
        SetHiddenErrorCode::InvalidPath,
        &[
            "parent directory components are not allowed",
            "invalid path component (nul byte)",
            "path contains nul byte",
            "unsupported path prefix",
            "invalid file name",
            "cannot derive visible name",
            "missing parent",
        ],
    ),
    (SetHiddenErrorCode::InvalidInput, &["no paths provided"]),
    (
        SetHiddenErrorCode::RootForbidden,
        &["refusing to operate on filesystem root"],
    ),
    (
        SetHiddenErrorCode::SymlinkUnsupported,
        &[
            "symlinks are not allowed in path",
            "symlinks are not allowed:",
        ],
    ),
    (SetHiddenErrorCode::TargetExists, &["target already exists"]),
    (
        SetHiddenErrorCode::NotFound,
        &["path does not exist", "no such file or directory"],
    ),
    (
        SetHiddenErrorCode::PermissionDenied,
        COMMON_PERMISSION_DENIED_PATTERNS,
    ),
    (
        SetHiddenErrorCode::HiddenUpdateFailed,
        &[
            "setfileattributes failed",
            "getfileattributes failed",
            "failed to rename",
        ],
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FsErrorCode {
    InvalidInput,
    PathNotAbsolute,
    InvalidPath,
    RootForbidden,
    SymlinkUnsupported,
    NotFound,
    PermissionDenied,
    ReadOnlyFilesystem,
    TargetExists,
    Cancelled,
    TaskFailed,
    CreateFailed,
    DeleteFailed,
    OpenFailed,
    TrashFailed,
    UnknownError,
}

impl ErrorCode for FsErrorCode {
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
            Self::TargetExists => "target_exists",
            Self::Cancelled => "cancelled",
            Self::TaskFailed => "task_failed",
            Self::CreateFailed => "create_failed",
            Self::DeleteFailed => "delete_failed",
            Self::OpenFailed => "open_failed",
            Self::TrashFailed => "trash_failed",
            Self::UnknownError => "unknown_error",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct FsError {
    code: FsErrorCode,
    message: String,
}

impl FsError {
    pub(super) fn new(code: FsErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub(super) fn from_external_message(message: impl Into<String>) -> Self {
        let message = message.into();
        if let Some(hint) = classify_io_hint_from_message(&message) {
            let io_code = match hint {
                IoErrorHint::NotFound => Some(FsErrorCode::NotFound),
                IoErrorHint::PermissionDenied => Some(FsErrorCode::PermissionDenied),
                IoErrorHint::ReadOnlyFilesystem => Some(FsErrorCode::ReadOnlyFilesystem),
                IoErrorHint::AlreadyExists => Some(FsErrorCode::TargetExists),
                _ => None,
            };
            if let Some(code) = io_code {
                return Self::new(code, message);
            }
        }
        let code = classify_message_by_patterns(
            &message,
            FS_CLASSIFICATION_RULES,
            FsErrorCode::UnknownError,
        );
        Self::new(code, message)
    }
}

impl fmt::Display for FsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for FsError {}

impl DomainError for FsError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

impl From<String> for FsError {
    fn from(message: String) -> Self {
        Self::from_external_message(message)
    }
}

impl From<&str> for FsError {
    fn from(message: &str) -> Self {
        Self::from_external_message(message)
    }
}

impl From<crate::fs_utils::FsUtilsError> for FsError {
    fn from(error: crate::fs_utils::FsUtilsError) -> Self {
        let code = match error.code() {
            crate::fs_utils::FsUtilsErrorCode::InvalidPath => FsErrorCode::InvalidPath,
            crate::fs_utils::FsUtilsErrorCode::NotFound => FsErrorCode::NotFound,
            crate::fs_utils::FsUtilsErrorCode::PermissionDenied => FsErrorCode::PermissionDenied,
            crate::fs_utils::FsUtilsErrorCode::ReadOnlyFilesystem => {
                FsErrorCode::ReadOnlyFilesystem
            }
            crate::fs_utils::FsUtilsErrorCode::RootForbidden => FsErrorCode::RootForbidden,
            crate::fs_utils::FsUtilsErrorCode::SymlinkUnsupported => {
                FsErrorCode::SymlinkUnsupported
            }
            crate::fs_utils::FsUtilsErrorCode::CanonicalizeFailed
            | crate::fs_utils::FsUtilsErrorCode::MetadataReadFailed => FsErrorCode::UnknownError,
        };
        Self::new(code, error.to_string())
    }
}

impl From<crate::path_guard::PathGuardError> for FsError {
    fn from(error: crate::path_guard::PathGuardError) -> Self {
        let code = match error.code() {
            crate::path_guard::PathGuardErrorCode::NotFound => FsErrorCode::NotFound,
            crate::path_guard::PathGuardErrorCode::PermissionDenied => {
                FsErrorCode::PermissionDenied
            }
            crate::path_guard::PathGuardErrorCode::NotDirectory => FsErrorCode::InvalidPath,
            crate::path_guard::PathGuardErrorCode::SymlinkUnsupported => {
                FsErrorCode::SymlinkUnsupported
            }
            crate::path_guard::PathGuardErrorCode::MetadataReadFailed => FsErrorCode::UnknownError,
        };
        Self::new(code, error.to_string())
    }
}

impl From<crate::undo::UndoError> for FsError {
    fn from(error: crate::undo::UndoError) -> Self {
        let code = match error.code() {
            crate::undo::UndoErrorCode::InvalidInput => FsErrorCode::InvalidInput,
            crate::undo::UndoErrorCode::NotFound => FsErrorCode::NotFound,
            crate::undo::UndoErrorCode::PermissionDenied => FsErrorCode::PermissionDenied,
            crate::undo::UndoErrorCode::ReadOnlyFilesystem => FsErrorCode::ReadOnlyFilesystem,
            crate::undo::UndoErrorCode::TargetExists => FsErrorCode::TargetExists,
            crate::undo::UndoErrorCode::SymlinkUnsupported => FsErrorCode::SymlinkUnsupported,
            crate::undo::UndoErrorCode::CrossDeviceMove
            | crate::undo::UndoErrorCode::AtomicRenameUnsupported
            | crate::undo::UndoErrorCode::SnapshotMismatch
            | crate::undo::UndoErrorCode::UndoUnavailable
            | crate::undo::UndoErrorCode::RedoUnavailable
            | crate::undo::UndoErrorCode::LockFailed
            | crate::undo::UndoErrorCode::IoError => FsErrorCode::UnknownError,
        };
        Self::new(code, error.to_string())
    }
}

pub(crate) type FsResult<T> = Result<T, FsError>;

pub(super) fn map_api_result<T>(result: FsResult<T>) -> ApiResult<T> {
    result.map_err(|error| error.to_api_error())
}

pub(super) fn map_external_result<T, E>(result: Result<T, E>) -> FsResult<T>
where
    E: Into<FsError>,
{
    result.map_err(Into::into)
}

const FS_CLASSIFICATION_RULES: &[(FsErrorCode, &[&str])] = &[
    (FsErrorCode::Cancelled, &["cancelled"]),
    (
        FsErrorCode::TaskFailed,
        &[
            "task failed",
            "task panicked",
            "failed to register cancel",
            "channel closed",
        ],
    ),
    (
        FsErrorCode::PathNotAbsolute,
        &["path must be absolute", "start directory not found"],
    ),
    (
        FsErrorCode::InvalidPath,
        &[
            "parent directory components are not allowed",
            "invalid path component (nul byte)",
            "path contains nul byte",
            "unsupported path prefix",
            "is not a directory",
        ],
    ),
    (
        FsErrorCode::InvalidInput,
        &[
            "no paths provided",
            "name cannot be empty",
            "folder name cannot be empty",
            "file name cannot be empty",
            "path separators",
            "nothing to restore",
            "nothing to delete",
        ],
    ),
    (
        FsErrorCode::RootForbidden,
        &["refusing to operate on filesystem root"],
    ),
    (
        FsErrorCode::SymlinkUnsupported,
        &["symlinks are not allowed"],
    ),
    (
        FsErrorCode::PermissionDenied,
        COMMON_PERMISSION_DENIED_PATTERNS,
    ),
    (
        FsErrorCode::TargetExists,
        &[
            "already exists",
            "destination exists",
            "file exists",
            "target already exists",
        ],
    ),
    (
        FsErrorCode::CreateFailed,
        &["failed to create folder", "failed to create file"],
    ),
    (
        FsErrorCode::DeleteFailed,
        &[
            "failed to delete",
            "delete cancelled",
            "rollback also failed",
        ],
    ),
    (
        FsErrorCode::OpenFailed,
        &["failed to open", "open timed out"],
    ),
    (
        FsErrorCode::TrashFailed,
        &[
            "failed to list trash",
            "failed to restore",
            "failed to delete permanently",
            "move to trash",
        ],
    ),
];
