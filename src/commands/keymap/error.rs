use crate::errors::{
    api_error::ApiResult,
    domain::{self, classify_message_by_patterns, DomainError, ErrorCode},
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum KeymapErrorCode {
    DatabaseOpenFailed,
    InvalidInput,
    InvalidAccelerator,
    ShortcutUpdateFailed,
    ShortcutResetFailed,
    ShortcutLoadFailed,
    UnknownError,
}

impl ErrorCode for KeymapErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::DatabaseOpenFailed => "database_open_failed",
            Self::InvalidInput => "invalid_input",
            Self::InvalidAccelerator => "invalid_accelerator",
            Self::ShortcutUpdateFailed => "shortcut_update_failed",
            Self::ShortcutResetFailed => "shortcut_reset_failed",
            Self::ShortcutLoadFailed => "shortcut_load_failed",
            Self::UnknownError => "unknown_error",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct KeymapError {
    code: KeymapErrorCode,
    message: String,
}

impl KeymapError {
    pub(super) fn new(code: KeymapErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub(super) fn from_external_message(message: impl Into<String>) -> Self {
        let message = message.into();
        let code = classify_message_by_patterns(
            &message,
            KEYMAP_CLASSIFICATION_RULES,
            KeymapErrorCode::UnknownError,
        );
        Self::new(code, message)
    }
}

impl fmt::Display for KeymapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for KeymapError {}

impl DomainError for KeymapError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

pub(super) type KeymapResult<T> = Result<T, KeymapError>;

pub(super) fn map_api_result<T>(result: KeymapResult<T>) -> ApiResult<T> {
    domain::map_api_result(result)
}

const KEYMAP_CLASSIFICATION_RULES: &[(KeymapErrorCode, &[&str])] = &[
    (
        KeymapErrorCode::InvalidInput,
        &[
            "command id cannot be empty",
            "accelerator cannot be empty",
            "invalid command id",
        ],
    ),
    (
        KeymapErrorCode::InvalidAccelerator,
        &["invalid accelerator", "unknown key token", "modifier"],
    ),
    (
        KeymapErrorCode::ShortcutLoadFailed,
        &["failed to load shortcuts", "failed to query shortcuts"],
    ),
    (
        KeymapErrorCode::ShortcutResetFailed,
        &["failed to reset shortcut", "failed to reset all shortcuts"],
    ),
    (
        KeymapErrorCode::ShortcutUpdateFailed,
        &["failed to save shortcut", "failed to set shortcut"],
    ),
];
