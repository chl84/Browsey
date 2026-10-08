//! Explicit, local-only cleanup. Never evict user data or permanently delete it.
use super::*;
use std::collections::HashSet;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingCopyDetails {
    #[serde(flatten)]
    copy: CloudWorkingCopy,
    storage_bytes: u64,
    cleanup_blocked_reason: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingCopyOverview {
    copies: Vec<WorkingCopyDetails>,
    storage_bytes: u64,
    incomplete: bool,
    retained_entries: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupSkipped {
    id: String,
    reason: String,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupResult {
    removed_ids: Vec<String>,
    skipped: Vec<CleanupSkipped>,
}

fn refused(message: &str) -> CloudCommandError {
    CloudCommandError::new(CloudCommandErrorCode::Conflict, message)
}

// Metadata only, on explicit inspection. Symlinks are never traversed and file
// contents are not read for storage accounting (sizes are logical, not allocated).
fn storage_size(path: &Path) -> (u64, bool) {
    let mut bytes = 0u64;
    let mut incomplete = false;
    let mut pending = vec![path.to_path_buf()];
    while let Some(path) = pending.pop() {
        match fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_symlink() => {}
            Ok(meta) if meta.is_file() => bytes = bytes.saturating_add(meta.len()),
            Ok(meta) if meta.is_dir() => match fs::read_dir(path) {
                Ok(entries) => {
                    for entry in entries {
                        match entry {
                            Ok(entry) => pending.push(entry.path()),
                            Err(_) => incomplete = true,
                        }
                    }
                }
                Err(_) => incomplete = true,
            },
            Ok(_) | Err(_) => incomplete = true,
        }
    }
    (bytes, incomplete)
}

fn regular_single_link(path: &Path) -> CloudCommandResult<()> {
    let meta = fs::symlink_metadata(path).map_err(io_error)?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(refused(
            "Linked or irregular files are kept for manual recovery",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if meta.nlink() != 1 {
            return Err(refused(
                "Files with hard-link aliases are kept for manual recovery",
            ));
        }
    }
    Ok(())
}

// Only a normal manifest and its unchanged file can be removed. Extra editor
// backups, snapshots, verification output and unknown data are retained.
fn validate_layout(dir: &Path, copy: &CloudWorkingCopy) -> CloudCommandResult<()> {
    if copy.pending_save.is_some()
        || matches!(
            copy.save_status,
            CloudSaveStatus::Pending
                | CloudSaveStatus::Uploading
                | CloudSaveStatus::Conflict
                | CloudSaveStatus::Error
        )
    {
        return Err(refused(
            "Resolve unfinished saves or save problems before removing this copy",
        ));
    }
    let files = dir.join("files");
    let meta = fs::symlink_metadata(&files).map_err(io_error)?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err(refused(
            "Linked or irregular folders are kept for manual recovery",
        ));
    }
    let name = Path::new(&copy.local_path)
        .file_name()
        .ok_or_else(|| refused("Invalid local filename"))?;
    for entry in fs::read_dir(dir).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        if entry.file_name() != "manifest.json" && entry.file_name() != "files" {
            return Err(refused(
                "Additional recovery files are present; inspect the local folder first",
            ));
        }
    }
    for entry in fs::read_dir(&files).map_err(io_error)? {
        if entry.map_err(io_error)?.file_name() != name {
            return Err(refused(
                "Additional editor files are present; inspect the local folder first",
            ));
        }
    }
    regular_single_link(&dir.join("manifest.json"))?;
    regular_single_link(&files.join(name))?;
    Ok(())
}

fn validate_removable(dir: &Path, copy: &CloudWorkingCopy) -> CloudCommandResult<()> {
    validate_layout(dir, copy)?;
    let name = Path::new(&copy.local_path)
        .file_name()
        .ok_or_else(|| refused("Invalid local filename"))?;
    // Do not trust the UI, persisted dirty flag or an old Saved event.
    if hash_file(&dir.join("files").join(name))? != copy.original_hash {
        return Err(refused(
            "New local changes were found; save them before removing this copy",
        ));
    }
    Ok(())
}

fn overview_at(base: &Path) -> CloudCommandResult<WorkingCopyOverview> {
    private_dir(base)?;
    let mut overview = WorkingCopyOverview {
        copies: Vec::new(),
        storage_bytes: 0,
        incomplete: false,
        retained_entries: 0,
    };
    for entry in fs::read_dir(base).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        let (bytes, incomplete) = storage_size(&entry.path());
        overview.storage_bytes = overview.storage_bytes.saturating_add(bytes);
        overview.incomplete |= incomplete;
        let copy = entry
            .file_name()
            .to_str()
            .and_then(|id| load_at(base, id).ok());
        if let Some(copy) = copy {
            let blocked = if copy.dirty {
                Some("New local changes were found; save them before removing this copy".into())
            } else {
                validate_layout(&entry.path(), &copy)
                    .err()
                    .map(|e| e.message().to_owned())
            };
            overview.copies.push(WorkingCopyDetails {
                copy,
                storage_bytes: bytes,
                cleanup_blocked_reason: blocked,
            });
        } else {
            // Staging, corrupt and incomplete sessions count towards storage,
            // but are never selected for cleanup.
            overview.retained_entries += 1;
        }
    }
    overview
        .copies
        .sort_by_key(|row| std::cmp::Reverse(row.copy.created_at));
    Ok(overview)
}

