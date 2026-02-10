use crate::keymap::ShortcutBinding;

#[tauri::command]
pub fn load_shortcuts() -> Result<Vec<ShortcutBinding>, String> {
    let conn = crate::db::open()?;
    crate::keymap::load_shortcuts(&conn)
}

#[tauri::command]
pub fn set_shortcut_binding(
    command_id: String,
    accelerator: String,
) -> Result<Vec<ShortcutBinding>, String> {
    let conn = crate::db::open()?;
    crate::keymap::set_shortcut_binding(&conn, &command_id, &accelerator)
}

#[tauri::command]
pub fn reset_shortcut_binding(command_id: String) -> Result<Vec<ShortcutBinding>, String> {
    let conn = crate::db::open()?;
    crate::keymap::reset_shortcut_binding(&conn, &command_id)
}

#[tauri::command]
pub fn reset_all_shortcuts() -> Result<Vec<ShortcutBinding>, String> {
    let conn = crate::db::open()?;
    crate::keymap::reset_all_shortcuts(&conn)
}
