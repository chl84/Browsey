use crate::errors::{
    api_error::ApiResult,
    domain::{self, DomainError, ErrorCode},
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum KeymapErrorCode {
    DatabaseOpenFailed,
    InvalidInput,
    InvalidAccelerator,
    ShortcutUpdateFailed,
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

impl From<crate::keymap::KeymapCoreError> for KeymapError {
    fn from(error: crate::keymap::KeymapCoreError) -> Self {
        let code = match error.code() {
            crate::keymap::KeymapCoreErrorCode::InvalidInput => KeymapErrorCode::InvalidInput,
            crate::keymap::KeymapCoreErrorCode::InvalidAccelerator => {
                KeymapErrorCode::InvalidAccelerator
            }
            crate::keymap::KeymapCoreErrorCode::ShortcutConflict
            | crate::keymap::KeymapCoreErrorCode::SerializeFailed
            | crate::keymap::KeymapCoreErrorCode::DbWriteFailed => {
                KeymapErrorCode::ShortcutUpdateFailed
            }
            crate::keymap::KeymapCoreErrorCode::ParseFailed
            | crate::keymap::KeymapCoreErrorCode::DbReadFailed => {
                KeymapErrorCode::ShortcutLoadFailed
            }
            crate::keymap::KeymapCoreErrorCode::UnknownError => KeymapErrorCode::UnknownError,
        };
        KeymapError::new(code, error.to_string())
    }
}

pub(super) type KeymapResult<T> = Result<T, KeymapError>;

pub(super) fn map_api_result<T>(result: KeymapResult<T>) -> ApiResult<T> {
    domain::map_api_result(result)
}

#[cfg(test)]
mod tests {
    use super::KeymapError;
    use crate::errors::domain::DomainError;
    use crate::keymap::{KeymapCoreError, KeymapCoreErrorCode};

    #[test]
    fn maps_keymap_core_conflict_to_shortcut_update_failed() {
        let error = KeymapError::from(KeymapCoreError::new(
            KeymapCoreErrorCode::ShortcutConflict,
            "already used by",
        ));
        assert_eq!(error.code_str(), "shortcut_update_failed");
    }

    #[test]
    fn maps_keymap_core_parse_failed_to_shortcut_load_failed() {
        let error = KeymapError::from(KeymapCoreError::new(
            KeymapCoreErrorCode::ParseFailed,
            "parse failed",
        ));
        assert_eq!(error.code_str(), "shortcut_load_failed");
    }

    #[test]
    fn maps_keymap_core_invalid_accelerator_to_invalid_accelerator() {
        let error = KeymapError::from(KeymapCoreError::new(
            KeymapCoreErrorCode::InvalidAccelerator,
            "invalid",
        ));
        assert_eq!(error.code_str(), "invalid_accelerator");
    }
}
