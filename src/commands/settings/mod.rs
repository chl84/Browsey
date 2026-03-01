//! Persisted UI settings such as column widths.

mod error;

use std::ops::RangeInclusive;

use crate::{
    db::{load_column_widths, save_column_widths},
    errors::api_error::ApiResult,
};

use self::error::{map_api_result, SettingsError, SettingsResult};

fn map_db_error(error: crate::db::DbError) -> SettingsError {
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

fn map_settings_result<T>(result: crate::db::DbResult<T>) -> SettingsResult<T> {
    result.map_err(map_db_error)
}

fn open_connection() -> SettingsResult<rusqlite::Connection> {
    map_settings_result(crate::db::open())
}

fn invalid_input<T>(message: &'static str) -> SettingsResult<T> {
    Err(SettingsError::invalid_input(message))
}

fn normalize_log_level(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "error" => Some("error"),
        "warn" => Some("warn"),
        "info" => Some("info"),
        "debug" => Some("debug"),
        _ => None,
    }
}

fn load_bounded_i64_setting(
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

#[tauri::command]
pub fn store_column_widths(widths: Vec<f64>) -> ApiResult<()> {
    map_api_result((|| -> SettingsResult<()> {
        let conn = open_connection()?;
        map_settings_result(save_column_widths(&conn, &widths))
    })())
}

#[tauri::command]
pub fn load_saved_column_widths() -> ApiResult<Option<Vec<f64>>> {
    map_api_result((|| -> SettingsResult<Option<Vec<f64>>> {
        let conn = open_connection()?;
        map_settings_result(load_column_widths(&conn))
    })())
}

#[tauri::command]
pub fn store_show_hidden(value: bool) -> ApiResult<()> {
    map_api_result((|| -> SettingsResult<()> {
        let conn = open_connection()?;
        map_settings_result(crate::db::set_setting_bool(&conn, "showHidden", value))
    })())
}

#[tauri::command]
pub fn load_show_hidden() -> ApiResult<Option<bool>> {
    map_api_result((|| -> SettingsResult<Option<bool>> {
        let conn = open_connection()?;
        map_settings_result(crate::db::get_setting_bool(&conn, "showHidden"))
    })())
}

#[tauri::command]
pub fn store_hidden_files_last(value: bool) -> ApiResult<()> {
    map_api_result((|| -> SettingsResult<()> {
        let conn = open_connection()?;
        map_settings_result(crate::db::set_setting_bool(&conn, "hiddenFilesLast", value))
    })())
}

#[tauri::command]
pub fn load_hidden_files_last() -> ApiResult<Option<bool>> {
    map_api_result((|| -> SettingsResult<Option<bool>> {
        let conn = open_connection()?;
        map_settings_result(crate::db::get_setting_bool(&conn, "hiddenFilesLast"))
    })())
}

#[tauri::command]
pub fn store_high_contrast(value: bool) -> ApiResult<()> {
    map_api_result((|| -> SettingsResult<()> {
        let conn = open_connection()?;
        map_settings_result(crate::db::set_setting_bool(&conn, "highContrast", value))
    })())
}

#[tauri::command]
pub fn load_high_contrast() -> ApiResult<Option<bool>> {
    map_api_result((|| -> SettingsResult<Option<bool>> {
        let conn = open_connection()?;
        map_settings_result(crate::db::get_setting_bool(&conn, "highContrast"))
    })())
}

#[tauri::command]
pub fn store_folders_first(value: bool) -> ApiResult<()> {
    map_api_result((|| -> SettingsResult<()> {
        let conn = open_connection()?;
        map_settings_result(crate::db::set_setting_bool(&conn, "foldersFirst", value))
    })())
}

#[tauri::command]
pub fn load_folders_first() -> ApiResult<Option<bool>> {
    map_api_result((|| -> SettingsResult<Option<bool>> {
        let conn = open_connection()?;
        map_settings_result(crate::db::get_setting_bool(&conn, "foldersFirst"))
    })())
}

#[tauri::command]
pub fn store_default_view(value: String) -> ApiResult<()> {
    map_api_result((|| -> SettingsResult<()> {
        let conn = open_connection()?;
        map_settings_result(crate::db::set_setting_string(&conn, "defaultView", &value))
    })())
}

#[tauri::command]
pub fn load_default_view() -> ApiResult<Option<String>> {
    map_api_result((|| -> SettingsResult<Option<String>> {
        let conn = open_connection()?;
        map_settings_result(crate::db::get_setting_string(&conn, "defaultView"))
    })())
}

#[tauri::command]
pub fn store_start_dir(value: String) -> ApiResult<()> {
    map_api_result((|| -> SettingsResult<()> {
        let conn = open_connection()?;
        map_settings_result(crate::db::set_setting_string(&conn, "startDir", &value))
    })())
}

#[tauri::command]
pub fn load_start_dir() -> ApiResult<Option<String>> {
    map_api_result((|| -> SettingsResult<Option<String>> {
        let conn = open_connection()?;
        map_settings_result(crate::db::get_setting_string(&conn, "startDir"))
    })())
}

#[tauri::command]
pub fn store_confirm_delete(value: bool) -> ApiResult<()> {
    map_api_result((|| -> SettingsResult<()> {
        let conn = open_connection()?;
        map_settings_result(crate::db::set_setting_bool(&conn, "confirmDelete", value))
    })())
}

#[tauri::command]
pub fn load_confirm_delete() -> ApiResult<Option<bool>> {
    map_api_result((|| -> SettingsResult<Option<bool>> {
        let conn = open_connection()?;
        map_settings_result(crate::db::get_setting_bool(&conn, "confirmDelete"))
    })())
}

#[tauri::command]
pub fn store_sort_field(value: String) -> ApiResult<()> {
    map_api_result((|| -> SettingsResult<()> {
        match value.as_str() {
            "name" | "type" | "modified" | "size" => {
                let conn = open_connection()?;
                map_settings_result(crate::db::set_setting_string(&conn, "sortField", &value))
            }
            _ => invalid_input("invalid sort field"),
        }
    })())
}

#[tauri::command]
pub fn load_sort_field() -> ApiResult<Option<String>> {
    map_api_result((|| -> SettingsResult<Option<String>> {
        let conn = open_connection()?;
        let value = map_settings_result(crate::db::get_setting_string(&conn, "sortField"))?;
        Ok(match value.as_deref() {
            Some("name") | Some("type") | Some("modified") | Some("size") => value,
            _ => None,
        })
    })())
}

#[tauri::command]
pub fn store_sort_direction(value: String) -> ApiResult<()> {
    map_api_result((|| -> SettingsResult<()> {
        match value.as_str() {
            "asc" | "desc" => {
                let conn = open_connection()?;
                map_settings_result(crate::db::set_setting_string(
                    &conn,
                    "sortDirection",
                    &value,
                ))
            }
            _ => invalid_input("invalid sort direction"),
        }
    })())
}

#[tauri::command]
pub fn load_sort_direction() -> ApiResult<Option<String>> {
    map_api_result((|| -> SettingsResult<Option<String>> {
        let conn = open_connection()?;
        let value = map_settings_result(crate::db::get_setting_string(&conn, "sortDirection"))?;
        Ok(match value.as_deref() {
            Some("asc") | Some("desc") => value,
            _ => None,
        })
    })())
}

#[tauri::command]
pub fn store_archive_name(value: String) -> ApiResult<()> {
    map_api_result((|| -> SettingsResult<()> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return invalid_input("archive name cannot be empty");
        }
        let normalized = trimmed.strip_suffix(".zip").unwrap_or(trimmed).to_string();
        let conn = open_connection()?;
        map_settings_result(crate::db::set_setting_string(
            &conn,
            "archiveName",
            &normalized,
        ))
    })())
}

