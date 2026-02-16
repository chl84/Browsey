//! File-system oriented Tauri commands: listings, mounts, trash, opening, renaming, deleting, and watch wiring.

use super::tasks::CancelState;
use crate::{
    entry::{CachedMeta, FsEntry},
    fs_utils::{check_no_symlink_components, sanitize_path_follow, sanitize_path_nofollow},
    runtime_lifecycle,
    undo::{
        assert_path_snapshot, is_destination_exists_error, move_with_fallback, run_actions,
        snapshot_existing_path, temp_backup_path, Action, Direction, UndoState,
    },
};
#[path = "fs/delete_ops.rs"]
mod delete_ops;
#[cfg(target_os = "windows")]
#[path = "fs_windows.rs"]
pub mod fs_windows;
#[cfg(not(target_os = "windows"))]
pub mod gvfs;
#[path = "fs/open_ops.rs"]
mod open_ops;
#[path = "fs/path_guard.rs"]
mod path_guard;
#[path = "fs/trash_ops.rs"]
mod trash_ops;

pub use crate::commands::listing::DirListing;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
#[cfg(target_os = "windows")]
use std::os::windows::prelude::*;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

pub use delete_ops::{delete_entries, delete_entry};
pub use open_ops::open_entry;
pub use trash_ops::{
    cleanup_stale_trash_staging, list_trash, move_to_trash, move_to_trash_many, purge_trash_items,
    restore_trash_items,
};

use path_guard::{ensure_existing_dir_nonsymlink, ensure_existing_path_nonsymlink};

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
fn set_hidden_attr(path: &Path, hidden: bool) -> Result<PathBuf, String> {
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
        return Err("GetFileAttributes failed".into());
    }
    let mut new_attrs = attrs;
    if hidden {
        new_attrs |= FILE_ATTRIBUTE_HIDDEN;
    } else {
        new_attrs &= !FILE_ATTRIBUTE_HIDDEN;
    }
    let ok = unsafe { SetFileAttributesW(wide.as_ptr(), new_attrs) };
    if ok == 0 {
        return Err("SetFileAttributes failed".into());
    }
    Ok(path.to_path_buf())
}

#[cfg(not(target_os = "windows"))]
fn set_hidden_attr(path: &Path, hidden: bool) -> Result<PathBuf, String> {
    // On Unix, hidden = leading dot. Rename within same directory.
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Invalid file name".to_string())?;
    let is_dot = file_name.starts_with('.');
    if hidden == is_dot {
        return Ok(path.to_path_buf());
    }
    let target_name = if hidden {
        format!(".{file_name}")
    } else {
        file_name.trim_start_matches('.').to_string()
    };
    if target_name.is_empty() {
        return Err("Cannot derive visible name".into());
    }
    let parent = path.parent().ok_or_else(|| "Missing parent".to_string())?;
    let target = parent.join(&target_name);
    if target == path {
        return Ok(path.to_path_buf());
    }
    match move_with_fallback(path, &target) {
        Ok(_) => {}
        Err(e) if is_destination_exists_error(&e) => {
            return Err(format!("Target already exists: {}", target.display()));
        }
        Err(e) => {
            return Err(format!("Failed to rename: {e}"));
        }
    }
    Ok(target)
}

#[tauri::command]
pub fn set_hidden(
    path: Option<String>,
    paths: Option<Vec<String>>,
    hidden: bool,
) -> Result<Vec<String>, String> {
    let targets: Vec<String> = match (paths, path) {
        (Some(list), _) if !list.is_empty() => list,
        (_, Some(single)) => vec![single],
        _ => return Err("No paths provided".into()),
    };
    if targets.is_empty() {
        return Err("No paths provided".into());
    }
    let mut results = Vec::with_capacity(targets.len());
    for raw in targets {
        let pb = sanitize_path_nofollow(&raw, true)?;
        check_no_symlink_components(&pb)?;
        let new_path = set_hidden_attr(&pb, hidden)?;
        results.push(new_path.to_string_lossy().into_owned());
    }
    Ok(results)
}

