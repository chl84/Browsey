//! Persisted UI settings such as column widths.

mod error;

use crate::{
    db::{load_column_widths, save_column_widths},
    errors::api_error::ApiResult,
};

use self::error::{map_api_result, SettingsError};

fn map_settings_result<T, E>(result: Result<T, E>) -> ApiResult<T>
where
    E: std::fmt::Display,
{
    map_api_result(result.map_err(|error| SettingsError::from_external_message(error.to_string())))
}

fn open_connection() -> ApiResult<rusqlite::Connection> {
    map_settings_result(crate::db::open())
}

fn invalid_input<T>(message: &'static str) -> ApiResult<T> {
    map_api_result(Err(SettingsError::invalid_input(message)))
}

#[tauri::command]
pub fn store_column_widths(widths: Vec<f64>) -> ApiResult<()> {
    let conn = open_connection()?;
    map_settings_result(save_column_widths(&conn, &widths))
}

#[tauri::command]
pub fn load_saved_column_widths() -> ApiResult<Option<Vec<f64>>> {
    let conn = open_connection()?;
    map_settings_result(load_column_widths(&conn))
}

#[tauri::command]
pub fn store_show_hidden(value: bool) -> ApiResult<()> {
    let conn = open_connection()?;
    map_settings_result(crate::db::set_setting_bool(&conn, "showHidden", value))
}

#[tauri::command]
pub fn load_show_hidden() -> ApiResult<Option<bool>> {
    let conn = open_connection()?;
    map_settings_result(crate::db::get_setting_bool(&conn, "showHidden"))
}

#[tauri::command]
pub fn store_hidden_files_last(value: bool) -> ApiResult<()> {
    let conn = open_connection()?;
    map_settings_result(crate::db::set_setting_bool(&conn, "hiddenFilesLast", value))
}

#[tauri::command]
pub fn load_hidden_files_last() -> ApiResult<Option<bool>> {
    let conn = open_connection()?;
    map_settings_result(crate::db::get_setting_bool(&conn, "hiddenFilesLast"))
}

#[tauri::command]
pub fn store_high_contrast(value: bool) -> ApiResult<()> {
    let conn = open_connection()?;
    map_settings_result(crate::db::set_setting_bool(&conn, "highContrast", value))
}

#[tauri::command]
pub fn load_high_contrast() -> ApiResult<Option<bool>> {
    let conn = open_connection()?;
    map_settings_result(crate::db::get_setting_bool(&conn, "highContrast"))
}

#[tauri::command]
pub fn store_folders_first(value: bool) -> ApiResult<()> {
    let conn = open_connection()?;
    map_settings_result(crate::db::set_setting_bool(&conn, "foldersFirst", value))
}

#[tauri::command]
pub fn load_folders_first() -> ApiResult<Option<bool>> {
    let conn = open_connection()?;
    map_settings_result(crate::db::get_setting_bool(&conn, "foldersFirst"))
}

#[tauri::command]
pub fn store_default_view(value: String) -> ApiResult<()> {
    let conn = open_connection()?;
    map_settings_result(crate::db::set_setting_string(&conn, "defaultView", &value))
}

#[tauri::command]
pub fn load_default_view() -> ApiResult<Option<String>> {
    let conn = open_connection()?;
    map_settings_result(crate::db::get_setting_string(&conn, "defaultView"))
}

#[tauri::command]
pub fn store_start_dir(value: String) -> ApiResult<()> {
    let conn = open_connection()?;
    map_settings_result(crate::db::set_setting_string(&conn, "startDir", &value))
}

#[tauri::command]
pub fn load_start_dir() -> ApiResult<Option<String>> {
    let conn = open_connection()?;
    map_settings_result(crate::db::get_setting_string(&conn, "startDir"))
}

#[tauri::command]
pub fn store_confirm_delete(value: bool) -> ApiResult<()> {
    let conn = open_connection()?;
    map_settings_result(crate::db::set_setting_bool(&conn, "confirmDelete", value))
}

#[tauri::command]
pub fn load_confirm_delete() -> ApiResult<Option<bool>> {
    let conn = open_connection()?;
    map_settings_result(crate::db::get_setting_bool(&conn, "confirmDelete"))
}

