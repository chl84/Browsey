//! Directory listing and watcher wiring.

use crate::{
    db,
    entry::{
        build_entry, get_cached_meta, is_network_location, normalize_key_for_db, store_cached_meta,
        FsEntry,
    },
    fs_utils::{check_no_symlink_components, debug_log, sanitize_path_follow},
    icons::icon_ids::{FILE, GENERIC_FOLDER, SHORTCUT},
    sorting::{sort_entries, SortSpec},
    watcher::{self, WatchState},
};
use chrono::{Local, NaiveDateTime};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::{
    fs, io,
    path::{Path, PathBuf},
};
use sysinfo::Disks;
use tracing::warn;

#[cfg(target_os = "windows")]
use std::os::windows::prelude::*;

const META_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(30);

#[derive(Serialize)]
pub struct DirListing {
    pub current: String,
    pub entries: Vec<FsEntry>,
}

fn entry_type_label(e: &FsEntry) -> String {
    if let Some(ext) = &e.ext {
        if !ext.is_empty() {
            return ext.to_lowercase();
        }
    }
    e.kind.to_lowercase()
}

fn collect_column_values(entries: &[FsEntry], column: &str, include_hidden: bool) -> Vec<String> {
    let mut set = std::collections::HashSet::new();
    let mut buckets: HashMap<String, i64> = HashMap::new();
    match column {
        "name" => {
            for e in entries {
                if !include_hidden && e.hidden {
                    continue;
                }
                let lower = e.name.to_lowercase();
                let (label, rank) = bucket_name(lower.as_str());
                let entry = buckets.entry(label.to_string()).or_insert(rank);
                if rank < *entry {
                    *entry = rank;
                }
            }
            let mut v: Vec<(String, i64)> = buckets.into_iter().collect();
            v.sort_by_key(|(_, rank)| *rank);
            return v.into_iter().map(|(label, _)| label).collect();
        }
        "type" => {
            for e in entries {
                if !include_hidden && e.hidden {
                    continue;
                }
                set.insert(entry_type_label(e));
            }
        }
        "size" => {
            for e in entries {
                if !include_hidden && e.hidden {
                    continue;
                }
                if e.kind != "file" {
                    continue;
                }
                if let Some(size) = e.size {
                    let (label, rank) = bucket_size(size);
                    let entry = buckets.entry(label).or_insert(rank);
                    if rank < *entry {
                        *entry = rank;
                    }
                }
            }
            let mut v: Vec<(String, i64)> = buckets.into_iter().collect();
            v.sort_by_key(|(_, rank)| *rank);
            return v.into_iter().map(|(label, _)| label).collect();
        }
        "modified" => {
            let now = Local::now().naive_local();
            for e in entries {
                if !include_hidden && e.hidden {
                    continue;
                }
                if let Some(mod_str) = &e.modified {
                    if let Ok(dt) = NaiveDateTime::parse_from_str(mod_str, "%Y-%m-%d %H:%M") {
                        let (label, days) = bucket_modified(dt, now);
                        let entry = buckets.entry(label).or_insert(days);
                        if days < *entry {
                            *entry = days;
                        }
                    }
                }
            }
            let mut v: Vec<(String, i64)> = buckets.into_iter().collect();
            v.sort_by_key(|(_, days)| *days);
            return v.into_iter().map(|(label, _)| label).collect();
        }
        _ => {}
    }
    let mut v: Vec<String> = set.into_iter().collect();
    v.sort_unstable();
    v
}

