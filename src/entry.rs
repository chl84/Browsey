use chrono::{DateTime, Local};
use serde::Serialize;
use std::fs::{self, Metadata};
use std::path::Path;
use std::time::SystemTime;

use crate::icons::icon_for;

#[derive(Serialize, Clone)]
pub struct FsEntry {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub ext: Option<String>,
    pub size: Option<u64>,
    pub items: Option<u64>,
    pub modified: Option<String>,
    pub icon: String,
    pub starred: bool,
}

#[derive(Serialize, Clone)]
pub struct EntryTimes {
    pub accessed: Option<String>,
    pub created: Option<String>,
    pub modified: Option<String>,
}

fn dir_item_count(path: &Path) -> Option<u64> {
    match fs::read_dir(path) {
        Ok(iter) => {
            let mut count: u64 = 0;
            for entry in iter {
                if entry.is_ok() {
                    count = count.saturating_add(1);
                }
            }
            Some(count)
        }
        Err(_) => None,
    }
}

fn fmt_time(value: Option<SystemTime>) -> Option<String> {
    value.and_then(|t| {
        DateTime::<Local>::from(t)
            .format("%Y-%m-%d %H:%M")
            .to_string()
            .into()
    })
}

pub fn build_entry(path: &Path, meta: &Metadata, is_link: bool, starred: bool) -> FsEntry {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());

    let kind = if is_link {
        "link"
    } else if meta.is_dir() {
        "dir"
    } else {
        "file"
    }
    .to_string();

    let size = if meta.is_file() {
        Some(meta.len())
    } else {
        None
    };
    let items = if meta.is_dir() {
        dir_item_count(path)
    } else {
        None
    };
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_string());
    let modified = fmt_time(meta.modified().ok());

    FsEntry {
        name,
        path: path.to_string_lossy().into_owned(),
        kind,
        ext,
        size,
        items,
        modified,
        icon: icon_for(path, meta, is_link).to_string(),
        starred,
    }
}

pub fn entry_times(path: &Path) -> Result<EntryTimes, String> {
    let meta = fs::symlink_metadata(path).map_err(|e| format!("Failed to read metadata: {e}"))?;
    Ok(EntryTimes {
        accessed: fmt_time(meta.accessed().ok()),
        created: fmt_time(meta.created().ok()),
        modified: fmt_time(meta.modified().ok()),
    })
}
