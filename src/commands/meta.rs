//! Metadata helpers used by the properties modal.

use crate::entry::{entry_times, EntryTimes};
use std::path::PathBuf;

#[tauri::command]
pub fn entry_times_cmd(path: String) -> Result<EntryTimes, String> {
    let pb = PathBuf::from(path);
    entry_times(&pb)
}
