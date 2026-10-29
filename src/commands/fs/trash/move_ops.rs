use super::super::{
    error::{map_api_result, map_external_result, FsError, FsErrorCode, FsResult},
    should_abort_fs_op, CancelState, DeleteProgressPayload, UndoState,
};
use super::{
    backend::{SystemTrashBackend, TrashBackend},
    listing::trash_item_path,
    staging::trash_delete_via_staged_rename,
};
use crate::{
    errors::api_error::ApiResult,
    fs_utils::{check_no_symlink_components, sanitize_path_nofollow},
    runtime_lifecycle,
    undo::{snapshot_existing_path, Action, PathSnapshot},
};
use ::trash::TrashItem;
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tracing::warn;

#[tauri::command]
pub async fn move_to_trash(
    path: String,
    app: tauri::AppHandle,
    undo: tauri::State<'_, UndoState>,
) -> ApiResult<()> {
    map_api_result(move_to_trash_impl(path, app, undo).await)
}

async fn move_to_trash_impl(
    path: String,
    app: tauri::AppHandle,
    undo: tauri::State<'_, UndoState>,
) -> FsResult<()> {
    let app_handle = app.clone();
    let undo_state = undo.inner().clone();
    let task = tauri::async_runtime::spawn_blocking(move || -> FsResult<()> {
        let _backup_operation = crate::undo::backup_operation().map_err(FsError::from)?;
        let action = move_single_to_trash(&path, &app_handle, true)?;
        let _ = undo_state.record_applied(action);
        Ok(())
    });
    match task.await {
        Ok(result) => result,
        Err(error) => Err(FsError::new(
            FsErrorCode::TaskFailed,
            format!("Move to trash task failed: {error}"),
        )),
    }
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
                items: done,
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
    src_snapshot: PathSnapshot,
    staged_src: Option<PathBuf>,
    excluded_ids: Vec<OsString>,
}

fn prepare_trash_move(raw: &str) -> FsResult<PreparedTrashMove> {
    super::super::network_delete::require_local_backup_path(std::path::Path::new(raw))?;
    let src = sanitize_path_nofollow(raw, true).map_err(FsError::from)?;
    map_external_result(check_no_symlink_components(&src))?;
    let src_snapshot = map_external_result(snapshot_existing_path(&src))?;
    Ok(PreparedTrashMove {
        src,
        src_snapshot,
        staged_src: None,
        excluded_ids: Vec::new(),
    })
}

type TrashCatalog = HashMap<PathBuf, Vec<TrashItem>>;

fn catalog(items: Vec<TrashItem>) -> TrashCatalog {
    let mut by_path = TrashCatalog::new();
    for item in items {
        by_path.entry(item.original_path()).or_default().push(item);
    }
    by_path
}

fn previous_ids(items: &TrashCatalog, original: &Path) -> Vec<OsString> {
    items
        .get(original)
        .into_iter()
        .flatten()
        .map(|item| item.id.clone())
        .collect()
}

fn find_item<'a>(
    items: &'a TrashCatalog,
    staged: &Path,
    excluded_ids: &[OsString],
) -> Option<&'a TrashItem> {
    let mut found = items
        .get(staged)?
        .iter()
        .filter(|item| !excluded_ids.contains(&item.id));
    let item = found.next()?;
    found.next().is_none().then_some(item)
}

fn finish_stage(staged: &std::path::Path, original: &std::path::Path) {
    #[cfg(not(target_os = "windows"))]
    if let Err(error) = super::staging::remove_trash_stage_journal_entry(staged, original) {
        warn!(%error, "Trash stage journal cleanup deferred");
    }
    #[cfg(target_os = "windows")]
    let _ = (staged, original);
}

fn history_action<B: TrashBackend>(
    prep: &PreparedTrashMove,
    items: &TrashCatalog,
    backend: &B,
) -> Action {
    let staged = prep.staged_src.as_ref().unwrap_or(&prep.src);
    if let Some(item) = find_item(items, staged, &prep.excluded_ids) {
        if let Err(error) = backend.rewrite_original_path(item, &prep.src) {
            warn!(%error, source = %prep.src.display(), "Trash metadata rewrite deferred");
        } else {
            finish_stage(staged, &prep.src);
        }
        Action::Move {
            from: prep.src.clone(),
            to: trash_item_path(item),
        }
    } else {
        Action::PendingTrash {
            from: prep.src.clone(),
            staged: staged.clone(),
            excluded_ids: prep.excluded_ids.clone(),
        }
    }
}

pub(crate) fn resolve_pending_trash(
    from: &std::path::Path,
    staged: &std::path::Path,
    excluded_ids: &[OsString],
) -> crate::undo::UndoResult<PathBuf> {
    resolve_pending_trash_with_backend(from, staged, &SystemTrashBackend, excluded_ids)
}

pub(super) fn resolve_pending_trash_with_backend<B: TrashBackend>(
    from: &std::path::Path,
    staged: &std::path::Path,
    backend: &B,
    excluded_ids: &[OsString],
) -> crate::undo::UndoResult<PathBuf> {
    let items = catalog(backend.list_items().map_err(|error| {
        crate::undo::UndoError::new(crate::undo::UndoErrorCode::IoError, error.message())
    })?);
    let item = find_item(&items, staged, excluded_ids).ok_or_else(|| crate::undo::UndoError::not_found(
        format!("Trash entry for {} is unavailable. Refresh Wastebasket before retrying Undo; no source was overwritten", from.display())))?;
    backend.rewrite_original_path(item, from).map_err(|error| {
        crate::undo::UndoError::new(crate::undo::UndoErrorCode::IoError, error.message())
    })?;
    finish_stage(staged, from);
    Ok(trash_item_path(item))
}

