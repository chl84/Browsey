use chrono::{DateTime, Local};
use serde::Serialize;
use std::fs::{self, Metadata, ReadDir};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::icons::icon_for;

#[derive(Serialize, Clone)]
pub struct FsEntry {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub ext: Option<String>,
    pub size: Option<u64>,
    pub modified: Option<String>,
    pub icon: String,
}

pub fn build_entry(path: &Path, meta: &Metadata, is_link: bool) -> FsEntry {
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
    let size = if meta.is_file() { Some(meta.len()) } else { None };
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_string());
    let modified = meta
        .modified()
        .ok()
        .and_then(|t: SystemTime| DateTime::<Local>::from(t).format("%Y-%m-%d %H:%M").to_string().into());

    FsEntry {
        name,
        path: path.to_string_lossy().into_owned(),
        kind,
        ext,
        size,
        modified,
        icon: icon_for(path, meta, is_link).to_string(),
    }
}

pub fn search_recursive(root: PathBuf, query: String) -> Result<Vec<FsEntry>, String> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }

    let mut results = Vec::new();
    let mut stack = vec![root];
    let needle = query.to_lowercase();

    while let Some(dir) = stack.pop() {
        let iter: ReadDir = match fs::read_dir(&dir) {
            Ok(i) => i,
            Err(_) => continue,
        };

        for entry in iter {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let path = entry.path();
            let meta = match fs::symlink_metadata(&path) {
                Ok(m) => m,
                Err(_) => continue,
            };

            let is_link = meta.file_type().is_symlink();
            let name_lc = entry
                .file_name()
                .to_string_lossy()
                .to_lowercase();
            let path_lc = path.to_string_lossy().to_lowercase();
            let is_dir = meta.is_dir();

            if name_lc.contains(&needle) || path_lc.contains(&needle) {
                results.push(build_entry(&path, &meta, is_link));
            }

            // Only traverse real directories (not symlinks) to avoid loops.
            if is_dir && !is_link {
                stack.push(path);
            }
        }
    }

    // Optional: sort folders before files, then name
    results.sort_by(|a, b| match (a.kind.as_str(), b.kind.as_str()) {
        ("dir", "file") | ("dir", "link") => std::cmp::Ordering::Less,
        ("link", "dir") | ("file", "dir") => std::cmp::Ordering::Greater,
        ("link", "file") => std::cmp::Ordering::Less,
        ("file", "link") => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    Ok(results)
}
