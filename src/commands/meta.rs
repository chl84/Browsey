//! Metadata helpers used by the properties modal.

use crate::entry::{entry_times, EntryTimes};
use crate::fs_utils::sanitize_path_follow;
use std::fs;
use std::path::PathBuf;

#[tauri::command]
pub fn entry_times_cmd(path: String) -> Result<EntryTimes, String> {
    let pb = PathBuf::from(path);
    entry_times(&pb)
}

#[tauri::command]
pub fn entry_kind_cmd(path: String) -> Result<String, String> {
    let pb = sanitize_path_follow(&path, false)?;
    let meta = fs::metadata(&pb).map_err(|e| format!("Failed to read metadata: {e}"))?;
    if meta.is_dir() {
        Ok("dir".into())
    } else {
        Ok("file".into())
    }
}
