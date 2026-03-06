use super::error::{self, SettingsError, SettingsResult};
use std::ops::RangeInclusive;

const SETTINGS_SCHEMA_VERSION_KEY: &str = "settingsSchemaVersion";
const CURRENT_SETTINGS_SCHEMA_VERSION: &str = "1";

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
    let mut conn = map_settings_result(crate::db::open())?;
    run_settings_migrations(&mut conn)?;
    Ok(conn)
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

fn run_settings_migrations(conn: &mut rusqlite::Connection) -> SettingsResult<()> {
    let version = map_settings_result(crate::db::get_setting_string(conn, SETTINGS_SCHEMA_VERSION_KEY))?;
    if version.as_deref() == Some(CURRENT_SETTINGS_SCHEMA_VERSION) {
        return Ok(());
    }

    let ops = collect_settings_v1_migration_ops(conn)?;
    if ops.is_empty() && version.is_some() {
        return Ok(());
    }

    let tx = conn
        .transaction()
        .map_err(map_db_error_from_sqlite(
            error::SettingsErrorCode::DbWriteFailed,
            "Failed to start settings migration transaction",
        ))?;

    for op in ops {
        match op {
            SettingsMigrationOp::Set { key, value } => {
                tx.execute(
                    "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
                    rusqlite::params![key, value],
                )
                .map_err(map_db_error_from_sqlite(
                    error::SettingsErrorCode::DbWriteFailed,
                    format!("Failed to migrate setting {key}"),
                ))?;
            }
            SettingsMigrationOp::Delete { key } => {
                tx.execute("DELETE FROM settings WHERE key = ?1", rusqlite::params![key])
                    .map_err(map_db_error_from_sqlite(
                        error::SettingsErrorCode::DbWriteFailed,
                        format!("Failed to prune legacy setting {key}"),
                    ))?;
            }
        }
    }

    tx.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
        rusqlite::params![SETTINGS_SCHEMA_VERSION_KEY, CURRENT_SETTINGS_SCHEMA_VERSION],
    )
    .map_err(map_db_error_from_sqlite(
        error::SettingsErrorCode::DbWriteFailed,
        "Failed to store settings schema version",
    ))?;

    tx.commit().map_err(map_db_error_from_sqlite(
        error::SettingsErrorCode::DbWriteFailed,
        "Failed to commit settings migration",
    ))?;
    Ok(())
}

enum SettingsMigrationOp {
    Set { key: &'static str, value: String },
    Delete { key: &'static str },
}

fn collect_settings_v1_migration_ops(
    conn: &rusqlite::Connection,
) -> SettingsResult<Vec<SettingsMigrationOp>> {
    let mut ops = Vec::new();

    migrate_with_normalizer(conn, &mut ops, "startDir", normalize_trimmed_preserving_nonempty)?;
    migrate_with_normalizer(conn, &mut ops, "archiveName", normalize_archive_name)?;
    migrate_with_normalizer(conn, &mut ops, "ffmpegPath", normalize_trimmed_preserving_nonempty)?;
    migrate_with_normalizer(conn, &mut ops, "rclonePath", normalize_trimmed_preserving_nonempty)?;
    migrate_with_normalizer(conn, &mut ops, "logLevel", normalize_log_level_owned)?;

    migrate_with_normalizer(conn, &mut ops, "defaultView", enum_normalizer(&["list", "grid"]))?;
    migrate_with_normalizer(conn, &mut ops, "density", enum_normalizer(&["cozy", "compact"]))?;
    migrate_with_normalizer(
        conn,
        &mut ops,
        "sortField",
        enum_normalizer(&["name", "type", "modified", "size"]),
    )?;
    migrate_with_normalizer(conn, &mut ops, "sortDirection", enum_normalizer(&["asc", "desc"]))?;

    for key in [
        "showHidden",
        "hiddenFilesLast",
        "highContrast",
        "foldersFirst",
        "confirmDelete",
        "openDestAfterExtract",
        "videoThumbs",
        "cloudThumbs",
        "cloudEnabled",
        "hardwareAcceleration",
    ] {
        migrate_with_normalizer(conn, &mut ops, key, normalize_bool_string)?;
    }

    migrate_with_normalizer(conn, &mut ops, "archiveLevel", bounded_i64_normalizer(0..=9))?;
    migrate_with_normalizer(conn, &mut ops, "thumbCacheMb", bounded_i64_normalizer(50..=1000))?;
    migrate_with_normalizer(conn, &mut ops, "mountsPollMs", bounded_i64_normalizer(500..=10000))?;
    migrate_with_normalizer(conn, &mut ops, "doubleClickMs", bounded_i64_normalizer(150..=600))?;
    migrate_with_normalizer(conn, &mut ops, "scrollbarWidth", bounded_i64_normalizer(6..=16))?;

    Ok(ops)
}

fn migrate_with_normalizer(
    conn: &rusqlite::Connection,
    ops: &mut Vec<SettingsMigrationOp>,
    key: &'static str,
    normalize: impl Fn(&str) -> Option<String>,
) -> SettingsResult<()> {
    let Some(raw) = map_settings_result(crate::db::get_setting_string(conn, key))? else {
        return Ok(());
    };
    match normalize(&raw) {
        Some(normalized) if normalized == raw => {}
        Some(normalized) => ops.push(SettingsMigrationOp::Set {
            key,
            value: normalized,
        }),
        None => ops.push(SettingsMigrationOp::Delete { key }),
    }
    Ok(())
}

fn normalize_trimmed_preserving_nonempty(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_string())
}

fn normalize_archive_name(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.strip_suffix(".zip").unwrap_or(trimmed).to_string())
}

fn normalize_log_level_owned(raw: &str) -> Option<String> {
    normalize_log_level(raw).map(str::to_string)
}

fn normalize_bool_string(raw: &str) -> Option<String> {
    match raw.trim() {
        "true" => Some("true".to_string()),
        "false" => Some("false".to_string()),
        _ => None,
    }
}

fn bounded_i64_normalizer(
    range: RangeInclusive<i64>,
) -> impl Fn(&str) -> Option<String> {
    move |raw: &str| {
        let value = raw.trim().parse::<i64>().ok()?;
        if range.contains(&value) {
            Some(value.to_string())
        } else {
            None
        }
    }
}

fn enum_normalizer<'a>(
    allowed: &'a [&'a str],
) -> impl Fn(&str) -> Option<String> + 'a {
    move |raw: &str| {
        let trimmed = raw.trim();
        if allowed.contains(&trimmed) {
            Some(trimmed.to_string())
        } else {
            None
        }
    }
}

fn map_db_error_from_sqlite(
    fallback: error::SettingsErrorCode,
    context: impl Into<String>,
) -> impl FnOnce(rusqlite::Error) -> SettingsError {
    let context = context.into();
    move |error| {
        let code = match &error {
            rusqlite::Error::SqliteFailure(inner, _) => match inner.code {
                rusqlite::ffi::ErrorCode::PermissionDenied => {
                    error::SettingsErrorCode::PermissionDenied
                }
                rusqlite::ffi::ErrorCode::ReadOnly => {
                    error::SettingsErrorCode::ReadOnlyFilesystem
                }
                rusqlite::ffi::ErrorCode::CannotOpen => error::SettingsErrorCode::DbOpenFailed,
                _ => fallback,
            },
            _ => fallback,
        };
        SettingsError::new(code, format!("{context}: {error}"))
    }
}