#[tauri::command]
pub fn load_archive_name() -> ApiResult<Option<String>> {
    map_api_result((|| -> SettingsResult<Option<String>> {
        let conn = open_connection()?;
        Ok(
            map_settings_result(crate::db::get_setting_string(&conn, "archiveName"))?
                .map(|value| value.strip_suffix(".zip").unwrap_or(&value).to_string()),
        )
    })())
}

#[tauri::command]
pub fn store_density(value: String) -> ApiResult<()> {
    map_api_result((|| -> SettingsResult<()> {
        match value.as_str() {
            "cozy" | "compact" => {
                let conn = open_connection()?;
                map_settings_result(crate::db::set_setting_string(&conn, "density", &value))
            }
            _ => invalid_input("invalid density"),
        }
    })())
}

#[tauri::command]
pub fn load_density() -> ApiResult<Option<String>> {
    map_api_result((|| -> SettingsResult<Option<String>> {
        let conn = open_connection()?;
        let value = map_settings_result(crate::db::get_setting_string(&conn, "density"))?;
        Ok(match value.as_deref() {
            Some("cozy") | Some("compact") => value,
            _ => None,
        })
    })())
}

#[tauri::command]
pub fn store_archive_level(value: i64) -> ApiResult<()> {
    map_api_result((|| -> SettingsResult<()> {
        if !(0..=9).contains(&value) {
            return invalid_input("archive level must be 0-9");
        }
        let conn = open_connection()?;
        map_settings_result(crate::db::set_setting_string(
            &conn,
            "archiveLevel",
            &value.to_string(),
        ))
    })())
}

#[tauri::command]
pub fn load_archive_level() -> ApiResult<Option<i64>> {
    map_api_result((|| -> SettingsResult<Option<i64>> {
        let conn = open_connection()?;
        load_bounded_i64_setting(&conn, "archiveLevel", 0..=9)
    })())
}

