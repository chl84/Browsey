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
    let pb = sanitize_path_nofollow(&path, true).map_err(FsError::from)?;
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
        fs::create_dir_all(parent).map_err(|error| {
            FsError::from_io_error(
                FsErrorCode::DeleteFailed,
                &format!("Failed to create backup dir {}", parent.display()),
                error,
            )
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

fn delete_entries_with_hooks<FShouldAbort, FEmitProgress>(
    paths: Vec<String>,
    undo: UndoState,
    cancel: Option<&AtomicBool>,
    mut should_abort: FShouldAbort,
    mut emit_progress: FEmitProgress,
) -> FsResult<()>
where
    FShouldAbort: FnMut(Option<&AtomicBool>) -> bool,
    FEmitProgress: FnMut(u64, u64, bool),
{
    if paths.is_empty() {
        emit_progress(0, 0, true);
        return Ok(());
    }
    let total = paths.len() as u64;
    let mut done = 0u64;
    let mut performed: Vec<Action> = Vec::with_capacity(paths.len());
    for raw in paths {
        if should_abort(cancel) {
            if !performed.is_empty() {
                let mut rollback = performed.clone();
                let _ = run_actions(&mut rollback, Direction::Backward);
            }
            emit_progress(done, total, true);
            return Err(FsError::new(FsErrorCode::Cancelled, "Delete cancelled"));
        }
        let path = match sanitize_path_nofollow(&raw, true).map_err(FsError::from) {
            Ok(path) => path,
            Err(err) => {
                if !performed.is_empty() {
                    let mut rollback = performed.clone();
                    if let Err(rb_err) = run_actions(&mut rollback, Direction::Backward) {
                        return Err(FsError::new(
                            FsErrorCode::DeleteFailed,
                            format!(
                                "Failed to delete {raw}: {err}; rollback also failed: {rb_err}"
                            ),
                        ));
                    }
                }
                return Err(FsError::new(
                    FsErrorCode::DeleteFailed,
                    format!("Failed to delete {raw}: {err}"),
                ));
            }
        };
        match delete_with_backup(&path) {
            Ok(action) => {
                performed.push(action);
                done = done.saturating_add(1);
                emit_progress(done, total, false);
            }
            Err(err) => {
                if !performed.is_empty() {
                    let mut rollback = performed.clone();
                    if let Err(rb_err) = run_actions(&mut rollback, Direction::Backward) {
                        return Err(FsError::new(
                            FsErrorCode::DeleteFailed,
                            format!(
                                "Failed to delete {}: {}; rollback also failed: {}",
                                path.display(),
                                err,
                                rb_err
                            ),
                        ));
                    }
                }
                return Err(FsError::new(
                    FsErrorCode::DeleteFailed,
                    format!("Failed to delete {}: {}", path.display(), err),
                ));
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
    emit_progress(done, total, true);
    Ok(())
}

fn delete_entries_blocking(
    app: tauri::AppHandle,
    paths: Vec<String>,
    progress_event: Option<String>,
    undo: UndoState,
    cancel: Option<&AtomicBool>,
) -> FsResult<()> {
    let mut last_emit = Instant::now();
    delete_entries_with_hooks(
        paths,
        undo,
        cancel,
        |cancel_flag| should_abort_fs_op(&app, cancel_flag),
        |done, total, finished| {
            emit_delete_progress(
                &app,
                progress_event.as_ref(),
                done,
                total,
                finished,
                &mut last_emit,
            );
        },
    )
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::undo::{run_actions, Direction};
    use std::cell::{Cell, RefCell};
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, SystemTime};

    fn uniq_path(label: &str) -> PathBuf {
        let ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_nanos();
        std::env::temp_dir().join(format!("browsey-delete-test-{label}-{ts}"))
    }

    fn write_file(path: &Path, bytes: &[u8]) {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)
            .expect("open file");
        file.write_all(bytes).expect("write file");
    }

    #[test]
    fn delete_with_backup_can_undo_single_file() {
        let dir = uniq_path("single-file");
        let _ = fs::create_dir_all(&dir);
        let src = dir.join("file.txt");
        write_file(&src, b"payload");

        let action = delete_with_backup(&src).expect("delete should succeed");
        let backup = match &action {
            Action::Delete { path, backup } => {
                assert_eq!(path, &src);
                backup.clone()
            }
            other => panic!("expected delete action, got {other:?}"),
        };

        assert!(!src.exists(), "source should be removed after delete");
        assert!(backup.exists(), "backup should be created for undo");

        let mut undo_actions = vec![action];
        run_actions(&mut undo_actions, Direction::Backward).expect("undo should restore file");

        assert!(src.exists(), "source should be restored by undo");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_with_backup_can_undo_directory_tree() {
        let dir = uniq_path("directory");
        let _ = fs::create_dir_all(&dir);
        let src_dir = dir.join("folder");
        let child = src_dir.join("child.txt");
        write_file(&child, b"child");

        let action = delete_with_backup(&src_dir).expect("delete should succeed");
        let backup = match &action {
            Action::Delete { path, backup } => {
                assert_eq!(path, &src_dir);
                backup.clone()
            }
            other => panic!("expected delete action, got {other:?}"),
        };

        assert!(!src_dir.exists(), "source directory should be removed");
        assert!(backup.exists(), "backup directory should exist");

        let mut undo_actions = vec![action];
        run_actions(&mut undo_actions, Direction::Backward).expect("undo should restore directory");

        assert!(
            src_dir.join("child.txt").exists(),
            "nested file should be restored"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_entries_with_hooks_rolls_back_when_later_item_fails() {
        let dir = uniq_path("rollback-later-failure");
        let _ = fs::create_dir_all(&dir);
        let first = dir.join("first.txt");
        let missing = dir.join("missing.txt");
        write_file(&first, b"first");
        let progress: RefCell<Vec<(u64, u64, bool)>> = RefCell::new(Vec::new());

        let err = delete_entries_with_hooks(
            vec![
                first.to_string_lossy().to_string(),
                missing.to_string_lossy().to_string(),
            ],
            UndoState::default(),
            None,
            |_| false,
            |done, total, finished| progress.borrow_mut().push((done, total, finished)),
        )
        .expect_err("second delete should fail");

        let err_msg = err.to_string().to_lowercase();
        assert!(
            err_msg.contains("failed to delete") || err_msg.contains("not exist"),
            "unexpected error: {err}"
        );
        assert!(
            first.exists(),
            "first item should be restored after rollback on later failure"
        );
        assert_eq!(
            progress.borrow().as_slice(),
            &[(1, 2, false)],
            "progress should report completed first item before failure"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_entries_with_hooks_cancellation_rolls_back_completed_items() {
        let dir = uniq_path("cancel-rollback");
        let _ = fs::create_dir_all(&dir);
        let first = dir.join("first.txt");
        let second = dir.join("second.txt");
        write_file(&first, b"first");
        write_file(&second, b"second");

        let cancel = AtomicBool::new(false);
        let checks = Cell::new(0usize);
        let progress: RefCell<Vec<(u64, u64, bool)>> = RefCell::new(Vec::new());

        let err = delete_entries_with_hooks(
            vec![
                first.to_string_lossy().to_string(),
                second.to_string_lossy().to_string(),
            ],
            UndoState::default(),
            Some(&cancel),
            |_| {
                let next = checks.get().saturating_add(1);
                checks.set(next);
                next >= 2
            },
            |done, total, finished| progress.borrow_mut().push((done, total, finished)),
        )
        .expect_err("second loop iteration should cancel");

        assert!(
            err.to_string().to_lowercase().contains("cancelled"),
            "unexpected error: {err}"
        );
        assert!(
            first.exists(),
            "first item should be restored after cancellation rollback"
        );
        assert!(second.exists(), "second item should remain untouched");
        assert_eq!(
            progress.borrow().as_slice(),
            &[(1, 2, false), (1, 2, true)],
            "progress should report completion snapshot at cancellation"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_entries_with_hooks_records_undo_for_successful_batch() {
        let dir = uniq_path("successful-batch");
        let _ = fs::create_dir_all(&dir);
        let first = dir.join("first.txt");
        let second = dir.join("second.txt");
        write_file(&first, b"first");
        write_file(&second, b"second");

        let undo = UndoState::default();
        let progress: RefCell<Vec<(u64, u64, bool)>> = RefCell::new(Vec::new());
        let cancel = AtomicBool::new(false);

        delete_entries_with_hooks(
            vec![
                first.to_string_lossy().to_string(),
                second.to_string_lossy().to_string(),
            ],
            undo.clone(),
            Some(&cancel),
            |flag| flag.is_some_and(|token| token.load(Ordering::Relaxed)),
            |done, total, finished| progress.borrow_mut().push((done, total, finished)),
        )
        .expect("batch delete should succeed");

        assert!(!first.exists());
        assert!(!second.exists());
        assert_eq!(
            progress.borrow().as_slice(),
            &[(1, 2, false), (2, 2, false), (2, 2, true)]
        );

        undo.undo().expect("undo should restore deleted batch");
        assert!(first.exists(), "first file should be restored");
        assert!(second.exists(), "second file should be restored");

        let _ = fs::remove_dir_all(&dir);
    }
}