fn remove_at(
    base: &Path,
    id: &str,
    trash: impl FnOnce(&Path) -> CloudCommandResult<()>,
) -> CloudCommandResult<()> {
    sync::try_with_copy_lock(id, || {
        private_dir(base)?;
        let copy = load_manifest_at(base, id, false)?;
        let dir = session_dir(base, id)?;
        validate_removable(&dir, &copy)?;
        // Detach the path atomically before the final content check. A save
        // racing the confirmation is retained, never recursively deleted.
        let staged = operation_dir_at(base, "cleanup")?;
        let detached = staged.join(id);
        if let Err(error) = fs::rename(&dir, &detached) {
            let _ = fs::remove_dir(&staged);
            return Err(io_error(error));
        }
        let result = finish_removal(&dir, &detached, &copy, trash);
        let _ = fs::remove_dir(&staged);
        result
    })
}

fn finish_removal(
    original: &Path,
    detached: &Path,
    copy: &CloudWorkingCopy,
    trash: impl FnOnce(&Path) -> CloudCommandResult<()>,
) -> CloudCommandResult<()> {
    let result = validate_removable(detached, copy).and_then(|()| trash(detached));
    if let Err(error) = result {
        if fs::symlink_metadata(detached).is_ok()
            && (fs::symlink_metadata(original).is_ok() || fs::rename(detached, original).is_err())
        {
            return Err(refused(&format!(
                "{} The retained working copy is at {}",
                error.message(),
                detached.display()
            )));
        }
        return Err(error);
    }
    Ok(())
}

#[tauri::command]
pub async fn cloud_working_copy_overview() -> ApiResult<WorkingCopyOverview> {
    map_api_result(super::super::map_spawn_result(
        tauri::async_runtime::spawn_blocking(|| overview_at(&root()?)).await,
        "Working-copy inspection failed",
    ))
}

#[tauri::command]
pub async fn remove_cloud_working_copies(
    ids: Vec<String>,
    editors_closed: bool,
    app: tauri::AppHandle,
) -> ApiResult<CleanupResult> {
    map_api_result(super::super::map_spawn_result(
        tauri::async_runtime::spawn_blocking(move || {
            if !editors_closed {
                return Err(refused(
                    "Close these files in your editor and confirm before cleanup",
                ));
            }
            let base = root()?;
            let mut result = CleanupResult::default();
            let mut seen = HashSet::new();
            for id in ids {
                if !seen.insert(id.clone()) {
                    continue;
                }
                match remove_at(&base, &id, |path| {
                    ::trash::delete(path).map_err(|error| {
                        CloudCommandError::new(
                            CloudCommandErrorCode::TaskFailed,
                            format!("Cannot move this copy to the trash; it was kept: {error}"),
                        )
                    })
                }) {
                    Ok(()) => {
                        monitor::forget(&app, &id);
                        result.removed_ids.push(id);
                    }
                    Err(error) => result.skipped.push(CleanupSkipped {
                        id,
                        reason: error.message().into(),
                    }),
                }
            }
            Ok(result)
        })
        .await,
        "Working-copy cleanup failed",
    ))
}

#[cfg(test)]
mod tests;
