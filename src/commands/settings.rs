//! Persisted UI settings such as column widths.

use crate::db::{load_column_widths, save_column_widths};

#[tauri::command]
pub fn store_column_widths(widths: Vec<f64>) -> Result<(), String> {
    let conn = crate::db::open()?;
    save_column_widths(&conn, &widths)
}

#[tauri::command]
pub fn load_saved_column_widths() -> Result<Option<Vec<f64>>, String> {
    let conn = crate::db::open()?;
    load_column_widths(&conn)
}

#[tauri::command]
pub fn store_show_hidden(value: bool) -> Result<(), String> {
    let conn = crate::db::open()?;
    crate::db::set_setting_bool(&conn, "showHidden", value)
}

#[tauri::command]
pub fn load_show_hidden() -> Result<Option<bool>, String> {
    let conn = crate::db::open()?;
    crate::db::get_setting_bool(&conn, "showHidden")
}

#[tauri::command]
pub fn store_hidden_files_last(value: bool) -> Result<(), String> {
    let conn = crate::db::open()?;
    crate::db::set_setting_bool(&conn, "hiddenFilesLast", value)
}

#[tauri::command]
pub fn load_hidden_files_last() -> Result<Option<bool>, String> {
    let conn = crate::db::open()?;
    crate::db::get_setting_bool(&conn, "hiddenFilesLast")
}

#[tauri::command]
pub fn store_folders_first(value: bool) -> Result<(), String> {
    let conn = crate::db::open()?;
    crate::db::set_setting_bool(&conn, "foldersFirst", value)
}

#[tauri::command]
pub fn load_folders_first() -> Result<Option<bool>, String> {
    let conn = crate::db::open()?;
    crate::db::get_setting_bool(&conn, "foldersFirst")
}

#[tauri::command]
pub fn store_default_view(value: String) -> Result<(), String> {
    let conn = crate::db::open()?;
    crate::db::set_setting_string(&conn, "defaultView", &value)
}

#[tauri::command]
pub fn load_default_view() -> Result<Option<String>, String> {
    let conn = crate::db::open()?;
    crate::db::get_setting_string(&conn, "defaultView")
}

#[tauri::command]
pub fn store_start_dir(value: String) -> Result<(), String> {
    let conn = crate::db::open()?;
    crate::db::set_setting_string(&conn, "startDir", &value)
}

#[tauri::command]
pub fn load_start_dir() -> Result<Option<String>, String> {
    let conn = crate::db::open()?;
    crate::db::get_setting_string(&conn, "startDir")
}

#[tauri::command]
pub fn store_confirm_delete(value: bool) -> Result<(), String> {
    let conn = crate::db::open()?;
    crate::db::set_setting_bool(&conn, "confirmDelete", value)
}

#[tauri::command]
pub fn load_confirm_delete() -> Result<Option<bool>, String> {
    let conn = crate::db::open()?;
    crate::db::get_setting_bool(&conn, "confirmDelete")
}

#[tauri::command]
pub fn store_sort_field(value: String) -> Result<(), String> {
    match value.as_str() {
        "name" | "type" | "modified" | "size" => {
            let conn = crate::db::open()?;
            crate::db::set_setting_string(&conn, "sortField", &value)
        }
        _ => Err("invalid sort field".into()),
    }
}

#[tauri::command]
pub fn load_sort_field() -> Result<Option<String>, String> {
    let conn = crate::db::open()?;
    let value = crate::db::get_setting_string(&conn, "sortField")?;
    Ok(match value.as_deref() {
        Some("name") | Some("type") | Some("modified") | Some("size") => value,
        _ => None,
    })
}

#[tauri::command]
pub fn store_sort_direction(value: String) -> Result<(), String> {
    match value.as_str() {
        "asc" | "desc" => {
            let conn = crate::db::open()?;
            crate::db::set_setting_string(&conn, "sortDirection", &value)
        }
        _ => Err("invalid sort direction".into()),
    }
}

#[tauri::command]
pub fn load_sort_direction() -> Result<Option<String>, String> {
    let conn = crate::db::open()?;
    let value = crate::db::get_setting_string(&conn, "sortDirection")?;
    Ok(match value.as_deref() {
        Some("asc") | Some("desc") => value,
        _ => None,
    })
}