#[tauri::command]
pub fn store_open_dest_after_extract(value: bool) -> ApiResult<()> {
    map_api_result((|| -> SettingsResult<()> {
        let conn = open_connection()?;
        map_settings_result(crate::db::set_setting_bool(
            &conn,
            "openDestAfterExtract",
            value,
        ))
    })())
}

#[tauri::command]
pub fn load_open_dest_after_extract() -> ApiResult<Option<bool>> {
    map_api_result((|| -> SettingsResult<Option<bool>> {
        let conn = open_connection()?;
        map_settings_result(crate::db::get_setting_bool(&conn, "openDestAfterExtract"))
    })())
}

#[tauri::command]
pub fn store_ffmpeg_path(value: String) -> ApiResult<()> {
    map_api_result((|| -> SettingsResult<()> {
        let trimmed = value.trim();
        let conn = open_connection()?;
        map_settings_result(crate::db::set_setting_string(&conn, "ffmpegPath", trimmed))
    })())
}

#[tauri::command]
pub fn load_ffmpeg_path() -> ApiResult<Option<String>> {
    map_api_result((|| -> SettingsResult<Option<String>> {
        let conn = open_connection()?;
        map_settings_result(crate::db::get_setting_string(&conn, "ffmpegPath"))
    })())
}

#[tauri::command]
pub fn store_thumb_cache_mb(value: i64) -> ApiResult<()> {
    map_api_result((|| -> SettingsResult<()> {
        if !(50..=1000).contains(&value) {
            return invalid_input("thumb cache must be 50-1000 MB");
        }
        let conn = open_connection()?;
        map_settings_result(crate::db::set_setting_string(
            &conn,
            "thumbCacheMb",
            &value.to_string(),
        ))
    })())
}

#[tauri::command]
pub fn load_thumb_cache_mb() -> ApiResult<Option<i64>> {
    map_api_result((|| -> SettingsResult<Option<i64>> {
        let conn = open_connection()?;
        load_bounded_i64_setting(&conn, "thumbCacheMb", 50..=1000)
    })())
}

#[tauri::command]
pub fn store_mounts_poll_ms(value: i64) -> ApiResult<()> {
    map_api_result((|| -> SettingsResult<()> {
        if !(500..=10000).contains(&value) {
            return invalid_input("mounts poll must be 500-10000 ms");
        }
        let conn = open_connection()?;
        map_settings_result(crate::db::set_setting_string(
            &conn,
            "mountsPollMs",
            &value.to_string(),
        ))
    })())
}

#[tauri::command]
pub fn load_mounts_poll_ms() -> ApiResult<Option<i64>> {
    map_api_result((|| -> SettingsResult<Option<i64>> {
        let conn = open_connection()?;
        load_bounded_i64_setting(&conn, "mountsPollMs", 500..=10000)
    })())
}

#[tauri::command]
pub fn store_video_thumbs(value: bool) -> ApiResult<()> {
    map_api_result((|| -> SettingsResult<()> {
        let conn = open_connection()?;
        map_settings_result(crate::db::set_setting_bool(&conn, "videoThumbs", value))
    })())
}

#[tauri::command]
pub fn load_video_thumbs() -> ApiResult<Option<bool>> {
    map_api_result((|| -> SettingsResult<Option<bool>> {
        let conn = open_connection()?;
        map_settings_result(crate::db::get_setting_bool(&conn, "videoThumbs"))
    })())
}

#[tauri::command]
pub fn store_hardware_acceleration(value: bool) -> ApiResult<()> {
    map_api_result((|| -> SettingsResult<()> {
        let conn = open_connection()?;
        map_settings_result(crate::db::set_setting_bool(
            &conn,
            "hardwareAcceleration",
            value,
        ))
    })())
}

#[tauri::command]
pub fn load_hardware_acceleration() -> ApiResult<Option<bool>> {
    map_api_result((|| -> SettingsResult<Option<bool>> {
        let conn = open_connection()?;
        map_settings_result(crate::db::get_setting_bool(&conn, "hardwareAcceleration"))
    })())
}

#[tauri::command]
pub fn store_scrollbar_width(value: i64) -> ApiResult<()> {
    map_api_result((|| -> SettingsResult<()> {
        if !(6..=16).contains(&value) {
            return invalid_input("scrollbar width must be 6-16 px");
        }
        let conn = open_connection()?;
        map_settings_result(crate::db::set_setting_string(
            &conn,
            "scrollbarWidth",
            &value.to_string(),
        ))
    })())
}

#[tauri::command]
pub fn load_scrollbar_width() -> ApiResult<Option<i64>> {
    map_api_result((|| -> SettingsResult<Option<i64>> {
        let conn = open_connection()?;
        load_bounded_i64_setting(&conn, "scrollbarWidth", 6..=16)
    })())
}