#[tauri::command]
pub fn store_sort_field(value: String) -> ApiResult<()> {
    match value.as_str() {
        "name" | "type" | "modified" | "size" => {
            let conn = open_connection()?;
            map_settings_result(crate::db::set_setting_string(&conn, "sortField", &value))
        }
        _ => invalid_input("invalid sort field"),
    }
}

#[tauri::command]
pub fn load_sort_field() -> ApiResult<Option<String>> {
    let conn = open_connection()?;
    let value = map_settings_result(crate::db::get_setting_string(&conn, "sortField"))?;
    Ok(match value.as_deref() {
        Some("name") | Some("type") | Some("modified") | Some("size") => value,
        _ => None,
    })
}

#[tauri::command]
pub fn store_sort_direction(value: String) -> ApiResult<()> {
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
}

#[tauri::command]
pub fn load_sort_direction() -> ApiResult<Option<String>> {
    let conn = open_connection()?;
    let value = map_settings_result(crate::db::get_setting_string(&conn, "sortDirection"))?;
    Ok(match value.as_deref() {
        Some("asc") | Some("desc") => value,
        _ => None,
    })
}

#[tauri::command]
pub fn store_archive_name(value: String) -> ApiResult<()> {
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
}

#[tauri::command]
pub fn load_archive_name() -> ApiResult<Option<String>> {
    let conn = open_connection()?;
    Ok(
        map_settings_result(crate::db::get_setting_string(&conn, "archiveName"))?
            .map(|v| v.strip_suffix(".zip").unwrap_or(&v).to_string()),
    )
}

#[tauri::command]
pub fn store_density(value: String) -> ApiResult<()> {
    match value.as_str() {
        "cozy" | "compact" => {
            let conn = open_connection()?;
            map_settings_result(crate::db::set_setting_string(&conn, "density", &value))
        }
        _ => invalid_input("invalid density"),
    }
}

#[tauri::command]
pub fn load_density() -> ApiResult<Option<String>> {
    let conn = open_connection()?;
    let value = map_settings_result(crate::db::get_setting_string(&conn, "density"))?;
    Ok(match value.as_deref() {
        Some("cozy") | Some("compact") => value,
        _ => None,
    })
}

#[tauri::command]
pub fn store_archive_level(value: i64) -> ApiResult<()> {
    if !(0..=9).contains(&value) {
        return invalid_input("archive level must be 0-9");
    }
    let conn = open_connection()?;
    map_settings_result(crate::db::set_setting_string(
        &conn,
        "archiveLevel",
        &value.to_string(),
    ))
}

#[tauri::command]
pub fn load_archive_level() -> ApiResult<Option<i64>> {
    let conn = open_connection()?;
    if let Some(s) = map_settings_result(crate::db::get_setting_string(&conn, "archiveLevel"))? {
        if let Ok(n) = s.parse::<i64>() {
            if (0..=9).contains(&n) {
                return Ok(Some(n));
            }
        }
    }
    Ok(None)
}

#[tauri::command]
pub fn store_open_dest_after_extract(value: bool) -> ApiResult<()> {
    let conn = open_connection()?;
    map_settings_result(crate::db::set_setting_bool(
        &conn,
        "openDestAfterExtract",
        value,
    ))
}

#[tauri::command]
pub fn load_open_dest_after_extract() -> ApiResult<Option<bool>> {
    let conn = open_connection()?;
    map_settings_result(crate::db::get_setting_bool(&conn, "openDestAfterExtract"))
}

#[tauri::command]
pub fn store_ffmpeg_path(value: String) -> ApiResult<()> {
    let trimmed = value.trim();
    let conn = open_connection()?;
    // Empty string means auto-detect; still store so the UI can show what the user chose.
    map_settings_result(crate::db::set_setting_string(&conn, "ffmpegPath", trimmed))
}

#[tauri::command]
pub fn load_ffmpeg_path() -> ApiResult<Option<String>> {
    let conn = open_connection()?;
    map_settings_result(crate::db::get_setting_string(&conn, "ffmpegPath"))
}

