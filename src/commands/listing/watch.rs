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

fn extra_watch_roots(extra: &std::ffi::OsStr) -> Vec<PathBuf> {
    std::env::split_paths(extra)
        .filter(|path| !path.as_os_str().is_empty())
        .filter_map(|path| path.canonicalize().ok())
        .filter(|path| path.is_dir())
        .collect()
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
    if let Some(extra) = std::env::var_os("FILEY_WATCH_EXTRA_ROOTS") {
        roots.extend(extra_watch_roots(extra.as_ref()));
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
    #[cfg(feature = "native-test")]
    if crate::native_test::watchers_enabled() {
        // This mode never calls expand_path, home fallback or broad discovery.
        let target = crate::native_test::owned_watch_path(path.as_deref())
            .map_err(|message| ListingError::new(ListingErrorCode::WatchNotAllowed, message))?;
        return watcher::start_watch(app, target, &state).map_err(|error| {
            ListingError::new(
                ListingErrorCode::TaskFailed,
                format!("Owned watcher failed: {error}"),
            )
        });
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn fixture() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "browsey-extra-watch-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        fs::create_dir(&path).unwrap();
        path
    }

    #[test]
    fn a_single_absolute_extra_root_is_preserved() {
        let root = fixture();
        let expected = fs::canonicalize(&root).unwrap();
        let actual = extra_watch_roots(root.as_os_str());
        fs::remove_dir_all(root).unwrap();
        assert_eq!(actual, [expected]);
    }

    #[test]
    fn extra_roots_use_the_platform_path_list_separator_and_canonical_directories() {
        let root = fixture();
        let first = root.join("first folder");
        let second = root.join("andre_æøå");
        let not_a_directory = root.join("file.txt");
        fs::create_dir(&first).unwrap();
        fs::create_dir(&second).unwrap();
        fs::write(&not_a_directory, b"fixture").unwrap();
        let paths = std::env::join_paths([
            first.clone(),
            second.clone(),
            root.join("missing"),
            not_a_directory,
        ])
        .unwrap();
        let actual = extra_watch_roots(&paths);
        let expected = [
            fs::canonicalize(first).unwrap(),
            fs::canonicalize(second).unwrap(),
        ];
        fs::remove_dir_all(root).unwrap();
        assert_eq!(actual, expected);
    }
}
