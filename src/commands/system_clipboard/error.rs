use crate::errors::{
    api_error::ApiResult,
    domain::{
        self, classify_io_hint_from_message, classify_message_by_patterns, DomainError, ErrorCode,
        IoErrorHint,
    },
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SystemClipboardErrorCode {
    InvalidInput,
    PathNotAbsolute,
    InvalidPath,
    ClipboardEmpty,
    ClipboardToolMissing,
    PermissionDenied,
    ClipboardReadFailed,
    ClipboardWriteFailed,
    UnknownError,
}

impl ErrorCode for SystemClipboardErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::PathNotAbsolute => "path_not_absolute",
            Self::InvalidPath => "invalid_path",
            Self::ClipboardEmpty => "clipboard_empty",
            Self::ClipboardToolMissing => "clipboard_tool_missing",
            Self::PermissionDenied => "permission_denied",
            Self::ClipboardReadFailed => "clipboard_read_failed",
            Self::ClipboardWriteFailed => "clipboard_write_failed",
            Self::UnknownError => "unknown_error",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct SystemClipboardError {
    code: SystemClipboardErrorCode,
    message: String,
}

impl SystemClipboardError {
    pub(super) fn new(code: SystemClipboardErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub(super) fn from_external_message(message: impl Into<String>) -> Self {
        let message = message.into();
        if let Some(hint) = classify_io_hint_from_message(&message) {
            let code = match hint {
                IoErrorHint::PermissionDenied => Some(SystemClipboardErrorCode::PermissionDenied),
                _ => None,
            };
            if let Some(code) = code {
                return Self::new(code, message);
            }
        }

        let code = classify_message_by_patterns(
            &message,
            SYSTEM_CLIPBOARD_CLASSIFICATION_RULES,
            SystemClipboardErrorCode::UnknownError,
        );
        Self::new(code, message)
    }

    pub(super) fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(SystemClipboardErrorCode::InvalidInput, message)
    }

    pub(super) fn code(&self) -> SystemClipboardErrorCode {
        self.code
    }
}

impl fmt::Display for SystemClipboardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for SystemClipboardError {}

impl DomainError for SystemClipboardError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

pub(super) type SystemClipboardResult<T> = Result<T, SystemClipboardError>;

pub(super) fn map_api_result<T>(result: SystemClipboardResult<T>) -> ApiResult<T> {
    domain::map_api_result(result)
}

const SYSTEM_CLIPBOARD_CLASSIFICATION_RULES: &[(SystemClipboardErrorCode, &[&str])] = &[
    (
        SystemClipboardErrorCode::PathNotAbsolute,
        &["path must be absolute"],
    ),
    (
        SystemClipboardErrorCode::InvalidPath,
        &[
            "parent directory components are not allowed",
            "invalid path component (nul byte)",
            "path contains nul byte",
            "unsupported path prefix",
        ],
    ),
    (
        SystemClipboardErrorCode::ClipboardToolMissing,
        &[
            "wl-copy not found",
            "xclip not found",
            "no compatible clipboard tool found",
        ],
    ),
    (
        SystemClipboardErrorCode::ClipboardReadFailed,
        &["clipboard read failed", "clipboard text decode failed"],
    ),
    (
        SystemClipboardErrorCode::ClipboardWriteFailed,
        &[
            "wl-copy failed",
            "xclip failed",
            "wl-copy exited with status",
            "xclip exited with status",
        ],
    ),
];
