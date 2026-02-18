use crate::errors::domain::{classify_message_by_patterns, DomainError, ErrorCode};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeymapCoreErrorCode {
    InvalidInput,
    InvalidAccelerator,
    ShortcutConflict,
    ParseFailed,
    SerializeFailed,
    DbReadFailed,
    DbWriteFailed,
    UnknownError,
}

impl ErrorCode for KeymapCoreErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::InvalidAccelerator => "invalid_accelerator",
            Self::ShortcutConflict => "shortcut_conflict",
            Self::ParseFailed => "parse_failed",
            Self::SerializeFailed => "serialize_failed",
            Self::DbReadFailed => "db_read_failed",
            Self::DbWriteFailed => "db_write_failed",
            Self::UnknownError => "unknown_error",
        }
    }
}

#[derive(Debug, Clone)]
pub struct KeymapCoreError {
    code: KeymapCoreErrorCode,
    message: String,
}

impl KeymapCoreError {
    pub fn new(code: KeymapCoreErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn from_external_message(message: impl Into<String>) -> Self {
        let message = message.into();
        let code = classify_message_by_patterns(
            &message,
            KEYMAP_CORE_CLASSIFICATION_RULES,
            KeymapCoreErrorCode::UnknownError,
        );
        Self::new(code, message)
    }
}

impl fmt::Display for KeymapCoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for KeymapCoreError {}

impl DomainError for KeymapCoreError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

pub type KeymapCoreResult<T> = Result<T, KeymapCoreError>;

const KEYMAP_CORE_CLASSIFICATION_RULES: &[(KeymapCoreErrorCode, &[&str])] = &[
    (
        KeymapCoreErrorCode::InvalidInput,
        &[
            "unknown shortcut command",
            "shortcut cannot be empty",
            "shortcut must include a key",
            "missing key",
        ],
    ),
    (
        KeymapCoreErrorCode::InvalidAccelerator,
        &[
            "invalid shortcut format",
            "unsupported key",
            "unsupported function key",
            "alphanumeric shortcuts require",
            "reserved",
            "invalid shortcut for",
            "invalid default shortcut",
        ],
    ),
    (KeymapCoreErrorCode::ShortcutConflict, &["already used by"]),
    (
        KeymapCoreErrorCode::ParseFailed,
        &["failed to parse shortcut settings"],
    ),
    (
        KeymapCoreErrorCode::SerializeFailed,
        &["failed to serialize shortcut settings"],
    ),
    (
        KeymapCoreErrorCode::DbReadFailed,
        &["failed to read setting", "failed to open db"],
    ),
    (
        KeymapCoreErrorCode::DbWriteFailed,
        &["failed to store setting"],
    ),
];
