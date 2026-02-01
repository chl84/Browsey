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
