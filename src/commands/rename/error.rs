use crate::errors::{
    api_error::ApiResult,
    domain::{
        self, classify_io_hint_from_message, classify_message_by_patterns, DomainError, ErrorCode,
        IoErrorHint,
    },
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RenameErrorCode {
    PathNotAbsolute,
    InvalidPath,
    InvalidInput,
    RootForbidden,
    SymlinkUnsupported,
    DuplicateSourcePath,
    DuplicateTargetName,
    TargetExists,
    NotFound,
    PermissionDenied,
    RollbackFailed,
    RenameFailed,
    UnknownError,
}

impl ErrorCode for RenameErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::PathNotAbsolute => "path_not_absolute",
            Self::InvalidPath => "invalid_path",
            Self::InvalidInput => "invalid_input",
            Self::RootForbidden => "root_forbidden",
            Self::SymlinkUnsupported => "symlink_unsupported",
            Self::DuplicateSourcePath => "duplicate_source_path",
            Self::DuplicateTargetName => "duplicate_target_name",
            Self::TargetExists => "target_exists",
            Self::NotFound => "not_found",
            Self::PermissionDenied => "permission_denied",
            Self::RollbackFailed => "rollback_failed",
            Self::RenameFailed => "rename_failed",
            Self::UnknownError => "unknown_error",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RenameError {
    code: RenameErrorCode,
    message: String,
}

impl RenameError {
    pub(super) fn new(code: RenameErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub(super) fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(RenameErrorCode::InvalidInput, message)
    }

    pub(super) fn from_external_message(message: impl Into<String>) -> Self {
        let message = message.into();
        if let Some(hint) = classify_io_hint_from_message(&message) {
            let code = match hint {
                IoErrorHint::NotFound => Some(RenameErrorCode::NotFound),
                IoErrorHint::PermissionDenied => Some(RenameErrorCode::PermissionDenied),
                IoErrorHint::AlreadyExists => Some(RenameErrorCode::TargetExists),
                _ => None,
            };
            if let Some(code) = code {
                return Self::new(code, message);
            }
        }
        let code = classify_message_by_patterns(
            &message,
            CLASSIFICATION_RULES,
            RenameErrorCode::UnknownError,
        );
        Self::new(code, message)
    }
}

impl fmt::Display for RenameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for RenameError {}

impl DomainError for RenameError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

impl From<String> for RenameError {
    fn from(message: String) -> Self {
        Self::from_external_message(message)
    }
}

impl From<&str> for RenameError {
    fn from(message: &str) -> Self {
        Self::from_external_message(message)
    }
}

impl From<crate::path_guard::PathGuardError> for RenameError {
    fn from(error: crate::path_guard::PathGuardError) -> Self {
        Self::from_external_message(error.to_string())
    }
}

pub(crate) type RenameResult<T> = Result<T, RenameError>;

pub(super) fn map_api_result<T>(result: RenameResult<T>) -> ApiResult<T> {
    domain::map_api_result(result)
}

const CLASSIFICATION_RULES: &[(RenameErrorCode, &[&str])] = &[
    (RenameErrorCode::PathNotAbsolute, &["path must be absolute"]),
    (
        RenameErrorCode::InvalidPath,
        &[
            "parent directory components are not allowed",
            "invalid path component (nul byte)",
            "path contains nul byte",
            "unsupported path prefix",
            "invalid destination path",
        ],
    ),
    (RenameErrorCode::InvalidInput, &["new name cannot be empty"]),
    (
        RenameErrorCode::RootForbidden,
        &["refusing to operate on filesystem root"],
    ),
    (
        RenameErrorCode::SymlinkUnsupported,
        &[
            "symlinks are not allowed in path",
            "symlinks are not allowed:",
        ],
    ),
    (
        RenameErrorCode::DuplicateSourcePath,
        &["duplicate source path in request"],
    ),
    (
        RenameErrorCode::DuplicateTargetName,
        &["duplicate target name in request"],
    ),
    (
        RenameErrorCode::TargetExists,
        &[
            "a file or directory with that name already exists",
            "target already exists",
        ],
    ),
    (
        RenameErrorCode::NotFound,
        &["path does not exist", "no such file or directory"],
    ),
    (
        RenameErrorCode::PermissionDenied,
        &[
            "permission denied",
            "operation not permitted",
            "access is denied",
        ],
    ),
    (RenameErrorCode::RollbackFailed, &["rollback also failed"]),
    (
        RenameErrorCode::RenameFailed,
        &["failed to rename", "cannot rename root"],
    ),
];
