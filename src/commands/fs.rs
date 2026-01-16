//! File-system oriented Tauri commands: listings, mounts, trash, opening, renaming, deleting, and watch wiring.

use crate::{
    db,
    entry::{
        build_entry, get_cached_meta, is_network_location, normalize_key_for_db, store_cached_meta,
        CachedMeta, FsEntry,
    },
    fs_utils::{
        check_no_symlink_components, debug_log, sanitize_path_follow, sanitize_path_nofollow,
    },
    sorting::{sort_entries, SortSpec},
    watcher::{self, WatchState},
};
#[cfg(target_os = "windows")]
#[path = "fs_windows.rs"]
mod fs_windows;
use serde::Serialize;
use std::collections::HashSet;
use std::ffi::OsString;
use std::time::Duration;
use std::{
    fs, io,
    path::{Path, PathBuf},
};
use sysinfo::Disks;
use tauri::Emitter;
use tracing::{error, info, warn};
use trash::{
    delete as trash_delete,
    os_limited::{list as trash_list, metadata as trash_metadata, purge_all, restore_all},
    TrashItem, TrashItemSize,
};

pub fn expand_path(raw: Option<String>) -> Result<PathBuf, String> {
    if let Some(p) = raw {
        if p == "~" {
            dirs_next::home_dir().ok_or_else(|| "Home directory not found".to_string())
        } else if let Some(stripped) = p.strip_prefix("~/") {
            let home =
                dirs_next::home_dir().ok_or_else(|| "Home directory not found".to_string())?;
            Ok(home.join(stripped))
        } else {
            Ok(PathBuf::from(p))
        }
    } else if let Some(home) = dirs_next::home_dir() {
        Ok(home)
    } else {
        std::env::current_dir().map_err(|e| format!("Failed to read working directory: {e}"))
    }
}

#[derive(Serialize)]
pub struct DirListing {
    pub current: String,
    pub entries: Vec<FsEntry>,
}

#[derive(Serialize, Clone)]
pub struct MountInfo {
    pub label: String,
    pub path: String,
    pub fs: String,
    pub removable: bool,
}

const META_CACHE_TTL: Duration = Duration::from_secs(30);

fn entry_from_cached(path: &Path, cached: &CachedMeta, starred: bool) -> FsEntry {
    let kind = if cached.is_link {
        "link"
    } else if cached.is_dir {
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

    FsEntry {
        name,
        path: path.to_string_lossy().into_owned(),
        kind,
        ext,
        size: cached.size,
        items: None,
        modified: cached.modified.clone(),
        original_path: None,
        trash_id: None,
        icon: cached.icon.clone(),
        starred,
        hidden: cached.hidden,
        network: cached.network,
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
    let icon = if is_link {
        "icons/scalable/mimetypes/inode-symlink.svg"
    } else if is_dir {
        "icons/scalable/places/folder.svg"
    } else {
        "icons/scalable/mimetypes/application-x-generic.svg"
    }
    .to_string();

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
        icon,
        starred,
        hidden: path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.starts_with('.'))
            .unwrap_or(false),
        network: is_network_location(path),
    }
}

