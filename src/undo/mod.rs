mod backup;
mod copy_recovery;
mod engine;
mod error;
mod nofollow;
mod path_checks;
mod path_ops;
mod recovery_notice;
mod security;
mod storage;
mod types;

use crate::errors::api_error::ApiResult;

pub use backup::{cleanup_stale_backups, temp_backup_path, BackupProtection};
pub use error::{UndoError, UndoErrorCode, UndoResult};
#[cfg(test)]
pub(crate) use path_ops::move_by_copy_delete_noreplace;
pub use path_ops::move_with_fallback;
#[cfg(all(unix, target_os = "linux"))]
pub(crate) use security::set_unix_mode_nofollow;
pub(crate) use security::{apply_ownership, apply_permissions, set_ownership_nofollow};
pub use security::{ownership_snapshot, permissions_snapshot};
pub use storage::UndoStorageSummary;
pub(crate) use types::PathSnapshot;
pub use types::{
    Action, CopyReceipt, Direction, OwnershipSnapshot, PermissionsSnapshot, UndoManager, UndoState,
};

pub(crate) use engine::{finalize_action, run_actions, run_rollback_actions};
pub(crate) use nofollow::rename_nofollow_io;
pub(crate) use path_checks::{assert_path_snapshot, snapshot_existing_path};
pub(crate) use path_ops::{copy_entry, delete_entry_path, is_destination_exists_error};

#[cfg(test)]
mod tests;

#[tauri::command]
pub async fn inspect_undo_storage() -> ApiResult<UndoStorageSummary> {
    let result = tauri::async_runtime::spawn_blocking(storage::inspect_storage)
        .await
        .map_err(|error| {
            UndoError::new(
                UndoErrorCode::IoError,
                format!("Undo storage worker failed: {error}"),
            )
        })
        .and_then(|result| result);
    error::map_api_result(result)
}

#[tauri::command]
pub async fn undo_action(state: tauri::State<'_, UndoState>) -> ApiResult<()> {
    error::map_api_result(
        execute_history_operation(state.inner().clone(), Direction::Backward).await,
    )
}

#[tauri::command]
pub async fn redo_action(state: tauri::State<'_, UndoState>) -> ApiResult<()> {
    error::map_api_result(
        execute_history_operation(state.inner().clone(), Direction::Forward).await,
    )
}

async fn execute_history_operation(state: UndoState, direction: Direction) -> UndoResult<()> {
    // Copy recovery may write large trees. Keep blocking filesystem work off
    // the webview/event-loop thread; the manager still serializes history.
    tauri::async_runtime::spawn_blocking(move || match direction {
        Direction::Backward => state.undo(),
        Direction::Forward => state.redo(),
    })
    .await
    .map_err(|error| {
        UndoError::new(
            UndoErrorCode::IoError,
            format!("Undo/redo worker failed: {error}"),
        )
    })?
}
