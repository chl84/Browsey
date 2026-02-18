//! Library-style listings: starred items and recent paths.

use crate::{
    commands::listing::DirListing,
    db,
    entry::{build_entry, normalize_key_for_db},
    errors::api_error::ApiResult,
    sorting::{sort_entries, SortSpec},
};
use error::{map_api_result, LibraryError, LibraryErrorCode, LibraryResult};
use std::collections::HashSet;
use std::{fs, path::PathBuf};
use tracing::error;
#[cfg(debug_assertions)]
use tracing::info;

mod error;

#[tauri::command]
pub fn toggle_star(path: String) -> ApiResult<bool> {
    map_api_result(toggle_star_impl(path))
}

fn toggle_star_impl(path: String) -> LibraryResult<bool> {
    let conn = db::open().map_err(|error| {
        LibraryError::new(
            LibraryErrorCode::DatabaseOpenFailed,
            format!("Failed to open library database: {error}"),
        )
    })?;
    let res = db::toggle_star(&conn, &path).map_err(|error| {
        LibraryError::new(
            LibraryErrorCode::ToggleStarFailed,
            format!("Failed to toggle star: {error}"),
        )
    });
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
pub fn list_starred(sort: Option<SortSpec>) -> ApiResult<DirListing> {
    map_api_result(list_starred_impl(sort))
}

fn list_starred_impl(sort: Option<SortSpec>) -> LibraryResult<DirListing> {
    let conn = db::open().map_err(|error| {
        LibraryError::new(
            LibraryErrorCode::DatabaseOpenFailed,
            format!("Failed to open library database: {error}"),
        )
    })?;
    let entries = db::starred_entries(&conn).map_err(|error| {
        LibraryError::new(
            LibraryErrorCode::ListFailed,
            format!("Failed to list starred entries: {error}"),
        )
    })?;
    let mut out = Vec::new();
    for (p, _) in &entries {
        let pb = PathBuf::from(p);
        if let Ok(meta) = fs::symlink_metadata(&pb) {
            let is_link = meta.file_type().is_symlink();
            out.push(build_entry(&pb, &meta, is_link, true));
        }
    }
    sort_entries(&mut out, sort);
    Ok(DirListing {
        current: "Starred".to_string(),
        entries: out,
    })
}

#[tauri::command]
pub fn list_recent(sort: Option<SortSpec>) -> ApiResult<DirListing> {
    map_api_result(list_recent_impl(sort))
}

fn list_recent_impl(sort: Option<SortSpec>) -> LibraryResult<DirListing> {
    let conn = db::open().map_err(|error| {
        LibraryError::new(
            LibraryErrorCode::DatabaseOpenFailed,
            format!("Failed to open library database: {error}"),
        )
    })?;
    let star_set: HashSet<String> = db::starred_set(&conn).map_err(|error| {
        LibraryError::new(
            LibraryErrorCode::ListFailed,
            format!("Failed to read starred set: {error}"),
        )
    })?;
    let mut out = Vec::new();
    for p in db::recent_paths(&conn).map_err(|error| {
        LibraryError::new(
            LibraryErrorCode::ListFailed,
            format!("Failed to list recent paths: {error}"),
        )
    })? {
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
    Ok(DirListing {
        current: "Recent".to_string(),
        entries: out,
    })
}

#[tauri::command]
pub fn remove_recent(paths: Vec<String>) -> ApiResult<()> {
    map_api_result(remove_recent_impl(paths))
}

fn remove_recent_impl(paths: Vec<String>) -> LibraryResult<()> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut conn = db::open().map_err(|error| {
        LibraryError::new(
            LibraryErrorCode::DatabaseOpenFailed,
            format!("Failed to open library database: {error}"),
        )
    })?;
    db::delete_recent_paths(&mut conn, &paths).map_err(|error| {
        LibraryError::new(
            LibraryErrorCode::DeleteFailed,
            format!("Failed to remove recent entries: {error}"),
        )
    })?;
    Ok(())
}

#[tauri::command]
pub fn clear_stars() -> ApiResult<u64> {
    map_api_result(clear_stars_impl())
}

fn clear_stars_impl() -> LibraryResult<u64> {
    let conn = db::open().map_err(|error| {
        LibraryError::new(
            LibraryErrorCode::DatabaseOpenFailed,
            format!("Failed to open library database: {error}"),
        )
    })?;
    let removed = db::delete_all_starred(&conn).map_err(|error| {
        LibraryError::new(
            LibraryErrorCode::DeleteFailed,
            format!("Failed to clear starred entries: {error}"),
        )
    })?;
    Ok(removed as u64)
}

#[tauri::command]
pub fn clear_recents() -> ApiResult<u64> {
    map_api_result(clear_recents_impl())
}

fn clear_recents_impl() -> LibraryResult<u64> {
    let conn = db::open().map_err(|error| {
        LibraryError::new(
            LibraryErrorCode::DatabaseOpenFailed,
            format!("Failed to open library database: {error}"),
        )
    })?;
    let removed = db::delete_all_recent(&conn).map_err(|error| {
        LibraryError::new(
            LibraryErrorCode::DeleteFailed,
            format!("Failed to clear recent entries: {error}"),
        )
    })?;
    Ok(removed as u64)
}