fn spawn_meta_refresh(app: tauri::AppHandle, jobs: Vec<(PathBuf, Option<fs::FileType>, bool)>) {
    if jobs.is_empty() {
        return;
    }
    tauri::async_runtime::spawn_blocking(move || {
        let mut batch: Vec<FsEntry> = Vec::with_capacity(128);
        for (idx, (path, file_type, starred)) in jobs.into_iter().enumerate() {
            let meta = match fs::symlink_metadata(&path) {
                Ok(m) => m,
                Err(e) => {
                    debug_log(&format!(
                        "background metadata fetch failed: path={} error={:?}",
                        path.display(),
                        e
                    ));
                    continue;
                }
            };
            let is_link = meta.file_type().is_symlink();
            store_cached_meta(&path, &meta, is_link);
            let mut entry = build_entry(&path, &meta, is_link, starred);
            if let Some(ft) = file_type {
                if entry.kind != "link" {
                    let dir_hint = ft.is_dir();
                    if dir_hint && entry.kind != "dir" {
                        entry.kind = "dir".to_string();
                    }
                }
            }
            batch.push(entry);
            if batch.len() >= 128 {
                let _ = app.emit("entry-meta-batch", &batch);
                batch.clear();
            }
            if idx % 512 == 511 {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
        if !batch.is_empty() {
            let _ = app.emit("entry-meta-batch", &batch);
        }
    });
}

#[tauri::command]
pub fn list_dir(
    path: Option<String>,
    sort: Option<SortSpec>,
    app: tauri::AppHandle,
) -> Result<DirListing, String> {
    let base_path = expand_path(path)?;
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
                entries.push(entry_from_cached(&path, &cached, starred));
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

#[cfg(not(target_os = "windows"))]
fn restorable_file_in_trash_from_info_file(info_file: &Path) -> PathBuf {
    let trash_folder = info_file.parent().and_then(|p| p.parent());
    let name_in_trash = info_file.file_stem();
    match (trash_folder, name_in_trash) {
        (Some(folder), Some(name)) => folder.join("files").join(name),
        _ => PathBuf::from(info_file),
    }
}

fn trash_item_path(item: &TrashItem) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        PathBuf::from(&item.id)
    }
    #[cfg(not(target_os = "windows"))]
    {
        restorable_file_in_trash_from_info_file(Path::new(&item.id))
    }
}

#[cfg(target_os = "windows")]
fn display_path(path: &Path) -> String {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        return rest.to_string();
    }
    s.into_owned()
}

#[cfg(not(target_os = "windows"))]

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
    roots
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(target_os = "windows")]
#[tauri::command]
pub fn list_mounts() -> Vec<MountInfo> {
    fs_windows::list_windows_mounts()
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub fn list_mounts() -> Vec<MountInfo> {
    let disks = Disks::new_with_refreshed_list();
    disks
        .iter()
        .filter_map(|d| {
            let mount_point = d.mount_point().to_string_lossy().to_string();
            if mount_point.is_empty() {
                return None;
            }
            let fs = d.file_system().to_string_lossy().to_string();
            let fs_lc = fs.to_lowercase();
            if matches!(
                fs_lc.as_str(),
                "tmpfs"
                    | "devtmpfs"
                    | "proc"
                    | "sysfs"
                    | "cgroup"
                    | "cgroup2"
                    | "overlay"
                    | "squashfs"
            ) {
                return None;
            }

            let label = mount_point.clone();

            Some(MountInfo {
                label,
                path: mount_point,
                fs,
                removable: d.is_removable(),
            })
        })
        .collect()
}

#[tauri::command]
pub fn watch_dir(
    path: Option<String>,
    state: tauri::State<WatchState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let base_path = expand_path(path)?;
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

#[tauri::command]
pub fn open_entry(path: String) -> Result<(), String> {
    let pb = sanitize_path_follow(&path, false)?;
    let conn = db::open()?;
    if let Err(e) = db::touch_recent(&conn, &pb.to_string_lossy()) {
        warn!("Failed to record recent for {:?}: {}", pb, e);
    }
    info!("Opening path {:?}", pb);
    open::that_detached(&pb).map_err(|e| {
        error!("Failed to open {:?}: {}", pb, e);
        format!("Failed to open: {e}")
    })
}

#[tauri::command]
pub fn list_trash(sort: Option<SortSpec>) -> Result<DirListing, String> {
    let items = trash_list().map_err(|e| format!("Failed to list trash: {e}"))?;
    let mut entries = Vec::new();
    for item in items {
        let path = trash_item_path(&item);
        let meta = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) => {
                debug_log(&format!(
                    "trash list: missing item path={}, skipping: {e:?}",
                    path.display()
                ));
                continue;
            }
        };
        let is_link = meta.file_type().is_symlink();
        let mut entry = build_entry(&path, &meta, is_link, false);
        entry.original_path = Some(item.original_path().to_string_lossy().into_owned());
        entry.trash_id = Some(item.id.to_string_lossy().into_owned());
        if let Ok(info) = trash_metadata(&item) {
            match info.size {
                TrashItemSize::Bytes(b) => entry.size = Some(b),
                TrashItemSize::Entries(n) => entry.items = Some(n as u64),
            }
        }
        entries.push(entry);
    }
    sort_entries(&mut entries, sort);
    Ok(DirListing {
        current: "Trash".to_string(),
        entries,
    })
}