#[tauri::command]
pub fn rename_entry(
    path: String,
    new_name: String,
    state: tauri::State<UndoState>,
) -> Result<String, String> {
    let (from, to) = prepare_rename_pair(path.as_str(), new_name.as_str())?;
    apply_rename(&from, &to)?;
    let _ = state.record_applied(Action::Rename {
        from: from.clone(),
        to: to.clone(),
    });
    Ok(to.to_string_lossy().to_string())
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameEntryRequest {
    pub path: String,
    pub new_name: String,
}

fn build_rename_target(from: &Path, new_name: &str) -> Result<PathBuf, String> {
    if new_name.trim().is_empty() {
        return Err("New name cannot be empty".into());
    }
    let parent = from
        .parent()
        .ok_or_else(|| "Cannot rename root".to_string())?;
    Ok(parent.join(new_name.trim()))
}

fn prepare_rename_pair(path: &str, new_name: &str) -> Result<(PathBuf, PathBuf), String> {
    let from = sanitize_path_nofollow(path, true)?;
    let to = build_rename_target(&from, new_name)?;
    Ok((from, to))
}

fn apply_rename(from: &Path, to: &Path) -> Result<(), String> {
    ensure_existing_path_nonsymlink(from)?;
    let from_snapshot = snapshot_existing_path(from)?;
    if let Some(parent) = to.parent() {
        ensure_existing_dir_nonsymlink(parent)?;
        let parent_snapshot = snapshot_existing_path(parent)?;
        assert_path_snapshot(parent, &parent_snapshot)?;
    } else {
        return Err("Invalid destination path".into());
    }
    assert_path_snapshot(from, &from_snapshot)?;
    match move_with_fallback(from, to) {
        Ok(_) => Ok(()),
        Err(e) if is_destination_exists_error(&e) => {
            Err("A file or directory with that name already exists".into())
        }
        Err(e) => Err(format!("Failed to rename: {e}")),
    }
}

#[tauri::command]
pub fn rename_entries(
    entries: Vec<RenameEntryRequest>,
    undo: tauri::State<UndoState>,
) -> Result<Vec<String>, String> {
    if entries.is_empty() {
        return Ok(Vec::new());
    }

    let mut pairs: Vec<(PathBuf, PathBuf)> = Vec::with_capacity(entries.len());
    let mut seen_sources: HashSet<PathBuf> = HashSet::with_capacity(entries.len());
    let mut seen_targets: HashSet<PathBuf> = HashSet::with_capacity(entries.len());

    for (idx, entry) in entries.into_iter().enumerate() {
        let (from, to) = prepare_rename_pair(entry.path.as_str(), entry.new_name.as_str())?;

        if !seen_sources.insert(from.clone()) {
            return Err(format!(
                "Duplicate source path in request (item {})",
                idx + 1
            ));
        }
        if !seen_targets.insert(to.clone()) {
            return Err(format!(
                "Duplicate target name in request (item {})",
                idx + 1
            ));
        }

        pairs.push((from, to));
    }

    let mut performed: Vec<Action> = Vec::new();
    let mut renamed_paths: Vec<String> = Vec::with_capacity(pairs.len());

    for (from, to) in pairs {
        if from == to {
            continue;
        }
        if let Err(err) = apply_rename(&from, &to) {
            if !performed.is_empty() {
                let mut rollback = performed.clone();
                if let Err(rb_err) = run_actions(&mut rollback, Direction::Backward) {
                    return Err(format!("{}; rollback also failed: {}", err, rb_err));
                }
            }
            return Err(err);
        }

        renamed_paths.push(to.to_string_lossy().to_string());
        performed.push(Action::Rename {
            from: from.clone(),
            to: to.clone(),
        });
    }

    if !performed.is_empty() {
        let recorded = if performed.len() == 1 {
            performed.pop().unwrap()
        } else {
            Action::Batch(performed)
        };
        let _ = undo.record_applied(recorded);
    }

    Ok(renamed_paths)
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