fn bucket_modified(dt: NaiveDateTime, now: NaiveDateTime) -> (String, i64) {
    let diff = now - dt;
    let days = diff.num_days();
    if days <= 0 {
        return ("Today".to_string(), 0);
    }
    if days == 1 {
        return ("Yesterday".to_string(), 1);
    }
    if days < 7 {
        return (format!("{days} days ago"), days);
    }
    if days < 30 {
        let weeks = (days + 6) / 7;
        let label = if weeks == 1 {
            "1 week ago".to_string()
        } else {
            format!("{weeks} weeks ago")
        };
        return (label, weeks * 7);
    }
    if days < 365 {
        let months = (days + 29) / 30;
        let label = if months == 1 {
            "1 month ago".to_string()
        } else {
            format!("{months} months ago")
        };
        return (label, months * 30);
    }
    let years = (days + 364) / 365;
    let label = if years == 1 {
        "1 year ago".to_string()
    } else {
        format!("{years} years ago")
    };
    (label, years * 365)
}

fn bucket_size(size: u64) -> (String, i64) {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    const TB: u64 = 1024 * GB;

    let buckets = [
        (10 * KB, "0–10 KB"),
        (100 * KB, "10–100 KB"),
        (MB, "100 KB–1 MB"),
        (10 * MB, "1–10 MB"),
        (100 * MB, "10–100 MB"),
        (GB, "100 MB–1 GB"),
        (10 * GB, "1–10 GB"),
        (100 * GB, "10–100 GB"),
        (TB, "100 GB–1 TB"),
    ];

    for (limit, label) in buckets.iter() {
        if size <= *limit {
            return (label.to_string(), *limit as i64);
        }
    }
    ("Over 1 TB".to_string(), (size / TB) as i64 * (TB as i64))
}

fn bucket_name(value: &str) -> (&'static str, i64) {
    let ch = value.chars().next().unwrap_or('\0');
    if ('a'..='f').contains(&ch) {
        return ("name:a-f", 0);
    }
    if ('g'..='l').contains(&ch) {
        return ("name:g-l", 1);
    }
    if ('m'..='r').contains(&ch) {
        return ("name:m-r", 2);
    }
    if ('s'..='z').contains(&ch) {
        return ("name:s-z", 3);
    }
    if ('0'..='9').contains(&ch) {
        return ("name:0-9", 4);
    }
    ("name:other", 5)
}

fn display_path_unix(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(target_os = "windows")]
fn display_path_windows(path: &Path) -> String {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        return rest.to_string();
    }
    s.into_owned()
}

fn display_path(path: &Path) -> String {
    #[cfg(target_os = "windows")]
    {
        return display_path_windows(path);
    }
    #[cfg(not(target_os = "windows"))]
    {
        return display_path_unix(path);
    }
}