fn rollback_prepared_trash<B: TrashBackend>(
    prepared: &[PreparedTrashMove],
    backend: &B,
    undo: &UndoState,
) -> Vec<String> {
    let items = catalog(backend.list_items().unwrap_or_default());
    let mut failures = Vec::new();
    for prep in prepared.iter().rev() {
        let staged = prep.staged_src.as_ref().unwrap_or(&prep.src);
        let restored = find_item(&items, staged, &prep.excluded_ids)
            .ok_or_else(|| {
                FsError::new(
                    FsErrorCode::TrashFailed,
                    "Trash catalog entry is not available yet",
                )
            })
            .and_then(|item| backend.restore_item(item, &prep.src));
        if let Err(error) = restored {
            failures.push(format!("{}: {error}", prep.src.display()));
            let _ = undo.record_applied(history_action(prep, &items, backend));
        } else {
            finish_stage(staged, &prep.src);
        }
    }
    failures
}

pub(super) fn move_to_trash_many_with_backend<B, FShouldAbort, FEmitProgress, FEmitChanged>(
    paths: Vec<String>,
    undo: UndoState,
    cancel: Option<&AtomicBool>,
    backend: &B,
    mut should_abort: FShouldAbort,
    mut emit_progress: FEmitProgress,
    mut emit_changed: FEmitChanged,
) -> FsResult<()>
where
    B: TrashBackend,
    FShouldAbort: FnMut(Option<&AtomicBool>) -> bool,
    FEmitProgress: FnMut(u64, u64, bool),
    FEmitChanged: FnMut(),
{
    let _backup_operation = crate::undo::backup_operation().map_err(FsError::from)?;
    let total = paths.len() as u64;
    if total == 0 {
        emit_progress(0, 0, true);
        return Ok(());
    }
    for raw in &paths {
        super::super::network_delete::require_local_backup_path(std::path::Path::new(raw))?;
    }
    // Refuse before changing sources if the native catalog is unavailable.
    let before = catalog(backend.list_items()?);
    let mut prepared = Vec::with_capacity(paths.len());
    for path in paths {
        let result = if should_abort(cancel) {
            Err(FsError::new(
                FsErrorCode::Cancelled,
                "Move to trash cancelled",
            ))
        } else {
            prepare_trash_move(&path).and_then(|mut prep| {
                prep.excluded_ids = previous_ids(&before, &prep.src);
                let staged =
                    trash_delete_via_staged_rename(&prep.src, &prep.src_snapshot, backend)?;
                prep.staged_src = Some(staged);
                Ok(prep)
            })
        };
        match result {
            Ok(prep) => {
                prepared.push(prep);
                emit_progress(prepared.len() as u64, total, false);
            }
            Err(error) => {
                let failures = rollback_prepared_trash(&prepared, backend, &undo);
                emit_progress(prepared.len() as u64, total, true);
                if !prepared.is_empty() {
                    emit_changed();
                }
                return Err(if failures.is_empty() {
                    error
                } else {
                    FsError::new(FsErrorCode::TrashFailed,format!("{error}; some items remain in Wastebasket and could not be restored: {}. Refresh Wastebasket or retry Undo; existing source paths were not overwritten",failures.join("; ")))
                });
            }
        }
    }
    let items = catalog(backend.list_items().unwrap_or_default());
    let mut actions: Vec<_> = prepared
        .iter()
        .map(|prep| history_action(prep, &items, backend))
        .collect();
    let action = if actions.len() == 1 {
        actions.pop().unwrap()
    } else {
        Action::Batch(actions)
    };
    let _ = undo.record_applied(action);
    emit_progress(total, total, true);
    emit_changed();
    Ok(())
}

fn move_to_trash_many_blocking(
    paths: Vec<String>,
    app: tauri::AppHandle,
    undo: UndoState,
    progress_event: Option<String>,
    cancel: Option<&AtomicBool>,
) -> FsResult<()> {
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
) -> ApiResult<()> {
    map_api_result(move_to_trash_many_impl(paths, app, undo, cancel, progress_event).await)
}

async fn move_to_trash_many_impl(
    paths: Vec<String>,
    app: tauri::AppHandle,
    undo: tauri::State<'_, UndoState>,
    cancel: tauri::State<'_, CancelState>,
    progress_event: Option<String>,
) -> FsResult<()> {
    let app_handle = app.clone();
    let undo_state = undo.inner().clone();
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
        move_to_trash_many_blocking(
            paths,
            app_handle,
            undo_state,
            progress_event,
            cancel_token.as_deref(),
        )
    });
    match task.await {
        Ok(result) => result,
        Err(error) => Err(FsError::new(
            FsErrorCode::TaskFailed,
            format!("Move to trash task failed: {error}"),
        )),
    }
}

pub(super) fn move_single_to_trash_with_backend<B: TrashBackend>(
    path: &str,
    backend: &B,
) -> FsResult<Action> {
    let mut prep = prepare_trash_move(path)?;
    prep.excluded_ids = previous_ids(&catalog(backend.list_items()?), &prep.src);
    prep.staged_src = Some(trash_delete_via_staged_rename(
        &prep.src,
        &prep.src_snapshot,
        backend,
    )?);
    let items = catalog(backend.list_items().unwrap_or_default());
    Ok(history_action(&prep, &items, backend))
}

fn move_single_to_trash(path: &str, app: &tauri::AppHandle, emit_event: bool) -> FsResult<Action> {
    let action = move_single_to_trash_with_system_backend(path)?;
    if emit_event {
        let _ = runtime_lifecycle::emit_if_running(app, "trash-changed", ());
    }
    Ok(action)
}

pub(in crate::commands::fs) fn move_single_to_trash_with_system_backend(
    path: &str,
) -> FsResult<Action> {
    move_single_to_trash_with_backend(path, &SystemTrashBackend)
}
