use super::error::{map_api_result, map_external_result, FsError, FsErrorCode, FsResult};
use super::{should_abort_fs_op, CancelState, DeleteProgressPayload, UndoState};
use crate::{
    errors::api_error::ApiResult,
    fs_utils::sanitize_path_nofollow,
    path_guard::{
        ensure_existing_dir_nonsymlink, ensure_existing_path_nonsymlink,
        ensure_no_symlink_components_existing_prefix,
    },
    runtime_lifecycle,
    undo::{
        assert_path_snapshot, move_with_fallback, run_actions, snapshot_existing_path,
        temp_backup_path, Action, Direction,
    },
};
use std::{
    fs,
    path::Path,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

#[tauri::command]
pub fn delete_entry(path: String, state: tauri::State<UndoState>) -> ApiResult<()> {
    map_api_result(delete_entry_impl(path, state))
}

fn delete_entry_impl(path: String, state: tauri::State<UndoState>) -> FsResult<()> {
    let pb = sanitize_path_nofollow(&path, true).map_err(FsError::from_external_message)?;
    let action = delete_with_backup(&pb)?;
    let _ = state.record_applied(action);
    Ok(())
}

fn delete_with_backup(path: &Path) -> FsResult<Action> {
    map_external_result(ensure_existing_path_nonsymlink(path))?;
    let src_snapshot = map_external_result(snapshot_existing_path(path))?;
    let backup = temp_backup_path(path);
    if let Some(parent) = path.parent() {
        map_external_result(ensure_existing_dir_nonsymlink(parent))?;
    }
    if let Some(parent) = backup.parent() {
        map_external_result(ensure_no_symlink_components_existing_prefix(parent))?;
        fs::create_dir_all(parent).map_err(|e| {
            FsError::from_external_message(format!(
                "Failed to create backup dir {}: {e}",
                parent.display()
            ))
        })?;
        map_external_result(ensure_existing_dir_nonsymlink(parent))?;
    }
    // Use the same robust move primitive as undo (copy+delete fallback).
    map_external_result(assert_path_snapshot(path, &src_snapshot))?;
    map_external_result(move_with_fallback(path, &backup))?;
    Ok(Action::Delete {
        path: path.to_path_buf(),
        backup,
    })
}

fn emit_delete_progress(
    app: &tauri::AppHandle,
    event: Option<&String>,
    done: u64,
    total: u64,
    finished: bool,
    last_emit: &mut Instant,
) {
    if let Some(evt) = event {
        let now = Instant::now();
        if finished || now.duration_since(*last_emit) >= Duration::from_millis(100) {
            let payload = DeleteProgressPayload {
                bytes: done,
                total,
                finished,
            };
            let _ = runtime_lifecycle::emit_if_running(app, evt, payload);
            *last_emit = now;
        }
    }
}

fn delete_entries_blocking(
    app: tauri::AppHandle,
    paths: Vec<String>,
    progress_event: Option<String>,
    undo: UndoState,
    cancel: Option<&AtomicBool>,
) -> FsResult<()> {
    if paths.is_empty() {
        return Ok(());
    }
    let total = paths.len() as u64;
    let mut done = 0u64;
    let mut last_emit = Instant::now();
    let mut performed: Vec<Action> = Vec::with_capacity(paths.len());
    for raw in paths {
        if should_abort_fs_op(&app, cancel) {
            if !performed.is_empty() {
                let mut rollback = performed.clone();
                let _ = run_actions(&mut rollback, Direction::Backward);
            }
            emit_delete_progress(
                &app,
                progress_event.as_ref(),
                done,
                total,
                true,
                &mut last_emit,
            );
            return Err(FsError::new(FsErrorCode::Cancelled, "Delete cancelled"));
        }
        let path = sanitize_path_nofollow(&raw, true).map_err(FsError::from_external_message)?;
        match delete_with_backup(&path) {
            Ok(action) => {
                performed.push(action);
                done = done.saturating_add(1);
                emit_delete_progress(
                    &app,
                    progress_event.as_ref(),
                    done,
                    total,
                    false,
                    &mut last_emit,
                );
            }
            Err(err) => {
                if !performed.is_empty() {
                    let mut rollback = performed.clone();
                    if let Err(rb_err) = run_actions(&mut rollback, Direction::Backward) {
                        return Err(FsError::from_external_message(format!(
                            "Failed to delete {}: {}; rollback also failed: {}",
                            path.display(),
                            err,
                            rb_err
                        )));
                    }
                }
                return Err(FsError::from_external_message(format!(
                    "Failed to delete {}: {}",
                    path.display(),
                    err
                )));
            }
        }
    }
    if !performed.is_empty() {
        let recorded = if performed.len() == 1 {
            performed.pop().unwrap()
        } else {
            Action::Batch(performed)
        };
        let _ = undo.record_applied(recorded);
    }
    emit_delete_progress(
        &app,
        progress_event.as_ref(),
        done,
        total,
        true,
        &mut last_emit,
    );
    Ok(())
}

#[tauri::command]
pub async fn delete_entries(
    app: tauri::AppHandle,
    paths: Vec<String>,
    progress_event: Option<String>,
    undo: tauri::State<'_, UndoState>,
    cancel: tauri::State<'_, CancelState>,
) -> ApiResult<()> {
    map_api_result(delete_entries_impl(app, paths, progress_event, undo, cancel).await)
}

async fn delete_entries_impl(
    app: tauri::AppHandle,
    paths: Vec<String>,
    progress_event: Option<String>,
    undo: tauri::State<'_, UndoState>,
    cancel: tauri::State<'_, CancelState>,
) -> FsResult<()> {
    let undo = undo.inner().clone();
    let cancel_state = cancel.inner().clone();
    let task = tauri::async_runtime::spawn_blocking(move || -> FsResult<()> {
        let cancel_guard = progress_event
            .as_ref()
            .map(|id| cancel_state.register(id.clone()))
            .transpose()
            .map_err(|error| {
                FsError::new(
                    FsErrorCode::TaskFailed,
                    format!("Failed to register cancel: {error}"),
                )
            })?;
        let cancel_token = cancel_guard.as_ref().map(|g| g.token());
        delete_entries_blocking(app, paths, progress_event, undo, cancel_token.as_deref())
    });
    match task.await {
        Ok(result) => result,
        Err(error) => Err(FsError::new(
            FsErrorCode::TaskFailed,
            format!("Delete task failed: {error}"),
        )),
    }
}
