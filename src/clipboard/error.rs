use crate::errors::{
    api_error::ApiResult,
    domain::{self, classify_io_error, DomainError, ErrorCode, IoErrorHint},
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

    pub(crate) fn from_io_error(
        fallback: ClipboardErrorCode,
        context: &str,
        error: std::io::Error,
    ) -> Self {
        let code = match classify_io_error(&error) {
            IoErrorHint::NotFound => ClipboardErrorCode::NotFound,
            IoErrorHint::AlreadyExists => ClipboardErrorCode::DestinationExists,
            IoErrorHint::InvalidInput => ClipboardErrorCode::InvalidInput,
            _ => fallback,
        };
        Self::new(code, format!("{context}: {error}"))
    }

    pub(crate) fn cancelled() -> Self {
        Self::new(ClipboardErrorCode::Cancelled, "Copy cancelled")
    }

    pub(crate) fn code(&self) -> ClipboardErrorCode {
        self.code
    }

    pub(crate) fn with_context(self, context: impl AsRef<str>) -> Self {
        Self::new(self.code, format!("{}: {}", context.as_ref(), self.message))
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

impl From<crate::fs_utils::FsUtilsError> for ClipboardError {
    fn from(error: crate::fs_utils::FsUtilsError) -> Self {
        let code = match error.code() {
            crate::fs_utils::FsUtilsErrorCode::NotFound => ClipboardErrorCode::NotFound,
            crate::fs_utils::FsUtilsErrorCode::SymlinkUnsupported => {
                ClipboardErrorCode::SymlinkUnsupported
            }
            crate::fs_utils::FsUtilsErrorCode::InvalidPath
            | crate::fs_utils::FsUtilsErrorCode::RootForbidden => ClipboardErrorCode::InvalidInput,
            crate::fs_utils::FsUtilsErrorCode::PermissionDenied
            | crate::fs_utils::FsUtilsErrorCode::ReadOnlyFilesystem
            | crate::fs_utils::FsUtilsErrorCode::CanonicalizeFailed
            | crate::fs_utils::FsUtilsErrorCode::MetadataReadFailed => ClipboardErrorCode::IoError,
        };
        Self::new(code, error.to_string())
    }
}

impl From<crate::tasks::TaskError> for ClipboardError {
    fn from(error: crate::tasks::TaskError) -> Self {
        let code = match error.code() {
            crate::tasks::TaskErrorCode::RegistryLockFailed
            | crate::tasks::TaskErrorCode::TaskNotFound => ClipboardErrorCode::TaskFailed,
        };
        Self::new(code, error.to_string())
    }
}

impl From<crate::undo::UndoError> for ClipboardError {
    fn from(error: crate::undo::UndoError) -> Self {
        let code = match error.code() {
            crate::undo::UndoErrorCode::InvalidInput => ClipboardErrorCode::InvalidInput,
            crate::undo::UndoErrorCode::NotFound => ClipboardErrorCode::NotFound,
            crate::undo::UndoErrorCode::TargetExists => ClipboardErrorCode::DestinationExists,
            crate::undo::UndoErrorCode::SymlinkUnsupported => {
                ClipboardErrorCode::SymlinkUnsupported
            }
            _ => ClipboardErrorCode::IoError,
        };
        Self::new(code, error.to_string())
    }
}

pub(crate) type ClipboardResult<T> = Result<T, ClipboardError>;

pub(crate) fn map_api_result<T>(result: ClipboardResult<T>) -> ApiResult<T> {
    domain::map_api_result(result)
}

#[cfg(test)]
mod tests {
    use super::{ClipboardError, ClipboardErrorCode};
    use crate::undo::{UndoError, UndoErrorCode};

    #[test]
    fn maps_undo_target_exists_to_destination_exists_even_with_misleading_message() {
        let undo = UndoError::new(UndoErrorCode::TargetExists, "permission denied");
        let clipboard: ClipboardError = undo.into();
        assert_eq!(clipboard.code(), ClipboardErrorCode::DestinationExists);
    }

    #[test]
    fn maps_io_already_exists_to_destination_exists() {
        let io_error = std::io::Error::new(std::io::ErrorKind::AlreadyExists, "exists");
        let clipboard =
            ClipboardError::from_io_error(ClipboardErrorCode::IoError, "copy failed", io_error);
        assert_eq!(clipboard.code(), ClipboardErrorCode::DestinationExists);
    }
}
