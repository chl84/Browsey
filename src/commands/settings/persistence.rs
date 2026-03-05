use super::error::{self, SettingsError, SettingsResult};
use std::ops::RangeInclusive;

pub(super) fn map_db_error(error: crate::db::DbError) -> SettingsError {
    match error.code() {
        crate::db::DbErrorCode::PermissionDenied => SettingsError::new(
            error::SettingsErrorCode::PermissionDenied,
            error.to_string(),
        ),
        crate::db::DbErrorCode::ReadOnlyFilesystem => SettingsError::new(
            error::SettingsErrorCode::ReadOnlyFilesystem,
            error.to_string(),
        ),
        crate::db::DbErrorCode::OpenFailed | crate::db::DbErrorCode::DataDirUnavailable => {
            SettingsError::new(error::SettingsErrorCode::DbOpenFailed, error.to_string())
        }
        crate::db::DbErrorCode::ReadFailed | crate::db::DbErrorCode::NotFound => {
            SettingsError::new(error::SettingsErrorCode::DbReadFailed, error.to_string())
        }
        crate::db::DbErrorCode::WriteFailed
        | crate::db::DbErrorCode::TransactionFailed
        | crate::db::DbErrorCode::SchemaInitFailed => {
            SettingsError::new(error::SettingsErrorCode::DbWriteFailed, error.to_string())
        }
        crate::db::DbErrorCode::SerializeFailed => {
            SettingsError::new(error::SettingsErrorCode::SerializeFailed, error.to_string())
        }
        crate::db::DbErrorCode::ParseFailed => {
            SettingsError::new(error::SettingsErrorCode::ParseFailed, error.to_string())
        }
        crate::db::DbErrorCode::UnknownError => {
            SettingsError::new(error::SettingsErrorCode::UnknownError, error.to_string())
        }
    }
}

pub(super) fn map_settings_result<T>(result: crate::db::DbResult<T>) -> SettingsResult<T> {
    result.map_err(map_db_error)
}

pub(super) fn open_connection() -> SettingsResult<rusqlite::Connection> {
    map_settings_result(crate::db::open())
}

pub(super) fn invalid_input<T>(message: &'static str) -> SettingsResult<T> {
    Err(SettingsError::invalid_input(message))
}

pub(super) fn normalize_log_level(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "error" => Some("error"),
        "warn" => Some("warn"),
        "info" => Some("info"),
        "debug" => Some("debug"),
        _ => None,
    }
}

pub(super) fn load_bounded_i64_setting(
    conn: &rusqlite::Connection,
    key: &str,
    range: RangeInclusive<i64>,
) -> SettingsResult<Option<i64>> {
    if let Some(raw) = map_settings_result(crate::db::get_setting_string(conn, key))? {
        if let Ok(value) = raw.parse::<i64>() {
            if range.contains(&value) {
                return Ok(Some(value));
            }
        }
    }
    Ok(None)
}
