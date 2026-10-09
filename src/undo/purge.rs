//! User-confirmed deletion. Recovery's session locks and snapshot deletion are
//! reused; no recursive deletion, unlocked sessions or legacy folders are used.
use std::{fs, io, path::Path};

use serde::Serialize;

use super::{
    backup, error::map_api_result, recovery, UndoError, UndoManager, UndoResult, UndoState,
};
use crate::{
    errors::api_error::ApiResult,
    fs_utils::{FileIdentity, TreeSnapshot},
};

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteBackupsResult {
    deleted_sessions: u64,
    retained_sessions: u64,
    errors: Vec<String>,
}

fn delete_at(base: &Path, manager: &mut UndoManager) -> UndoResult<DeleteBackupsResult> {
    backup::validate_undo_dir(base)?;
    crate::fs_utils::check_no_symlink_components(base)?;
    let entries = match fs::read_dir(base) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            manager.clear();
            return Ok(DeleteBackupsResult::default());
        }
        Err(error) => {
            return Err(UndoError::from_io_error(
                "Inspect backups before deletion",
                error,
            ))
        }
    };
    let root = FileIdentity::capture(base)
        .ok_or_else(|| UndoError::invalid_input("Cannot verify backup directory"))?;
    // Clear both stacks before deleting any file, including on partial failure.
    manager.clear();
    let mut result = DeleteBackupsResult::default();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                result.retained_sessions += 1;
                result.errors.push(error.to_string());
                continue;
            }
        };
        let name = entry.file_name();
        let Some(name) = name.to_str().filter(|name| name.starts_with("session-")) else {
            continue;
        };
        let kind = match entry.file_type() {
            Ok(kind) => kind,
            Err(error) => {
                result.retained_sessions += 1;
                result.errors.push(error.to_string());
                continue;
            }
        };
        if kind.is_file() {
            continue;
        } // Sibling locks/notices.
        if !kind.is_dir() {
            result.retained_sessions += 1;
            result.errors.push(format!(
                "Unsafe backup session kept: {}",
                entry.path().display()
            ));
            continue;
        }
        let remove = || -> UndoResult<bool> {
            if !root.matches(base) {
                return Err(UndoError::snapshot_mismatch(base));
            }
            let Some(lock) = recovery::lock_session(base, name)? else {
                return Ok(false);
            };
            let verify = || -> io::Result<()> {
                crate::fs_utils::check_no_symlink_components(base).map_err(io::Error::other)?;
                if !root.matches(base) {
                    return Err(io::Error::other("Backup directory changed"));
                }
                lock.verify().map_err(io::Error::other)
            };
            // Inspect the complete session before removing anything. Symlinks,
            // devices and unreadable trees retain the session and its markers.
            let snapshot = TreeSnapshot::capture_with_check(&lock.directory, verify)
                .map_err(|error| UndoError::from_io_error("Inspect backup deletion", error))?;
            if lock.current.is_some() {
                // Keep the registered live directory and original lifetime lock.
                // New allocations can reuse this empty session after maintenance.
                let children = fs::read_dir(&lock.directory)
                    .map_err(|error| UndoError::from_io_error("Read owned backup session", error))?
                    .map(|child| {
                        let path = child?.path();
                        let tree = TreeSnapshot::capture_with_check(&path, verify)?;
                        Ok((path, tree))
                    })
                    .collect::<io::Result<Vec<_>>>()
                    .map_err(|error| UndoError::from_io_error("Inspect owned backups", error))?;
                snapshot
                    .verify_with_check(&lock.directory, verify)
                    .map_err(|error| {
                        UndoError::from_io_error("Verify backups before deletion", error)
                    })?;
                for (path, tree) in children {
                    tree.remove_recorded_with_check(&path, verify)
                        .map_err(|error| UndoError::from_io_error("Delete backup entry", error))?;
                }
            } else {
                snapshot
                    .remove_recorded_with_check(&lock.directory, verify)
                    .map_err(|error| UndoError::from_io_error("Delete backup session", error))?;
                if !root.matches(base) || !lock.lock_identity.matches(&lock.lock_path) {
                    return Err(UndoError::snapshot_mismatch(&lock.lock_path));
                }
                fs::remove_file(&lock.lock_path).map_err(|error| {
                    UndoError::from_io_error("Delete backup session lock", error)
                })?;
                super::recovery_notice::remove(&lock.directory);
            }
            Ok(true)
        };
        match remove() {
            Ok(true) => result.deleted_sessions += 1,
            Ok(false) => result.retained_sessions += 1,
            Err(error) => {
                result.retained_sessions += 1;
                result.errors.push(error.to_string());
            }
        }
    }
    Ok(result)
}

#[tauri::command]
pub async fn delete_all_recovery_backups(
    state: tauri::State<'_, UndoState>,
) -> ApiResult<DeleteBackupsResult> {
    let inner = state.clone_inner();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _maintenance = super::maintenance::exclusive()?;
        let mut manager = inner.try_lock().map_err(|_| {
            UndoError::lock_failed(
                "Undo history is in use. Wait for the operation to finish, then retry.",
            )
        })?;
        delete_at(&backup::base_undo_dir(), &mut manager)
    })
    .await
    .map_err(|error| UndoError::lock_failed(format!("Backup deletion worker failed: {error}")))
    .and_then(|result| result);
    map_api_result(result)
}

#[cfg(test)]
mod tests;
