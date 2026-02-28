//! Library-style listings: starred items and recent paths.

use crate::{
    commands::listing::DirListing,
    db,
    entry::{
        build_entry, get_cached_meta, is_network_location, normalize_key_for_db, store_cached_meta,
    },
    errors::api_error::ApiResult,
    sorting::{sort_entries, SortSpec},
};
use error::{map_api_result, LibraryError, LibraryErrorCode, LibraryResult};
use std::collections::HashSet;
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::mpsc,
    time::Duration,
};
#[cfg(debug_assertions)]
use tracing::info;
use tracing::{error, warn};

mod error;

const RECENT_META_CACHE_TTL: Duration = Duration::from_secs(30);
const RECENT_NETWORK_METADATA_TIMEOUT: Duration = Duration::from_millis(150);

enum RecentMetadataProbe {
    Available(fs::Metadata),
    Missing,
    TimedOut,
    Failed(io::Error),
}

fn should_prune_recent_error(error: &io::Error) -> bool {
    matches!(error.kind(), io::ErrorKind::NotFound)
        || matches!(error.raw_os_error(), Some(2) | Some(3) | Some(20))
}

fn probe_recent_metadata(path: &Path) -> RecentMetadataProbe {
    let result = if is_network_location(path) {
        let path = path.to_path_buf();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(fs::symlink_metadata(&path));
        });
        match rx.recv_timeout(RECENT_NETWORK_METADATA_TIMEOUT) {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => return RecentMetadataProbe::TimedOut,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return RecentMetadataProbe::Failed(io::Error::other(
                    "recent metadata probe channel closed",
                ))
            }
        }
    } else {
        fs::symlink_metadata(path)
    };

    match result {
        Ok(meta) => RecentMetadataProbe::Available(meta),
        Err(error) if should_prune_recent_error(&error) => RecentMetadataProbe::Missing,
        Err(error) => RecentMetadataProbe::Failed(error),
    }
}

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
    let mut conn = db::open().map_err(|error| {
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
    let recent_paths = db::recent_paths(&conn).map_err(|error| {
        LibraryError::new(
            LibraryErrorCode::ListFailed,
            format!("Failed to list recent paths: {error}"),
        )
    })?;
    let mut out = Vec::new();
    let mut stale_paths = Vec::new();
    for p in recent_paths {
        let pb = PathBuf::from(&p);
        let starred = star_set.contains(&normalize_key_for_db(&pb));

        if is_network_location(&pb) {
            if let Some(cached) = get_cached_meta(&pb, RECENT_META_CACHE_TTL) {
                out.push(crate::commands::fs::entry_from_cached(
                    &pb, &cached, starred,
                ));
                continue;
            }
        }

        match probe_recent_metadata(&pb) {
            RecentMetadataProbe::Available(meta) => {
                let is_link = meta.file_type().is_symlink();
                store_cached_meta(&pb, &meta, is_link);
                out.push(build_entry(&pb, &meta, is_link, starred));
            }
            RecentMetadataProbe::Missing => stale_paths.push(p),
            RecentMetadataProbe::TimedOut => {
                warn!("Skipping slow recent path probe for {}", pb.display());
            }
            RecentMetadataProbe::Failed(error) => {
                warn!(
                    "Skipping recent path {} after metadata failure: {}",
                    pb.display(),
                    error
                );
            }
        }
    }
    if !stale_paths.is_empty() {
        if let Err(error) = db::delete_recent_paths(&mut conn, &stale_paths) {
            warn!("Failed to prune stale recent entries: {}", error);
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

#[cfg(test)]
mod tests {
    use super::should_prune_recent_error;
    use std::io;

    #[test]
    fn prunes_not_found_recent_errors() {
        let error = io::Error::new(io::ErrorKind::NotFound, "missing");
        assert!(should_prune_recent_error(&error));
    }

    #[test]
    fn keeps_permission_denied_recent_errors() {
        let error = io::Error::new(io::ErrorKind::PermissionDenied, "denied");
        assert!(!should_prune_recent_error(&error));
    }
}
