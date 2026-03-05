use super::{ListingError, ListingErrorCode, ListingResult};
use crate::{
    db,
    fs_utils::{check_no_symlink_components, debug_log, sanitize_path_follow},
    watcher::{self, WatchState},
};
use std::path::PathBuf;
use sysinfo::Disks;
use tracing::warn;

fn watch_allow_all() -> bool {
    matches!(
        std::env::var("FILEY_WATCH_ALLOW_ALL")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn watch_allowed_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = dirs_next::home_dir() {
        roots.push(home);
    }
    let disks = Disks::new_with_refreshed_list();
    for disk in disks.iter() {
        let mount_point = disk.mount_point().to_path_buf();
        if mount_point.as_os_str().is_empty() || !mount_point.exists() {
            continue;
        }
        roots.push(mount_point);
    }
    if let Ok(extra) = std::env::var("FILEY_WATCH_EXTRA_ROOTS") {
        for part in extra.split(std::path::MAIN_SEPARATOR) {
            if part.is_empty() {
                continue;
            }
            let pb = PathBuf::from(part);
            if pb.exists() {
                roots.push(pb);
            }
        }
    }
    // Allow GVFS mounts (e.g., MTP) even if we don't get filesystem events; watcher will be best-effort.
    if let Some(gvfs_root) = dirs_next::runtime_dir().map(|p| p.join("gvfs")) {
        if gvfs_root.exists() {
            roots.push(gvfs_root);
        }
    }
    if let Ok(conn) = db::open() {
        if let Ok(bookmarks) = db::list_bookmarks(&conn) {
            for (_label, path) in bookmarks {
                match sanitize_path_follow(&path, false) {
                    Ok(pb) => roots.push(pb),
                    Err(e) => debug_log(&format!("Skipping bookmark path {path}: {e}")),
                }
            }
        }
    }
    roots
}

pub(super) fn watch_dir_impl(
    path: Option<String>,
    state: tauri::State<WatchState>,
    app: tauri::AppHandle,
) -> ListingResult<()> {
    let base_path = crate::commands::fs::expand_path(path).map_err(ListingError::from)?;
    let target = match sanitize_path_follow(&base_path.to_string_lossy(), true) {
        Ok(p) if p.exists() => p,
        _ => {
            let home = dirs_next::home_dir().ok_or_else(|| {
                ListingError::new(ListingErrorCode::InvalidInput, "Start directory not found")
            })?;
            sanitize_path_follow(&home.to_string_lossy(), true).map_err(ListingError::from)?
        }
    };

    check_no_symlink_components(&target).map_err(ListingError::from)?;

    if !watch_allow_all() {
        let allowed = watch_allowed_roots();
        let in_allowed = allowed.iter().any(|root| target.starts_with(root));
        if !in_allowed {
            return Err(ListingError::new(
                ListingErrorCode::WatchNotAllowed,
                "Watching this path is not allowed",
            ));
        }
    }

    if let Err(e) = watcher::start_watch(app, target.clone(), &state) {
        warn!(
            error = %e,
            path = %target.to_string_lossy(),
            "watch_dir failed; continuing without file watcher"
        );
        debug_log(&format!(
            "watch_dir failed: path={} error={:?}",
            target.display(),
            e
        ));
    }

    Ok(())
}