#[cfg(target_os = "windows")]
fn read_dir_resilient(target: &Path) -> Result<fs::ReadDir, std::io::Error> {
    match fs::read_dir(target) {
        Ok(rd) => Ok(rd),
        Err(err) => {
            // Retry with a canonical path for common network-related errors that can show up on DFS-mapped drives.
            let retry = matches!(
                err.raw_os_error(),
                Some(59)   // ERROR_UNEXP_NET_ERR
                    | Some(64)   // ERROR_NETNAME_DELETED
                    | Some(67)   // ERROR_BAD_NET_NAME
                    | Some(1219) // ERROR_SESSION_CREDENTIAL_CONFLICT
                    | Some(1231) // ERROR_NETWORK_UNREACHABLE
                    | Some(1232) // ERROR_HOST_UNREACHABLE
            );
            if retry {
                if let Ok(canon) = std::fs::canonicalize(target) {
                    if canon != target {
                        debug_log(&format!(
                            "read_dir retry: orig={} canon={} err={:?}",
                            target.display(),
                            canon.display(),
                            err
                        ));
                        if let Ok(rd) = fs::read_dir(&canon) {
                            return Ok(rd);
                        }
                    }
                }
            }
            Err(err)
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn read_dir_resilient(target: &Path) -> Result<fs::ReadDir, std::io::Error> {
    fs::read_dir(target)
}

fn stub_entry(path: &Path, file_type: Option<fs::FileType>, starred: bool) -> FsEntry {
    let is_link = file_type
        .as_ref()
        .map(|ft| ft.is_symlink())
        .unwrap_or(false);
    let is_dir = file_type.as_ref().map(|ft| ft.is_dir()).unwrap_or(!is_link);
    let kind = if is_link {
        "link"
    } else if is_dir {
        "dir"
    } else {
        "file"
    }
    .to_string();
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_string());
    let icon_id = if is_link {
        SHORTCUT
    } else if is_dir {
        GENERIC_FOLDER
    } else {
        FILE
    };

    FsEntry {
        name,
        path: path.to_string_lossy().into_owned(),
        kind,
        ext,
        size: None,
        items: None,
        modified: None,
        original_path: None,
        trash_id: None,
        icon_id,
        starred,
        hidden: path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.starts_with('.'))
            .unwrap_or(false),
        network: is_network_location(path),
        read_only: false,
        read_denied: false,
    }
}

fn spawn_meta_refresh(app: tauri::AppHandle, jobs: Vec<(PathBuf, Option<fs::FileType>, bool)>) {
    if jobs.is_empty() {
        return;
    }
    let Some(activity_guard) = crate::runtime_lifecycle::try_enter_background_job_from_app(&app)
    else {
        return;
    };
    tauri::async_runtime::spawn_blocking(move || {
        let _activity_guard = activity_guard;
        let mut batch: Vec<FsEntry> = Vec::with_capacity(128);
        for (idx, (path, _file_type, starred)) in jobs.into_iter().enumerate() {
            if crate::runtime_lifecycle::is_shutting_down(&app) {
                break;
            }
            let meta = match fs::symlink_metadata(&path) {
                Ok(m) => m,
                Err(e) => {
                    debug_log(&format!(
                        "symlink_metadata failed (meta refresh): path={} error={:?}",
                        path.display(),
                        e
                    ));
                    continue;
                }
            };
            let is_link = meta.file_type().is_symlink();
            store_cached_meta(&path, &meta, is_link);
            batch.push(build_entry(&path, &meta, is_link, starred));
            if batch.len() >= 128 {
                let _ = crate::runtime_lifecycle::emit_if_running(&app, "entry-meta-batch", &batch);
                batch.clear();
            }
            if idx % 512 == 511 {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
        if !batch.is_empty() {
            let _ = crate::runtime_lifecycle::emit_if_running(&app, "entry-meta-batch", &batch);
        }
    });
}

fn list_dir_sync(
    path: Option<String>,
    sort: Option<SortSpec>,
    app: tauri::AppHandle,
) -> Result<DirListing, String> {
    let base_path = crate::commands::fs::expand_path(path)?;
    let target = sanitize_path_follow(&base_path.to_string_lossy(), false)?;
    debug_log(&format!(
        "list_dir read_dir attempt: path={} normalized={}",
        base_path.display(),
        target.display()
    ));

    let star_conn = db::open()?;
    let star_set: HashSet<String> = db::starred_set(&star_conn)?;

    let mut entries = Vec::new();
    let mut pending_meta = Vec::new();
    let mut pending_seen: HashSet<PathBuf> = HashSet::new();
    let read_dir = read_dir_resilient(&target).map_err(|e| {
        tracing::warn!(error = %e, path = %target.to_string_lossy(), "read_dir failed");
        debug_log(&format!(
            "read_dir failed: path={} error={:?}",
            target.display(),
            e
        ));
        format!("{}: {e}", target.display())
    })?;
    debug_log(&format!(
        "read_dir success: path={} entries_pending",
        target.display()
    ));

    for entry in read_dir {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                debug_log(&format!("read_dir entry failed: error={:?}", e));
                continue;
            }
        };
        let path = entry.path();
        let key = normalize_key_for_db(&path);
        let starred = star_set.contains(&key);
        let file_type = entry.file_type().ok();
        if is_network_location(&path) {
            if let Some(cached) = get_cached_meta(&path, META_CACHE_TTL) {
                entries.push(crate::commands::fs::entry_from_cached(
                    &path, &cached, starred,
                ));
                continue;
            }
            if pending_seen.insert(path.clone()) {
                pending_meta.push((path.clone(), file_type, starred));
            }
            entries.push(stub_entry(&path, file_type, starred));
            continue;
        }
        let meta = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) => {
                let raw = e.raw_os_error();
                let perm = e.kind() == io::ErrorKind::PermissionDenied
                    || matches!(raw, Some(5) | Some(32));
                let recoverable_net = matches!(
                    raw,
                    Some(59)   // ERROR_UNEXP_NET_ERR
                        | Some(64)   // ERROR_NETNAME_DELETED
                        | Some(67)   // ERROR_BAD_NET_NAME
                        | Some(1219) // ERROR_SESSION_CREDENTIAL_CONFLICT
                        | Some(1231) // ERROR_NETWORK_UNREACHABLE
                        | Some(1232) // ERROR_HOST_UNREACHABLE
                        | Some(22) // Mapped to EINVAL from WinError 1232 in some bindings
                );
                debug_log(&format!(
                    "symlink_metadata failed: path={} error={:?}",
                    path.display(),
                    e
                ));
                if !perm && !recoverable_net {
                    tracing::warn!(error = %e, path = %path.to_string_lossy(), "symlink_metadata failed");
                }
                entries.push(stub_entry(&path, file_type, starred));
                continue;
            }
        };
        let is_link = meta.file_type().is_symlink();
        store_cached_meta(&path, &meta, is_link);
        entries.push(build_entry(&path, &meta, is_link, starred));
    }

    sort_entries(&mut entries, sort);
    spawn_meta_refresh(app, pending_meta);

    Ok(DirListing {
        current: display_path(&target),
        entries,
    })
}

