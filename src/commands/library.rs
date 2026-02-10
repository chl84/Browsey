//! Library-style listings: starred items and recent paths.

use crate::{
    db,
    entry::{build_entry, normalize_key_for_db, FsEntry},
    sorting::{sort_entries, SortSpec},
};
use std::collections::HashSet;
use std::{fs, path::PathBuf};
use tracing::error;
#[cfg(debug_assertions)]
use tracing::info;

#[tauri::command]
pub fn toggle_star(path: String) -> Result<bool, String> {
    let conn = db::open()?;
    let res = db::toggle_star(&conn, &path);
    match &res {
        Ok(_st) => {
            #[cfg(debug_assertions)]
            info!("Toggled star for {} -> {}", path, _st)
        }
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
            let starred = star_set.contains(&normalize_key_for_db(&pb));
            out.push(build_entry(&pb, &meta, is_link, starred));
        }
    }
    if sort.is_some() {
        sort_entries(&mut out, sort);
    }
    Ok(out)
}

#[tauri::command]
pub fn remove_recent(paths: Vec<String>) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut conn = db::open()?;
    db::delete_recent_paths(&mut conn, &paths)?;
    Ok(())
}

#[tauri::command]
pub fn clear_stars() -> Result<u64, String> {
    let conn = db::open()?;
    let removed = db::delete_all_starred(&conn)?;
    Ok(removed as u64)
}

#[tauri::command]
pub fn clear_recents() -> Result<u64, String> {
    let conn = db::open()?;
    let removed = db::delete_all_recent(&conn)?;
    Ok(removed as u64)
}
