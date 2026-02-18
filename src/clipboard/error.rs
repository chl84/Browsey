use crate::errors::{
    api_error::ApiResult,
    domain::{
        self, classify_io_hint_from_message, classify_message_by_patterns, DomainError, ErrorCode,
        IoErrorHint,
    },
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClipboardErrorCode {
    InvalidInput,
    InvalidMode,
    ClipboardEmpty,
    NotFound,
    NotDirectory,
    SymlinkUnsupported,
    DestinationExists,
    Cancelled,
    TaskFailed,
    RollbackFailed,
    IoError,
    UnknownError,
}

impl ErrorCode for ClipboardErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::InvalidMode => "invalid_mode",
            Self::ClipboardEmpty => "clipboard_empty",
            Self::NotFound => "not_found",
            Self::NotDirectory => "not_directory",
            Self::SymlinkUnsupported => "symlink_unsupported",
            Self::DestinationExists => "destination_exists",
            Self::Cancelled => "cancelled",
            Self::TaskFailed => "task_failed",
            Self::RollbackFailed => "rollback_failed",
            Self::IoError => "io_error",
            Self::UnknownError => "unknown_error",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ClipboardError {
    code: ClipboardErrorCode,
    message: String,
}

impl ClipboardError {
    pub(crate) fn new(code: ClipboardErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub(crate) fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(ClipboardErrorCode::InvalidInput, message)
    }

    pub(crate) fn cancelled() -> Self {
        Self::new(ClipboardErrorCode::Cancelled, "Copy cancelled")
    }

    pub(crate) fn code(&self) -> ClipboardErrorCode {
        self.code
    }

    pub(crate) fn from_external_message(message: impl Into<String>) -> Self {
        let message = message.into();
        if let Some(hint) = classify_io_hint_from_message(&message) {
            let code = match hint {
                IoErrorHint::NotFound => Some(ClipboardErrorCode::NotFound),
                IoErrorHint::AlreadyExists => Some(ClipboardErrorCode::DestinationExists),
                IoErrorHint::PermissionDenied | IoErrorHint::ReadOnlyFilesystem => {
                    Some(ClipboardErrorCode::IoError)
                }
                _ => None,
            };
            if let Some(code) = code {
                return Self::new(code, message);
            }
        }

        let code = classify_message_by_patterns(
            &message,
            CLIPBOARD_CLASSIFICATION_RULES,
            ClipboardErrorCode::UnknownError,
        );
        Self::new(code, message)
    }
}

impl fmt::Display for ClipboardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ClipboardError {}

impl DomainError for ClipboardError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

pub(crate) type ClipboardResult<T> = Result<T, ClipboardError>;

pub(crate) fn map_api_result<T>(result: ClipboardResult<T>) -> ApiResult<T> {
    domain::map_api_result(result)
}

const CLIPBOARD_CLASSIFICATION_RULES: &[(ClipboardErrorCode, &[&str])] = &[
    (ClipboardErrorCode::Cancelled, &["copy cancelled"]),
    (
        ClipboardErrorCode::ClipboardEmpty,
        &["clipboard is empty", "clipboard empty"],
    ),
    (
        ClipboardErrorCode::InvalidMode,
        &["invalid mode", "invalid conflict policy"],
    ),
    (ClipboardErrorCode::InvalidInput, &["invalid source path"]),
    (
        ClipboardErrorCode::NotDirectory,
        &["drop destination must be a directory"],
    ),
    (
        ClipboardErrorCode::SymlinkUnsupported,
        &[
            "symlinks are not supported",
            "refusing to copy symlinks",
            "refusing to overwrite symlinks",
        ],
    ),
    (
        ClipboardErrorCode::DestinationExists,
        &["already exists", "file exists", "destination exists"],
    ),
    (ClipboardErrorCode::TaskFailed, &["paste task failed"]),
    (ClipboardErrorCode::RollbackFailed, &["rollback"]),
];
