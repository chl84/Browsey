use super::super::{should_abort_fs_op, CancelState, DeleteProgressPayload, UndoState};
use super::{
    backend::{SystemTrashBackend, TrashBackend},
    listing::trash_item_path,
    staging::trash_delete_via_staged_rename,
};
use crate::{
    fs_utils::{check_no_symlink_components, sanitize_path_nofollow},
    runtime_lifecycle,
    undo::{
        assert_path_snapshot, copy_entry as undo_copy_entry, delete_entry_path as undo_delete_path,
        run_actions, snapshot_existing_path, temp_backup_path, Action, Direction, PathSnapshot,
    },
};
use ::trash::TrashItem;
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tracing::warn;

#[tauri::command]
pub async fn move_to_trash(
    path: String,
    app: tauri::AppHandle,
    undo: tauri::State<'_, UndoState>,
) -> Result<(), String> {
    let app_handle = app.clone();
    let undo_state = undo.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let action = move_single_to_trash(&path, &app_handle, true)?;
        let _ = undo_state.record_applied(action);
        Ok(())
    })
    .await
    .map_err(|e| format!("Move to trash task failed: {e}"))?
}

fn emit_trash_progress(
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

struct PreparedTrashMove {
    src: PathBuf,
    backup: PathBuf,
    src_snapshot: PathSnapshot,
    staged_src: Option<PathBuf>,
}

fn rollback_prepared_trash(prepared: &[PreparedTrashMove]) {
    let mut rollback: Vec<Action> = prepared
        .iter()
        .map(|p| Action::Delete {
            path: p.src.clone(),
            backup: p.backup.clone(),
        })
        .collect();
    let _ = run_actions(&mut rollback, Direction::Backward);
}

fn prepare_trash_move(raw: &str) -> Result<PreparedTrashMove, String> {
    let src = sanitize_path_nofollow(raw, true)?;
    check_no_symlink_components(&src)?;
    let src_snapshot = snapshot_existing_path(&src)?;

    // Backup into the central undo directory in case we cannot locate the trash item path later.
    let backup = temp_backup_path(&src);
    if let Some(parent) = backup.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create backup dir {}: {e}", parent.display()))?;
    }
    undo_copy_entry(&src, &backup)?;
    if let Err(err) = assert_path_snapshot(&src, &src_snapshot) {
        let _ = undo_delete_path(&backup);
        return Err(err);
    }

    Ok(PreparedTrashMove {
        src,
        backup,
        src_snapshot,
        staged_src: None,
    })
}

pub(super) fn move_to_trash_many_with_backend<B, FShouldAbort, FEmitProgress, FEmitChanged>(
    paths: Vec<String>,
    undo: UndoState,
    cancel: Option<&AtomicBool>,
    backend: &B,
    mut should_abort: FShouldAbort,
    mut emit_progress: FEmitProgress,
    mut emit_changed: FEmitChanged,
) -> Result<(), String>
where
    B: TrashBackend,
    FShouldAbort: FnMut(Option<&AtomicBool>) -> bool,
    FEmitProgress: FnMut(u64, u64, bool),
    FEmitChanged: FnMut(),
{
    let total = paths.len() as u64;
    if total == 0 {
        emit_progress(0, 0, true);
        return Ok(());
    }
    // Capture current trash contents once to avoid O(n^2) directory scans.
    let before_ids: HashSet<OsString> = backend
        .list_items()?
        .into_iter()
        .map(|item| item.id)
        .collect();

    let mut prepared: Vec<PreparedTrashMove> = Vec::with_capacity(paths.len());
    let mut done: u64 = 0;
    for path in paths {
        if should_abort(cancel) {
            rollback_prepared_trash(&prepared);
            emit_progress(done, total, true);
            return Err("Move to trash cancelled".into());
        }
        match prepare_trash_move(&path) {
            Ok(mut prep) => {
                match trash_delete_via_staged_rename(&prep.src, &prep.src_snapshot, backend) {
                    Ok(staged_src) => {
                        prep.staged_src = Some(staged_src);
                        done = done.saturating_add(1);
                        emit_progress(done, total, done == total);
                        prepared.push(prep);
                    }
                    Err(err) => {
                        rollback_prepared_trash(&prepared);
                        let _ = undo_delete_path(&prep.backup);
                        emit_progress(done, total, true);
                        return Err(err);
                    }
                }
            }
            Err(err) => {
                // Nothing was moved for this entry; roll back previous ones.
                rollback_prepared_trash(&prepared);
                emit_progress(done, total, true);
                return Err(err);
            }
        }
    }

    // Identify new trash items with a single post-scan.
    let mut new_items: HashMap<PathBuf, TrashItem> = HashMap::new();
    if let Ok(after) = backend.list_items() {
        for item in after.into_iter().filter(|i| !before_ids.contains(&i.id)) {
            new_items.insert(item.original_path(), item);
        }
    }

    let mut actions = Vec::with_capacity(prepared.len());
    for prep in prepared {
        let lookup = prep.staged_src.as_ref().unwrap_or(&prep.src);
        if let Some(item) = new_items.remove(lookup) {
            if let Err(err) = backend.rewrite_original_path(&item, &prep.src) {
                warn!(
                    "Failed to rewrite trash info for {}: {}",
                    prep.src.display(),
                    err
                );
            }
            let _ = undo_delete_path(&prep.backup);
            actions.push(Action::Move {
                from: prep.src,
                to: trash_item_path(&item),
            });
        } else {
            actions.push(Action::Delete {
                path: prep.src,
                backup: prep.backup,
            });
        }
    }

    let recorded = if actions.len() == 1 {
        actions.pop().unwrap()
    } else {
        Action::Batch(actions)
    };
    let _ = undo.record_applied(recorded);
    emit_changed();
    Ok(())
}

