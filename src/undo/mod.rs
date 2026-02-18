mod backup;
mod engine;
mod nofollow;
mod path_checks;
mod path_ops;
mod security;
mod types;

pub use backup::{cleanup_stale_backups, temp_backup_path};
#[cfg(test)]
pub(crate) use path_ops::move_by_copy_delete_noreplace;
pub use path_ops::move_with_fallback;
#[cfg(all(unix, target_os = "linux"))]
pub(crate) use security::set_unix_mode_nofollow;
pub(crate) use security::{apply_ownership, apply_permissions, set_ownership_nofollow};
pub use security::{ownership_snapshot, permissions_snapshot};
pub(crate) use types::PathSnapshot;
pub use types::{
    Action, Direction, OwnershipSnapshot, PermissionsSnapshot, UndoManager, UndoState,
};

pub(crate) use engine::run_actions;
pub(crate) use path_checks::{assert_path_snapshot, snapshot_existing_path};
pub(crate) use path_ops::{copy_entry, delete_entry_path, is_destination_exists_error};

#[cfg(test)]
mod tests;

#[tauri::command]
pub fn undo_action(state: tauri::State<'_, UndoState>) -> Result<(), String> {
    state.undo()
}

#[tauri::command]
pub fn redo_action(state: tauri::State<'_, UndoState>) -> Result<(), String> {
    state.redo()
}