#[tauri::command]
pub fn store_thumb_cache_mb(value: i64) -> ApiResult<()> {
    if !(50..=1000).contains(&value) {
        return invalid_input("thumb cache must be 50-1000 MB");
    }
    let conn = open_connection()?;
    map_settings_result(crate::db::set_setting_string(
        &conn,
        "thumbCacheMb",
        &value.to_string(),
    ))
}

#[tauri::command]
pub fn load_thumb_cache_mb() -> ApiResult<Option<i64>> {
    let conn = open_connection()?;
    if let Some(s) = map_settings_result(crate::db::get_setting_string(&conn, "thumbCacheMb"))? {
        if let Ok(n) = s.parse::<i64>() {
            if (50..=1000).contains(&n) {
                return Ok(Some(n));
            }
        }
    }
    Ok(None)
}

#[tauri::command]
pub fn store_mounts_poll_ms(value: i64) -> ApiResult<()> {
    if !(500..=10000).contains(&value) {
        return invalid_input("mounts poll must be 500-10000 ms");
    }
    let conn = open_connection()?;
    map_settings_result(crate::db::set_setting_string(
        &conn,
        "mountsPollMs",
        &value.to_string(),
    ))
}

#[tauri::command]
pub fn load_mounts_poll_ms() -> ApiResult<Option<i64>> {
    let conn = open_connection()?;
    if let Some(s) = map_settings_result(crate::db::get_setting_string(&conn, "mountsPollMs"))? {
        if let Ok(n) = s.parse::<i64>() {
            if (500..=10000).contains(&n) {
                return Ok(Some(n));
            }
        }
    }
    Ok(None)
}

#[tauri::command]
pub fn store_video_thumbs(value: bool) -> ApiResult<()> {
    let conn = open_connection()?;
    map_settings_result(crate::db::set_setting_bool(&conn, "videoThumbs", value))
}

#[tauri::command]
pub fn load_video_thumbs() -> ApiResult<Option<bool>> {
    let conn = open_connection()?;
    map_settings_result(crate::db::get_setting_bool(&conn, "videoThumbs"))
}

#[tauri::command]
pub fn store_hardware_acceleration(value: bool) -> ApiResult<()> {
    let conn = open_connection()?;
    map_settings_result(crate::db::set_setting_bool(
        &conn,
        "hardwareAcceleration",
        value,
    ))
}

#[tauri::command]
pub fn load_hardware_acceleration() -> ApiResult<Option<bool>> {
    let conn = open_connection()?;
    map_settings_result(crate::db::get_setting_bool(&conn, "hardwareAcceleration"))
}

#[tauri::command]
pub fn store_scrollbar_width(value: i64) -> ApiResult<()> {
    if !(6..=16).contains(&value) {
        return invalid_input("scrollbar width must be 6-16 px");
    }
    let conn = open_connection()?;
    map_settings_result(crate::db::set_setting_string(
        &conn,
        "scrollbarWidth",
        &value.to_string(),
    ))
}

#[tauri::command]
pub fn load_scrollbar_width() -> ApiResult<Option<i64>> {
    let conn = open_connection()?;
    if let Some(s) = map_settings_result(crate::db::get_setting_string(&conn, "scrollbarWidth"))? {
        if let Ok(n) = s.parse::<i64>() {
            if (6..=16).contains(&n) {
                return Ok(Some(n));
            }
        }
    }
    Ok(None)
}

#[tauri::command]
pub fn store_double_click_ms(value: i64) -> ApiResult<()> {
    if !(150..=600).contains(&value) {
        return invalid_input("double click speed must be 150-600 ms");
    }
    let conn = open_connection()?;
    map_settings_result(crate::db::set_setting_string(
        &conn,
        "doubleClickMs",
        &value.to_string(),
    ))
}

#[tauri::command]
pub fn load_double_click_ms() -> ApiResult<Option<i64>> {
    let conn = open_connection()?;
    if let Some(s) = map_settings_result(crate::db::get_setting_string(&conn, "doubleClickMs"))? {
        if let Ok(n) = s.parse::<i64>() {
            if (150..=600).contains(&n) {
                return Ok(Some(n));
            }
        }
    }
    Ok(None)
}
