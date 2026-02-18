use crate::errors::{
    api_error::ApiError,
    domain::{classify_message_by_patterns, DomainError, ErrorCode},
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
        let code = classify_message_by_patterns(
            &message,
            SET_HIDDEN_CLASSIFICATION_RULES,
            SetHiddenErrorCode::UnknownError,
        );
        Self::new(code, message)
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
        &["path must be absolute"],
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
        &[
            "permission denied",
            "operation not permitted",
            "access is denied",
        ],
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