fn move_to_trash_many_blocking(
    paths: Vec<String>,
    app: tauri::AppHandle,
    undo: UndoState,
    progress_event: Option<String>,
    cancel: Option<&AtomicBool>,
) -> Result<(), String> {
    let backend = SystemTrashBackend;
    let mut last_emit = Instant::now();
    move_to_trash_many_with_backend(
        paths,
        undo,
        cancel,
        &backend,
        |cancel_flag| should_abort_fs_op(&app, cancel_flag),
        |done, total, finished| {
            emit_trash_progress(
                &app,
                progress_event.as_ref(),
                done,
                total,
                finished,
                &mut last_emit,
            );
        },
        || {
            let _ = runtime_lifecycle::emit_if_running(&app, "trash-changed", ());
        },
    )
}

#[tauri::command]
pub async fn move_to_trash_many(
    paths: Vec<String>,
    app: tauri::AppHandle,
    undo: tauri::State<'_, UndoState>,
    cancel: tauri::State<'_, CancelState>,
    progress_event: Option<String>,
) -> Result<(), String> {
    let app_handle = app.clone();
    let undo_state = undo.inner().clone();
    let cancel_state = cancel.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let cancel_guard = progress_event
            .as_ref()
            .map(|id| cancel_state.register(id.clone()))
            .transpose()?;
        let cancel_token = cancel_guard.as_ref().map(|g| g.token());
        move_to_trash_many_blocking(
            paths,
            app_handle,
            undo_state,
            progress_event,
            cancel_token.as_deref(),
        )
    })
    .await
    .map_err(|e| format!("Move to trash task failed: {e}"))?
}

pub(super) fn move_single_to_trash_with_backend<B: TrashBackend>(
    path: &str,
    backend: &B,
) -> Result<Action, String> {
    let src = sanitize_path_nofollow(path, true)?;
    check_no_symlink_components(&src)?;
    let src_snapshot = snapshot_existing_path(&src)?;

    // Backup into the central undo directory in case the OS trash item can't be found.
    let backup = temp_backup_path(&src);
    if let Some(parent) = backup.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create backup dir {}: {e}", parent.display()))?;
    }
    undo_copy_entry(&src, &backup)?;

    let before: HashSet<OsString> = backend
        .list_items()?
        .into_iter()
        .map(|item| item.id)
        .collect();

    let staged_src = match trash_delete_via_staged_rename(&src, &src_snapshot, backend) {
        Ok(staged) => staged,
        Err(err) => {
            let _ = undo_delete_path(&backup);
            return Err(err);
        }
    };

    let trashed_item = backend.list_items().ok().and_then(|after| {
        after
            .into_iter()
            .find(|item| !before.contains(&item.id) && item.original_path() == staged_src)
    });

    match trashed_item {
        Some(item) => {
            if let Err(err) = backend.rewrite_original_path(&item, &src) {
                warn!(
                    "Failed to rewrite trash info for {}: {}",
                    src.display(),
                    err
                );
            }
            // Remove the backup once we know the trash location.
            let _ = undo_delete_path(&backup);
            Ok(Action::Move {
                from: src,
                to: trash_item_path(&item),
            })
        }
        None => Ok(Action::Delete { path: src, backup }),
    }
}

fn move_single_to_trash(
    path: &str,
    app: &tauri::AppHandle,
    emit_event: bool,
) -> Result<Action, String> {
    let backend = SystemTrashBackend;
    let action = move_single_to_trash_with_backend(path, &backend)?;
    if emit_event {
        let _ = runtime_lifecycle::emit_if_running(app, "trash-changed", ());
    }
    Ok(action)
}