#[tauri::command]
pub async fn list_dir(
    path: Option<String>,
    sort: Option<SortSpec>,
    app: tauri::AppHandle,
) -> Result<DirListing, String> {
    let app_clone = app.clone();
    tauri::async_runtime::spawn_blocking(move || list_dir_sync(path, sort, app_clone))
        .await
        .unwrap_or_else(|e| Err(format!("list_dir task panicked: {e}")))
}

#[tauri::command]
pub async fn list_column_values(
    path: Option<String>,
    column: String,
    include_hidden: Option<bool>,
    app: tauri::AppHandle,
) -> Result<Vec<String>, String> {
    let p = match path {
        Some(p) if !p.is_empty() => std::path::PathBuf::from(p),
        _ => return Ok(Vec::new()),
    };
    if !p.exists() {
        return Ok(Vec::new());
    }

    let app_clone = app.clone();
    let path_arg = Some(p.to_string_lossy().to_string());
    let include_hidden = include_hidden.unwrap_or(false);
    tauri::async_runtime::spawn_blocking(move || {
        let listing = list_dir_sync(path_arg, None, app_clone)?;
        Ok(collect_column_values(
            &listing.entries,
            column.as_str(),
            include_hidden,
        ))
    })
    .await
    .unwrap_or_else(|e| Err(format!("list_column_values task panicked: {e}")))
}

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

#[tauri::command]
pub fn watch_dir(
    path: Option<String>,
    state: tauri::State<WatchState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let base_path = crate::commands::fs::expand_path(path)?;
    let target = match sanitize_path_follow(&base_path.to_string_lossy(), true) {
        Ok(p) if p.exists() => p,
        _ => {
            let home =
                dirs_next::home_dir().ok_or_else(|| "Start directory not found".to_string())?;
            sanitize_path_follow(&home.to_string_lossy(), true)?
        }
    };

    check_no_symlink_components(&target)?;

    if !watch_allow_all() {
        let allowed = watch_allowed_roots();
        let in_allowed = allowed.iter().any(|root| target.starts_with(root));
        if !in_allowed {
            return Err("Watching this path is not allowed".into());
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
