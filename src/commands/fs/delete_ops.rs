use super::{should_abort_fs_op, CancelState, DeleteProgressPayload, UndoState};
use crate::{
    commands::path_guard::{
        ensure_existing_dir_nonsymlink, ensure_existing_path_nonsymlink,
        ensure_no_symlink_components_existing_prefix,
    },
    fs_utils::sanitize_path_nofollow,
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
pub fn delete_entry(path: String, state: tauri::State<UndoState>) -> Result<(), String> {
    let pb = sanitize_path_nofollow(&path, true)?;
    let action = delete_with_backup(&pb)?;
    let _ = state.record_applied(action);
    Ok(())
}

fn delete_with_backup(path: &Path) -> Result<Action, String> {
    ensure_existing_path_nonsymlink(path)?;
    let src_snapshot = snapshot_existing_path(path)?;
    let backup = temp_backup_path(path);
    if let Some(parent) = path.parent() {
        ensure_existing_dir_nonsymlink(parent)?;
    }
    if let Some(parent) = backup.parent() {
        ensure_no_symlink_components_existing_prefix(parent)?;
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create backup dir {}: {e}", parent.display()))?;
        ensure_existing_dir_nonsymlink(parent)?;
    }
    // Use the same robust move primitive as undo (copy+delete fallback).
    assert_path_snapshot(path, &src_snapshot)?;
    move_with_fallback(path, &backup)?;
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
) -> Result<(), String> {
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
            return Err("Delete cancelled".into());
        }
        let path = sanitize_path_nofollow(&raw, true)?;
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
                        return Err(format!(
                            "Failed to delete {}: {}; rollback also failed: {}",
                            path.display(),
                            err,
                            rb_err
                        ));
                    }
                }
                return Err(format!("Failed to delete {}: {}", path.display(), err));
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
) -> Result<(), String> {
    let undo = undo.inner().clone();
    let cancel_state = cancel.inner().clone();
    let task = tauri::async_runtime::spawn_blocking(move || {
        let cancel_guard = progress_event
            .as_ref()
            .map(|id| cancel_state.register(id.clone()))
            .transpose()?;
        let cancel_token = cancel_guard.as_ref().map(|g| g.token());
        delete_entries_blocking(app, paths, progress_event, undo, cancel_token.as_deref())
    });
    task.await.map_err(|e| format!("Delete task failed: {e}"))?
}
