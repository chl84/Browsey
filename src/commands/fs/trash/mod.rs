use super::error::{map_api_result, FsError, FsErrorCode, FsResult};
use crate::errors::api_error::ApiResult;
use crate::runtime_lifecycle;
use ::trash::os_limited::{list as trash_list, purge_all, restore_all};
use std::collections::HashSet;
use std::ffi::OsString;

mod backend;
mod listing;
mod move_ops;
mod staging;

#[cfg(all(test, not(target_os = "windows")))]
mod tests;

pub use listing::list_trash;
pub(super) use move_ops::move_single_to_trash_with_system_backend;
pub use move_ops::{move_to_trash, move_to_trash_many};
pub use staging::cleanup_stale_trash_staging;

pub(super) trait TrashOps {
    fn list_items(&self) -> FsResult<Vec<::trash::TrashItem>>;
    fn restore_items(&self, items: Vec<::trash::TrashItem>) -> FsResult<()>;
    fn purge_items(&self, items: Vec<::trash::TrashItem>) -> FsResult<()>;
}

struct SystemTrashOps;

impl TrashOps for SystemTrashOps {
    fn list_items(&self) -> FsResult<Vec<::trash::TrashItem>> {
        trash_list().map_err(|error| {
            FsError::new(
                FsErrorCode::TrashFailed,
                format!("Failed to list trash: {error}"),
            )
        })
    }

    fn restore_items(&self, items: Vec<::trash::TrashItem>) -> FsResult<()> {
        restore_all(items).map_err(|error| {
            FsError::new(
                FsErrorCode::TrashFailed,
                format!("Failed to restore: {error}"),
            )
        })
    }

    fn purge_items(&self, items: Vec<::trash::TrashItem>) -> FsResult<()> {
        purge_all(items).map_err(|error| {
            FsError::new(
                FsErrorCode::TrashFailed,
                format!("Failed to delete permanently: {error}"),
            )
        })
    }
}

#[tauri::command]
pub fn restore_trash_items(ids: Vec<String>, app: tauri::AppHandle) -> ApiResult<()> {
    map_api_result(restore_trash_items_impl(ids, app))
}

#[tauri::command]
pub async fn empty_trash(app: tauri::AppHandle) -> ApiResult<()> {
    let task = tauri::async_runtime::spawn_blocking(move || {
        empty_trash_with_ops(&SystemTrashOps, || {
            let _ = runtime_lifecycle::emit_if_running(&app, "trash-changed", ());
        })
    });
    let result = match task.await {
        Ok(result) => result,
        Err(error) => Err(FsError::new(
            FsErrorCode::TaskFailed,
            format!("Empty Wastebasket task failed: {error}"),
        )),
    };
    map_api_result(result)
}

pub(super) fn empty_trash_with_ops<T: TrashOps, F: FnOnce()>(
    ops: &T,
    emit_changed: F,
) -> FsResult<()> {
    // Use the native catalog, not the UI listing: unreadable metadata and
    // non-UTF-8 identifiers must not silently leave items behind.
    let items = ops.list_items()?;
    if items.is_empty() {
        return Ok(());
    }
    let result = ops.purge_items(items);
    // A failed purge can have removed some items. Refresh without retrying.
    emit_changed();
    result
}

fn restore_trash_items_impl(ids: Vec<String>, app: tauri::AppHandle) -> FsResult<()> {
    restore_trash_items_with_ops(ids, &SystemTrashOps, || {
        let _ = runtime_lifecycle::emit_if_running(&app, "trash-changed", ());
    })
}

pub(super) fn restore_trash_items_with_ops<T, F>(
    ids: Vec<String>,
    ops: &T,
    emit_changed: F,
) -> FsResult<()>
where
    T: TrashOps,
    F: FnOnce(),
{
    let wanted: HashSet<OsString> = ids.into_iter().map(OsString::from).collect();
    if wanted.is_empty() {
        return Ok(());
    }
    let items = ops.list_items()?;
    let selected: Vec<_> = items
        .into_iter()
        .filter(|item| wanted.contains(&item.id))
        .collect();
    if selected.is_empty() {
        return Err(FsError::new(
            FsErrorCode::InvalidInput,
            "Nothing to restore",
        ));
    }
    ops.restore_items(selected).map(|_| {
        emit_changed();
    })
}

#[tauri::command]
pub fn purge_trash_items(ids: Vec<String>, app: tauri::AppHandle) -> ApiResult<()> {
    map_api_result(purge_trash_items_impl(ids, app))
}

fn purge_trash_items_impl(ids: Vec<String>, app: tauri::AppHandle) -> FsResult<()> {
    purge_trash_items_with_ops(ids, &SystemTrashOps, || {
        let _ = runtime_lifecycle::emit_if_running(&app, "trash-changed", ());
    })
}

pub(super) fn purge_trash_items_with_ops<T, F>(
    ids: Vec<String>,
    ops: &T,
    emit_changed: F,
) -> FsResult<()>
where
    T: TrashOps,
    F: FnOnce(),
{
    let wanted: HashSet<OsString> = ids.into_iter().map(OsString::from).collect();
    if wanted.is_empty() {
        return Ok(());
    }
    let items = ops.list_items()?;
    let selected: Vec<_> = items
        .into_iter()
        .filter(|item| wanted.contains(&item.id))
        .collect();
    if selected.is_empty() {
        return Err(FsError::new(FsErrorCode::InvalidInput, "Nothing to delete"));
    }
    ops.purge_items(selected).map(|_| {
        emit_changed();
    })
}
