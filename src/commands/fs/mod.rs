//! File-system oriented Tauri commands: listings, mounts, trash, opening, create/delete, and watch wiring.

use crate::{
    entry::{CachedMeta, FsEntry},
    errors::api_error::ApiError,
    fs_utils::{check_no_symlink_components, sanitize_path_follow, sanitize_path_nofollow},
    path_guard::ensure_existing_dir_nonsymlink,
    runtime_lifecycle,
    tasks::CancelState,
    undo::{is_destination_exists_error, move_with_fallback, temp_backup_path, Action, UndoState},
};
mod delete_ops;
mod error;
#[cfg(target_os = "windows")]
#[path = "windows.rs"]
pub mod fs_windows;
mod open_ops;
mod trash;

pub use crate::commands::listing::DirListing;
use serde::Serialize;
#[cfg(target_os = "windows")]
use std::os::windows::prelude::*;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

pub use delete_ops::{delete_entries, delete_entry};
use error::{is_expected_set_hidden_error, SetHiddenError, SetHiddenErrorCode, SetHiddenResult};
pub use open_ops::open_entry;
pub use trash::{
    cleanup_stale_trash_staging, list_trash, move_to_trash, move_to_trash_many, purge_trash_items,
    restore_trash_items,
};

pub fn expand_path(raw: Option<String>) -> Result<PathBuf, String> {
    if let Some(p) = raw {
        if p == "~" {
            dirs_next::home_dir().ok_or_else(|| "Home directory not found".to_string())
        } else if let Some(stripped) = p.strip_prefix("~/") {
            let home =
                dirs_next::home_dir().ok_or_else(|| "Home directory not found".to_string())?;
            Ok(home.join(stripped))
        } else {
            Ok(PathBuf::from(p))
        }
    } else if let Some(home) = dirs_next::home_dir() {
        Ok(home)
    } else {
        std::env::current_dir().map_err(|e| format!("Failed to read working directory: {e}"))
    }
}

#[derive(Serialize, Clone)]
pub struct MountInfo {
    pub label: String,
    pub path: String,
    pub fs: String,
    pub removable: bool,
}

pub(crate) fn entry_from_cached(path: &Path, cached: &CachedMeta, starred: bool) -> FsEntry {
    let kind = if cached.is_link {
        "link"
    } else if cached.is_dir {
        "dir"
    } else {
        "file"
    }
    .to_string();
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_string());

    FsEntry {
        name,
        path: path.to_string_lossy().into_owned(),
        kind,
        ext,
        size: cached.size,
        items: None,
        modified: cached.modified.clone(),
        original_path: None,
        trash_id: None,
        icon_id: cached.icon_id,
        starred,
        hidden: cached.hidden,
        network: cached.network,
        read_only: cached.read_only,
        read_denied: cached.read_denied,
    }
}

#[cfg(target_os = "windows")]
fn set_hidden_attr(path: &Path, hidden: bool) -> SetHiddenResult<(PathBuf, bool)> {
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileAttributesW, SetFileAttributesW, FILE_ATTRIBUTE_HIDDEN,
    };
    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let attrs = unsafe { GetFileAttributesW(wide.as_ptr()) };
    if attrs == u32::MAX {
        return Err(SetHiddenError::new(
            SetHiddenErrorCode::HiddenUpdateFailed,
            "GetFileAttributes failed",
        ));
    }
    let is_hidden = attrs & FILE_ATTRIBUTE_HIDDEN != 0;
    if hidden == is_hidden {
        return Ok((path.to_path_buf(), false));
    }
    let mut new_attrs = attrs;
    if hidden {
        new_attrs |= FILE_ATTRIBUTE_HIDDEN;
    } else {
        new_attrs &= !FILE_ATTRIBUTE_HIDDEN;
    }
    let ok = unsafe { SetFileAttributesW(wide.as_ptr(), new_attrs) };
    if ok == 0 {
        return Err(SetHiddenError::new(
            SetHiddenErrorCode::HiddenUpdateFailed,
            "SetFileAttributes failed",
        ));
    }
    Ok((path.to_path_buf(), true))
}

