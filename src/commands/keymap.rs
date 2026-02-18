use crate::keymap::ShortcutBinding;
use crate::{errors::api_error::ApiResult, keymap as keymap_core};
use error::{map_api_result, KeymapError, KeymapErrorCode, KeymapResult};

mod error;

#[tauri::command]
pub fn load_shortcuts() -> ApiResult<Vec<ShortcutBinding>> {
    map_api_result(load_shortcuts_impl())
}

fn load_shortcuts_impl() -> KeymapResult<Vec<ShortcutBinding>> {
    let conn = crate::db::open().map_err(|error| {
        KeymapError::new(
            KeymapErrorCode::DatabaseOpenFailed,
            format!("Failed to open shortcuts database: {error}"),
        )
    })?;
    keymap_core::load_shortcuts(&conn)
        .map_err(|error| KeymapError::from_external_message(error.to_string()))
}

#[tauri::command]
pub fn set_shortcut_binding(
    command_id: String,
    accelerator: String,
) -> ApiResult<Vec<ShortcutBinding>> {
    map_api_result(set_shortcut_binding_impl(command_id, accelerator))
}

fn set_shortcut_binding_impl(
    command_id: String,
    accelerator: String,
) -> KeymapResult<Vec<ShortcutBinding>> {
    let conn = crate::db::open().map_err(|error| {
        KeymapError::new(
            KeymapErrorCode::DatabaseOpenFailed,
            format!("Failed to open shortcuts database: {error}"),
        )
    })?;
    keymap_core::set_shortcut_binding(&conn, &command_id, &accelerator)
        .map_err(|error| KeymapError::from_external_message(error.to_string()))
}

#[tauri::command]
pub fn reset_shortcut_binding(command_id: String) -> ApiResult<Vec<ShortcutBinding>> {
    map_api_result(reset_shortcut_binding_impl(command_id))
}

fn reset_shortcut_binding_impl(command_id: String) -> KeymapResult<Vec<ShortcutBinding>> {
    let conn = crate::db::open().map_err(|error| {
        KeymapError::new(
            KeymapErrorCode::DatabaseOpenFailed,
            format!("Failed to open shortcuts database: {error}"),
        )
    })?;
    keymap_core::reset_shortcut_binding(&conn, &command_id)
        .map_err(|error| KeymapError::from_external_message(error.to_string()))
}

#[tauri::command]
pub fn reset_all_shortcuts() -> ApiResult<Vec<ShortcutBinding>> {
    map_api_result(reset_all_shortcuts_impl())
}

fn reset_all_shortcuts_impl() -> KeymapResult<Vec<ShortcutBinding>> {
    let conn = crate::db::open().map_err(|error| {
        KeymapError::new(
            KeymapErrorCode::DatabaseOpenFailed,
            format!("Failed to open shortcuts database: {error}"),
        )
    })?;
    keymap_core::reset_all_shortcuts(&conn)
        .map_err(|error| KeymapError::from_external_message(error.to_string()))
}