#[tauri::command]
pub fn store_rclone_path(value: String) -> ApiResult<()> {
    map_api_result((|| -> SettingsResult<()> {
        let conn = open_connection()?;
        let normalized = value.trim();
        map_settings_result(crate::db::set_setting_string(
            &conn,
            "rclonePath",
            normalized,
        ))?;
        crate::commands::cloud::invalidate_cloud_caches_for_backend_change();
        Ok(())
    })())
}

#[tauri::command]
pub fn load_rclone_path() -> ApiResult<Option<String>> {
    map_api_result((|| -> SettingsResult<Option<String>> {
        let conn = open_connection()?;
        map_settings_result(crate::db::get_setting_string(&conn, "rclonePath"))
    })())
}

#[tauri::command]
pub fn store_log_level(value: String) -> ApiResult<()> {
    map_api_result((|| -> SettingsResult<()> {
        let normalized = match normalize_log_level(&value) {
            Some(level) => level,
            None => return invalid_input("invalid log level"),
        };
        let conn = open_connection()?;
        map_settings_result(crate::db::set_setting_string(
            &conn,
            "logLevel",
            normalized,
        ))?;
        crate::apply_runtime_log_level(normalized)
            .map_err(|error| SettingsError::new(error::SettingsErrorCode::UnknownError, error))?;
        Ok(())
    })())
}

#[tauri::command]
pub fn load_log_level() -> ApiResult<Option<String>> {
    map_api_result((|| -> SettingsResult<Option<String>> {
        let conn = open_connection()?;
        let value = map_settings_result(crate::db::get_setting_string(&conn, "logLevel"))?;
        Ok(value
            .as_deref()
            .and_then(normalize_log_level)
            .map(|level| level.to_string()))
    })())
}

#[tauri::command]
pub fn store_double_click_ms(value: i64) -> ApiResult<()> {
    map_api_result((|| -> SettingsResult<()> {
        if !(150..=600).contains(&value) {
            return invalid_input("double click speed must be 150-600 ms");
        }
        let conn = open_connection()?;
        map_settings_result(crate::db::set_setting_string(
            &conn,
            "doubleClickMs",
            &value.to_string(),
        ))
    })())
}

#[tauri::command]
pub fn load_double_click_ms() -> ApiResult<Option<i64>> {
    map_api_result((|| -> SettingsResult<Option<i64>> {
        let conn = open_connection()?;
        load_bounded_i64_setting(&conn, "doubleClickMs", 150..=600)
    })())
}

#[cfg(test)]
mod tests {
    use super::store_rclone_path;
    use crate::commands::cloud::{
        cloud_dir_listing_cache_contains_for_tests,
        cloud_remote_discovery_cache_is_populated_for_tests,
        path::CloudPath,
        store_cloud_dir_listing_cache_entry_for_tests,
        store_cloud_remote_discovery_cache_entry_for_tests,
        types::{CloudCapabilities, CloudProviderKind, CloudRemote},
    };
    use std::{
        ffi::OsString,
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    struct TempDataHomeGuard {
        previous: Option<OsString>,
        dir: PathBuf,
    }

    impl Drop for TempDataHomeGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => std::env::set_var("XDG_DATA_HOME", value),
                None => std::env::remove_var("XDG_DATA_HOME"),
            }
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    fn temp_data_home_guard() -> TempDataHomeGuard {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        let dir = std::env::temp_dir().join(format!(
            "browsey-settings-test-data-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).expect("create temp data dir");
        let previous = std::env::var_os("XDG_DATA_HOME");
        std::env::set_var("XDG_DATA_HOME", &dir);
        TempDataHomeGuard { previous, dir }
    }

    #[test]
    fn store_rclone_path_invalidates_cloud_caches() {
        let _data_home = temp_data_home_guard();
        let path = CloudPath::parse("rclone://work/docs").expect("cloud path");
        store_cloud_remote_discovery_cache_entry_for_tests(vec![CloudRemote {
            id: "work".to_string(),
            label: "Work".to_string(),
            provider: CloudProviderKind::Onedrive,
            root_path: "rclone://work".to_string(),
            capabilities: CloudCapabilities::v1_core_rw(),
        }]);
        store_cloud_dir_listing_cache_entry_for_tests(&path, Vec::new());

        assert!(cloud_remote_discovery_cache_is_populated_for_tests());
        assert!(cloud_dir_listing_cache_contains_for_tests(&path));

        store_rclone_path("/usr/bin/rclone-does-not-exist".to_string()).expect("store rclone path");

        assert!(!cloud_remote_discovery_cache_is_populated_for_tests());
        assert!(!cloud_dir_listing_cache_contains_for_tests(&path));
    }
}