#[cfg(not(target_os = "windows"))]
fn set_hidden_attr(path: &Path, hidden: bool) -> SetHiddenResult<(PathBuf, bool)> {
    // On Unix, hidden = leading dot. Rename within same directory.
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| SetHiddenError::new(SetHiddenErrorCode::InvalidPath, "Invalid file name"))?;
    let is_dot = file_name.starts_with('.');
    if hidden == is_dot {
        return Ok((path.to_path_buf(), false));
    }
    let target_name = if hidden {
        format!(".{file_name}")
    } else {
        file_name.trim_start_matches('.').to_string()
    };
    if target_name.is_empty() {
        return Err(SetHiddenError::new(
            SetHiddenErrorCode::InvalidPath,
            "Cannot derive visible name",
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| SetHiddenError::new(SetHiddenErrorCode::InvalidPath, "Missing parent"))?;
    let target = parent.join(&target_name);
    if target == path {
        return Ok((path.to_path_buf(), false));
    }
    match move_with_fallback(path, &target) {
        Ok(_) => {}
        Err(e) if is_destination_exists_error(&e) => {
            return Err(SetHiddenError::new(
                SetHiddenErrorCode::TargetExists,
                format!("Target already exists: {}", target.display()),
            ));
        }
        Err(e) => {
            return Err(SetHiddenError::new(
                SetHiddenErrorCode::HiddenUpdateFailed,
                format!("Failed to rename: {e}"),
            ));
        }
    }
    Ok((target, true))
}

#[derive(Serialize)]
pub struct SetHiddenBatchItem {
    pub path: String,
    pub ok: bool,
    pub new_path: String,
    pub error: Option<ApiError>,
}

#[derive(Serialize)]
pub struct SetHiddenBatchResult {
    pub per_item: Vec<SetHiddenBatchItem>,
    pub failures: usize,
    pub unexpected_failures: usize,
}

#[tauri::command]
pub fn set_hidden(
    path: Option<String>,
    paths: Option<Vec<String>>,
    hidden: bool,
    state: tauri::State<UndoState>,
) -> Result<SetHiddenBatchResult, String> {
    let targets: Vec<String> = match (paths, path) {
        (Some(list), _) if !list.is_empty() => list,
        (_, Some(single)) => vec![single],
        _ => return Err("No paths provided".into()),
    };
    if targets.is_empty() {
        return Err("No paths provided".into());
    }
    let mut per_item: Vec<SetHiddenBatchItem> = Vec::with_capacity(targets.len());
    let mut failures = 0usize;
    let mut unexpected_failures = 0usize;
    let mut performed: Vec<Action> = Vec::new();

    for raw in targets {
        let result: SetHiddenResult<(PathBuf, PathBuf, bool)> = (|| {
            let pb = sanitize_path_nofollow(&raw, true).map_err(SetHiddenError::from)?;
            check_no_symlink_components(&pb).map_err(SetHiddenError::from)?;
            let (new_path, changed) = set_hidden_attr(&pb, hidden)?;
            Ok((pb, new_path, changed))
        })();
        match result {
            Ok((from_path, new_path, changed)) => {
                if changed {
                    #[cfg(target_os = "windows")]
                    performed.push(Action::SetHidden {
                        path: from_path,
                        hidden,
                    });
                    #[cfg(not(target_os = "windows"))]
                    performed.push(Action::Rename {
                        from: from_path,
                        to: new_path.clone(),
                    });
                }
                per_item.push(SetHiddenBatchItem {
                    path: raw,
                    ok: true,
                    new_path: new_path.to_string_lossy().into_owned(),
                    error: None,
                })
            }
            Err(error) => {
                failures += 1;
                if !is_expected_set_hidden_error(&error) {
                    unexpected_failures += 1;
                }
                per_item.push(SetHiddenBatchItem {
                    path: raw.clone(),
                    ok: false,
                    new_path: raw,
                    error: Some(error.to_api_error()),
                });
            }
        }
    }

    if !performed.is_empty() {
        let recorded = if performed.len() == 1 {
            performed.remove(0)
        } else {
            Action::Batch(performed)
        };
        let _ = state.record_applied(recorded);
    }

    Ok(SetHiddenBatchResult {
        per_item,
        failures,
        unexpected_failures,
    })
}

#[tauri::command]
pub fn create_folder(
    path: String,
    name: String,
    state: tauri::State<UndoState>,
) -> Result<String, String> {
    let base = sanitize_path_follow(&path, true)?;
    ensure_existing_dir_nonsymlink(&base)?;
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Folder name cannot be empty".into());
    }
    if trimmed.contains(['/', '\\']) {
        return Err("Folder name cannot contain path separators".into());
    }
    let target = base.join(trimmed);
    if target.exists() {
        return Err("A file or directory with that name already exists".into());
    }
    fs::create_dir(&target).map_err(|e| format!("Failed to create folder: {e}"))?;
    let _ = state.record_applied(Action::CreateFolder {
        path: target.clone(),
    });
    Ok(target.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn create_file(
    path: String,
    name: String,
    state: tauri::State<UndoState>,
) -> Result<String, String> {
    let base = sanitize_path_follow(&path, true)?;
    ensure_existing_dir_nonsymlink(&base)?;

    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("File name cannot be empty".into());
    }
    if trimmed.contains(['/', '\\']) {
        return Err("File name cannot contain path separators".into());
    }

    let target = base.join(trimmed);
    if target.exists() {
        return Err("A file or directory with that name already exists".into());
    }

    fs::File::options()
        .write(true)
        .create_new(true)
        .open(&target)
        .map_err(|e| format!("Failed to create file: {e}"))?;

    let backup = temp_backup_path(&target);
    let _ = state.record_applied(Action::Create {
        path: target.clone(),
        backup,
    });

    Ok(target.to_string_lossy().into_owned())
}

#[derive(Serialize, Clone)]
pub struct DeleteProgressPayload {
    pub bytes: u64,
    pub total: u64,
    pub finished: bool,
}

pub(super) fn should_abort_fs_op(app: &tauri::AppHandle, cancel: Option<&AtomicBool>) -> bool {
    runtime_lifecycle::is_shutting_down(app)
        || cancel
            .map(|flag| flag.load(Ordering::Relaxed))
            .unwrap_or(false)
}