#[tauri::command]
pub fn rename_entry(path: String, new_name: String) -> Result<String, String> {
    let from = sanitize_path_nofollow(&path, true)?;
    check_no_symlink_components(&from)?;
    if new_name.trim().is_empty() {
        return Err("New name cannot be empty".into());
    }
    let parent = from
        .parent()
        .ok_or_else(|| "Cannot rename root".to_string())?;
    let to = parent.join(new_name.trim());
    if to.exists() {
        return Err("A file or directory with that name already exists".into());
    }
    fs::rename(&from, &to).map_err(|e| format!("Failed to rename: {e}"))?;
    Ok(to.to_string_lossy().to_string())
}

#[tauri::command]
pub fn move_to_trash(path: String, app: tauri::AppHandle) -> Result<(), String> {
    let src = sanitize_path_nofollow(&path, true)?;
    check_no_symlink_components(&src)?;

    trash_delete(&src)
        .map_err(|e| format!("Failed to move to trash: {e}"))
        .map(|_| {
            let _ = app.emit("trash-changed", ());
        })
}

#[tauri::command]
pub fn restore_trash_items(ids: Vec<String>, app: tauri::AppHandle) -> Result<(), String> {
    let wanted: HashSet<OsString> = ids.into_iter().map(OsString::from).collect();
    if wanted.is_empty() {
        return Ok(());
    }
    let items = trash_list().map_err(|e| format!("Failed to list trash: {e}"))?;
    let selected: Vec<_> = items
        .into_iter()
        .filter(|item| wanted.contains(&item.id))
        .collect();
    if selected.is_empty() {
        return Err("Nothing to restore".into());
    }
    restore_all(selected)
        .map_err(|e| format!("Failed to restore: {e}"))
        .map(|_| {
            let _ = app.emit("trash-changed", ());
        })
}

#[tauri::command]
pub fn purge_trash_items(ids: Vec<String>, app: tauri::AppHandle) -> Result<(), String> {
    let wanted: HashSet<OsString> = ids.into_iter().map(OsString::from).collect();
    if wanted.is_empty() {
        return Ok(());
    }
    let items = trash_list().map_err(|e| format!("Failed to list trash: {e}"))?;
    let selected: Vec<_> = items
        .into_iter()
        .filter(|item| wanted.contains(&item.id))
        .collect();
    if selected.is_empty() {
        return Err("Nothing to delete".into());
    }
    purge_all(selected)
        .map_err(|e| format!("Failed to delete permanently: {e}"))
        .map(|_| {
            let _ = app.emit("trash-changed", ());
        })
}

#[tauri::command]
pub fn delete_entry(path: String) -> Result<(), String> {
    let pb = sanitize_path_nofollow(&path, true)?;
    check_no_symlink_components(&pb)?;
    delete_now(&pb)
}

fn delete_now(pb: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(pb).map_err(|e| format!("Failed to read metadata: {e}"))?;
    if meta.is_dir() {
        fs::remove_dir_all(pb).map_err(|e| format!("Failed to delete directory: {e}"))
    } else {
        fs::remove_file(pb).map_err(|e| format!("Failed to delete file: {e}"))
    }
}
