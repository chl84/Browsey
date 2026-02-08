//! Duplicate detection commands.
//!
//! Current strategy:
//! 1) coarse filter by byte length
//! 2) byte-for-byte comparison for same-length candidates (early exit on mismatch)

mod scan;

use crate::{
    commands::fs::expand_path,
    fs_utils::{check_no_symlink_components, sanitize_path_follow},
};

#[tauri::command]
pub async fn check_duplicates(target_path: String, start_path: String) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || check_duplicates_sync(target_path, start_path))
        .await
        .unwrap_or_else(|e| Err(format!("duplicate scan task panicked: {e}")))
}

fn check_duplicates_sync(target_path: String, start_path: String) -> Result<Vec<String>, String> {
    let target = sanitize_path_follow(&target_path, false)?;
    check_no_symlink_components(&target)?;

    let target_meta = std::fs::symlink_metadata(&target)
        .map_err(|e| format!("Failed to read target metadata: {e}"))?;
    if target_meta.file_type().is_symlink() {
        return Err("Target must be a regular file (symlinks are ignored)".into());
    }
    if !target_meta.is_file() {
        return Err("Target must be a file".into());
    }

    let start_expanded = expand_path(Some(start_path))?;
    let start = sanitize_path_follow(&start_expanded.to_string_lossy(), false)?;
    check_no_symlink_components(&start)?;

    let start_meta = std::fs::symlink_metadata(&start)
        .map_err(|e| format!("Failed to read start folder metadata: {e}"))?;
    if start_meta.file_type().is_symlink() {
        return Err("Start path must be a directory (symlinks are ignored)".into());
    }
    if !start_meta.is_dir() {
        return Err("Start path must be a directory".into());
    }

    let matches = scan::find_identical_files(&target, &start, target_meta.len());
    Ok(matches
        .into_iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect())
}