#[tauri::command]
pub fn store_archive_name(value: String) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("archive name cannot be empty".into());
    }
    let normalized = trimmed.strip_suffix(".zip").unwrap_or(trimmed).to_string();
    let conn = crate::db::open()?;
    crate::db::set_setting_string(&conn, "archiveName", &normalized)
}

#[tauri::command]
pub fn load_archive_name() -> Result<Option<String>, String> {
    let conn = crate::db::open()?;
    Ok(crate::db::get_setting_string(&conn, "archiveName")?
        .map(|v| v.strip_suffix(".zip").unwrap_or(&v).to_string()))
}

#[tauri::command]
pub fn store_density(value: String) -> Result<(), String> {
    match value.as_str() {
        "cozy" | "compact" => {
            let conn = crate::db::open()?;
            crate::db::set_setting_string(&conn, "density", &value)
        }
        _ => Err("invalid density".into()),
    }
}

#[tauri::command]
pub fn load_density() -> Result<Option<String>, String> {
    let conn = crate::db::open()?;
    let value = crate::db::get_setting_string(&conn, "density")?;
    Ok(match value.as_deref() {
        Some("cozy") | Some("compact") => value,
        _ => None,
    })
}

#[tauri::command]
pub fn store_archive_level(value: i64) -> Result<(), String> {
    if !(0..=9).contains(&value) {
        return Err("archive level must be 0-9".into());
    }
    let conn = crate::db::open()?;
    crate::db::set_setting_string(&conn, "archiveLevel", &value.to_string())
}

#[tauri::command]
pub fn load_archive_level() -> Result<Option<i64>, String> {
    let conn = crate::db::open()?;
    if let Some(s) = crate::db::get_setting_string(&conn, "archiveLevel")? {
        if let Ok(n) = s.parse::<i64>() {
            if (0..=9).contains(&n) {
                return Ok(Some(n));
            }
        }
    }
    Ok(None)
}

#[tauri::command]
pub fn store_open_dest_after_extract(value: bool) -> Result<(), String> {
    let conn = crate::db::open()?;
    crate::db::set_setting_bool(&conn, "openDestAfterExtract", value)
}

#[tauri::command]
pub fn load_open_dest_after_extract() -> Result<Option<bool>, String> {
    let conn = crate::db::open()?;
    crate::db::get_setting_bool(&conn, "openDestAfterExtract")
}

#[tauri::command]
pub fn store_ffmpeg_path(value: String) -> Result<(), String> {
    let trimmed = value.trim();
    let conn = crate::db::open()?;
    // Empty string means auto-detect; still store so the UI can show what the user chose.
    crate::db::set_setting_string(&conn, "ffmpegPath", trimmed)
}

#[tauri::command]
pub fn load_ffmpeg_path() -> Result<Option<String>, String> {
    let conn = crate::db::open()?;
    Ok(crate::db::get_setting_string(&conn, "ffmpegPath")?)
}

#[tauri::command]
pub fn store_thumb_cache_mb(value: i64) -> Result<(), String> {
    if !(50..=1000).contains(&value) {
        return Err("thumb cache must be 50-1000 MB".into());
    }
    let conn = crate::db::open()?;
    crate::db::set_setting_string(&conn, "thumbCacheMb", &value.to_string())
}

#[tauri::command]
pub fn load_thumb_cache_mb() -> Result<Option<i64>, String> {
    let conn = crate::db::open()?;
    if let Some(s) = crate::db::get_setting_string(&conn, "thumbCacheMb")? {
        if let Ok(n) = s.parse::<i64>() {
            if (50..=1000).contains(&n) {
                return Ok(Some(n));
            }
        }
    }
    Ok(None)
}

#[tauri::command]
pub fn store_video_thumbs(value: bool) -> Result<(), String> {
    let conn = crate::db::open()?;
    crate::db::set_setting_bool(&conn, "videoThumbs", value)
}

#[tauri::command]
pub fn load_video_thumbs() -> Result<Option<bool>, String> {
    let conn = crate::db::open()?;
    crate::db::get_setting_bool(&conn, "videoThumbs")
}
