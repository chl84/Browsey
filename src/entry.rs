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
    let modified = meta.modified().ok().and_then(|t: SystemTime| {
        DateTime::<Local>::from(t)
            .format("%Y-%m-%d %H:%M")
            .to_string()
            .into()
    });

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
