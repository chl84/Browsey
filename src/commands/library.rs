//! Library-style listings: starred items and recent paths.

use crate::{
    db,
    entry::{build_entry, FsEntry},
    sorting::{sort_entries, SortSpec},
};
use std::collections::HashSet;
use std::{fs, path::PathBuf};
use tracing::{error, info};

#[tauri::command]
pub fn toggle_star(path: String) -> Result<bool, String> {
    let conn = db::open()?;
    let res = db::toggle_star(&conn, &path);
    match &res {
        Ok(state) => info!("Toggled star for {} -> {}", path, state),
        Err(e) => error!("Failed to toggle star for {}: {}", path, e),
    }
    res
}

#[tauri::command]
pub fn list_starred(sort: Option<SortSpec>) -> Result<Vec<FsEntry>, String> {
    let conn = db::open()?;
    let entries = db::starred_entries(&conn)?;
    let mut out = Vec::new();
    for (p, _) in &entries {
        let pb = PathBuf::from(p);
        if let Ok(meta) = fs::symlink_metadata(&pb) {
            let is_link = meta.file_type().is_symlink();
            out.push(build_entry(&pb, &meta, is_link, true));
        }
    }
    sort_entries(&mut out, sort);
    Ok(out)
}

#[tauri::command]
pub fn list_recent(sort: Option<SortSpec>) -> Result<Vec<FsEntry>, String> {
    let conn = db::open()?;
    let star_set: HashSet<String> = db::starred_set(&conn)?;
    let mut out = Vec::new();
    for p in db::recent_paths(&conn)? {
        let pb = PathBuf::from(&p);
        if let Ok(meta) = fs::symlink_metadata(&pb) {
            let is_link = meta.file_type().is_symlink();
            let starred = star_set.contains(&p);
            out.push(build_entry(&pb, &meta, is_link, starred));
        }
    }
    if sort.is_some() {
        sort_entries(&mut out, sort);
    }
    Ok(out)
}
