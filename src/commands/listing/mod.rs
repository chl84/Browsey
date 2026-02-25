//! Directory listing and watcher wiring.

use crate::{
    commands::cloud::types::{CloudEntry as BrowseyCloudEntry, CloudEntryKind},
    db,
    entry::{
        build_entry, get_cached_meta, is_network_location, normalize_key_for_db, store_cached_meta,
        EntryCapabilities, FsEntry,
    },
    errors::api_error::ApiResult,
    fs_utils::{check_no_symlink_components, debug_log, sanitize_path_follow},
    icons::icon_ids::{FILE, GENERIC_FOLDER, SHORTCUT},
    sorting::{sort_entries, SortSpec},
    watcher::{self, WatchState},
};
use chrono::{Local, NaiveDateTime};
use error::{map_api_result, ListingError, ListingErrorCode, ListingResult};
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

mod error;

const META_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(30);

#[derive(Serialize, Clone)]
pub struct FacetOption {
    pub id: String,
    pub label: String,
}

#[derive(Serialize, Default, Clone)]
pub struct ListingFacets {
    pub name: Vec<FacetOption>,
    #[serde(rename = "type")]
    pub type_values: Vec<FacetOption>,
    pub modified: Vec<FacetOption>,
    pub size: Vec<FacetOption>,
}

pub struct ListingFacetBuilder {
    name: HashMap<String, u64>,
    type_values: HashMap<String, u64>,
    modified: HashMap<String, (i64, u64)>,
    size: HashMap<String, (i64, u64)>,
    now: NaiveDateTime,
}

impl Default for ListingFacetBuilder {
    fn default() -> Self {
        Self {
            name: HashMap::new(),
            type_values: HashMap::new(),
            modified: HashMap::new(),
            size: HashMap::new(),
            now: Local::now().naive_local(),
        }
    }
}

impl ListingFacetBuilder {
    pub fn add(&mut self, entry: &FsEntry) {
        let name_key = entry.name.to_lowercase();
        let (name_bucket, _) = bucket_name(name_key.as_str());
        *self.name.entry(name_bucket.to_string()).or_insert(0) += 1;

        let ty = entry_type_label(entry);
        *self.type_values.entry(ty).or_insert(0) += 1;

        if let Some(modified) = &entry.modified {
            if let Ok(dt) = NaiveDateTime::parse_from_str(modified, "%Y-%m-%d %H:%M") {
                let (label, rank) = bucket_modified(dt, self.now);
                let slot = self.modified.entry(label).or_insert((rank, 0));
                slot.1 += 1;
                if rank < slot.0 {
                    slot.0 = rank;
                }
            }
        }

        if entry.kind == "file" {
            if let Some(size) = entry.size {
                let (label, rank) = bucket_size(size);
                let slot = self.size.entry(label).or_insert((rank, 0));
                slot.1 += 1;
                if rank < slot.0 {
                    slot.0 = rank;
                }
            }
        }
    }

    pub fn finish(self) -> ListingFacets {
        let mut name: Vec<FacetOption> = self
            .name
            .into_keys()
            .map(|id| FacetOption {
                label: name_filter_label(id.as_str()).to_string(),
                id,
            })
            .collect();
        name.sort_by_key(|opt| name_filter_rank(opt.id.as_str()));

        let mut type_values: Vec<FacetOption> = self
            .type_values
            .into_keys()
            .map(|label| FacetOption {
                id: format!("type:{label}"),
                label,
            })
            .collect();
        type_values.sort_by(|a, b| a.label.cmp(&b.label));

        let mut modified: Vec<(String, i64)> = self
            .modified
            .into_iter()
            .map(|(label, (rank, _count))| (label, rank))
            .collect();
        modified.sort_by_key(|(_, rank)| *rank);
        let modified: Vec<FacetOption> = modified
            .into_iter()
            .map(|(label, _)| FacetOption {
                id: format!("modified:{label}"),
                label,
            })
            .collect();

        let mut size: Vec<(String, i64)> = self
            .size
            .into_iter()
            .map(|(label, (rank, _count))| (label, rank))
            .collect();
        size.sort_by_key(|(_, rank)| *rank);
        let size: Vec<FacetOption> = size
            .into_iter()
            .map(|(label, _)| FacetOption {
                id: format!("size:{label}"),
                label,
            })
            .collect();

        ListingFacets {
            name,
            type_values,
            modified,
            size,
        }
    }
}

pub fn build_listing_facets_with_hidden(
    entries: &[FsEntry],
    include_hidden: bool,
) -> ListingFacets {
    let mut builder = ListingFacetBuilder::default();
    for entry in entries {
        if !include_hidden && entry.hidden {
            continue;
        }
        builder.add(entry);
    }
    builder.finish()
}

fn name_filter_label(id: &str) -> &'static str {
    match id {
        "name:a-f" => "A–F",
        "name:g-l" => "G–L",
        "name:m-r" => "M–R",
        "name:s-z" => "S–Z",
        "name:0-9" => "0-9",
        "name:other" => "Other symbols",
        _ => "Other symbols",
    }
}

fn name_filter_rank(id: &str) -> i64 {
    match id {
        "name:a-f" => 0,
        "name:g-l" => 1,
        "name:m-r" => 2,
        "name:s-z" => 3,
        "name:0-9" => 4,
        "name:other" => 5,
        _ => i64::MAX,
    }
}

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
    if ch.is_ascii_digit() {
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
        display_path_unix(path)
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
        capabilities: None,
    }
}

fn is_cloud_path_str(path: &str) -> bool {
    path.starts_with("rclone://")
}

fn fs_entry_from_cloud_entry(entry: BrowseyCloudEntry) -> FsEntry {
    let is_dir = matches!(entry.kind, CloudEntryKind::Dir);
    let ext = if is_dir {
        None
    } else {
        entry.name.rsplit_once('.').map(|(_, ext)| ext.to_string())
    };
    FsEntry {
        name: entry.name.clone(),
        path: entry.path,
        kind: if is_dir { "dir" } else { "file" }.to_string(),
        ext,
        size: if is_dir { None } else { entry.size },
        items: None,
        modified: entry.modified,
        original_path: None,
        trash_id: None,
        icon_id: if is_dir { GENERIC_FOLDER } else { FILE },
        starred: false,
        hidden: entry.name.starts_with('.'),
        network: true,
        read_only: false,
        read_denied: false,
        capabilities: Some(EntryCapabilities {
            can_list: entry.capabilities.can_list,
            can_mkdir: entry.capabilities.can_mkdir,
            can_delete: entry.capabilities.can_delete,
            can_rename: entry.capabilities.can_rename,
            can_move: entry.capabilities.can_move,
            can_copy: entry.capabilities.can_copy,
            can_trash: entry.capabilities.can_trash,
            can_undo: entry.capabilities.can_undo,
            can_permissions: entry.capabilities.can_permissions,
        }),
    }
}

fn listing_error_from_api(error: crate::errors::api_error::ApiError) -> ListingError {
    let code = match error.code.as_str() {
        "invalid_path" => ListingErrorCode::InvalidPath,
        "not_found" => ListingErrorCode::NotFound,
        "permission_denied" => ListingErrorCode::PermissionDenied,
        "task_failed" => ListingErrorCode::TaskFailed,
        _ => ListingErrorCode::UnknownError,
    };
    ListingError::new(code, error.message)
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

    let star_conn = db::open().map_err(|error| error.to_string())?;
    let star_set: HashSet<String> =
        db::starred_set(&star_conn).map_err(|error| error.to_string())?;

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
) -> ApiResult<DirListing> {
    map_api_result(list_dir_impl(path, sort, app).await)
}

async fn list_dir_impl(
    path: Option<String>,
    sort: Option<SortSpec>,
    app: tauri::AppHandle,
) -> ListingResult<DirListing> {
    if let Some(raw_path) = path.as_deref() {
        if is_cloud_path_str(raw_path) {
            let entries = crate::commands::cloud::list_cloud_entries(raw_path.to_string())
                .await
                .map_err(listing_error_from_api)?;
            let mut mapped: Vec<FsEntry> =
                entries.into_iter().map(fs_entry_from_cloud_entry).collect();
            sort_entries(&mut mapped, sort);
            return Ok(DirListing {
                current: raw_path.to_string(),
                entries: mapped,
            });
        }
    }
    let task = tauri::async_runtime::spawn_blocking(move || list_dir_sync(path, sort, app));
    match task.await {
        Ok(result) => result.map_err(ListingError::from_external_message),
        Err(error) => Err(ListingError::new(
            ListingErrorCode::TaskFailed,
            format!("list_dir task panicked: {error}"),
        )),
    }
}

#[tauri::command]
pub async fn list_facets(
    scope: String,
    path: Option<String>,
    include_hidden: Option<bool>,
    app: tauri::AppHandle,
) -> ApiResult<ListingFacets> {
    map_api_result(list_facets_impl(scope, path, include_hidden, app).await)
}

async fn list_facets_impl(
    scope: String,
    path: Option<String>,
    include_hidden: Option<bool>,
    app: tauri::AppHandle,
) -> ListingResult<ListingFacets> {
    let include_hidden = include_hidden.unwrap_or(true);
    if scope == "dir" {
        if let Some(raw_path) = path.as_deref() {
            if is_cloud_path_str(raw_path) {
                let entries = list_dir_impl(path, None, app).await?.entries;
                return Ok(build_listing_facets_with_hidden(&entries, include_hidden));
            }
        }
    }
    let task = tauri::async_runtime::spawn_blocking(move || {
        let entries = match scope.as_str() {
            "dir" => list_dir_sync(path, None, app.clone())?.entries,
            "recent" => {
                crate::commands::library::list_recent(None)
                    .map_err(|error| error.message)?
                    .entries
            }
            "starred" => {
                crate::commands::library::list_starred(None)
                    .map_err(|error| error.message)?
                    .entries
            }
            "trash" => {
                crate::commands::fs::list_trash(None)
                    .map_err(|error| error.message)?
                    .entries
            }
            _ => return Err(format!("Unsupported facet scope: {scope}")),
        };
        Ok(build_listing_facets_with_hidden(&entries, include_hidden))
    });
    match task.await {
        Ok(result) => result.map_err(ListingError::from_external_message),
        Err(error) => Err(ListingError::new(
            ListingErrorCode::TaskFailed,
            format!("list_facets task panicked: {error}"),
        )),
    }
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
) -> ApiResult<()> {
    map_api_result(watch_dir_impl(path, state, app))
}

fn watch_dir_impl(
    path: Option<String>,
    state: tauri::State<WatchState>,
    app: tauri::AppHandle,
) -> ListingResult<()> {
    if let Some(raw_path) = path.as_deref() {
        if is_cloud_path_str(raw_path) {
            state.replace(None);
            return Ok(());
        }
    }
    let base_path =
        crate::commands::fs::expand_path(path).map_err(ListingError::from_external_message)?;
    let target = match sanitize_path_follow(&base_path.to_string_lossy(), true) {
        Ok(p) if p.exists() => p,
        _ => {
            let home = dirs_next::home_dir().ok_or_else(|| {
                ListingError::new(ListingErrorCode::InvalidInput, "Start directory not found")
            })?;
            sanitize_path_follow(&home.to_string_lossy(), true)
                .map_err(ListingError::from_external_message)?
        }
    };

    check_no_symlink_components(&target).map_err(ListingError::from_external_message)?;

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
    use super::{bucket_modified, bucket_name, bucket_size};
    use chrono::NaiveDateTime;

    fn parse_dt(value: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M").expect("valid test datetime")
    }

    #[test]
    fn bucket_modified_uses_naive_day_boundaries() {
        let now = parse_dt("2026-03-10 12:00");

        assert_eq!(
            bucket_modified(parse_dt("2026-03-10 11:01"), now).0,
            "Today"
        );
        assert_eq!(
            bucket_modified(parse_dt("2026-03-09 13:00"), now).0,
            "Today"
        );
        assert_eq!(
            bucket_modified(parse_dt("2026-03-09 12:00"), now).0,
            "Yesterday"
        );
        assert_eq!(
            bucket_modified(parse_dt("2026-03-08 12:00"), now).0,
            "2 days ago"
        );
        assert_eq!(
            bucket_modified(parse_dt("2026-03-03 12:00"), now).0,
            "1 week ago"
        );
        assert_eq!(
            bucket_modified(parse_dt("2026-01-01 12:00"), now).0,
            "3 months ago"
        );
        assert_eq!(
            bucket_modified(parse_dt("2025-03-10 12:00"), now).0,
            "1 year ago"
        );
    }

    #[test]
    fn bucket_size_assigns_expected_ranges() {
        const KB: u64 = 1024;
        const MB: u64 = 1024 * KB;
        const GB: u64 = 1024 * MB;
        const TB: u64 = 1024 * GB;

        assert_eq!(bucket_size(0).0, "0–10 KB");
        assert_eq!(bucket_size(10 * KB).0, "0–10 KB");
        assert_eq!(bucket_size(10 * KB + 1).0, "10–100 KB");
        assert_eq!(bucket_size(MB).0, "100 KB–1 MB");
        assert_eq!(bucket_size(GB).0, "100 MB–1 GB");
        assert_eq!(bucket_size(TB).0, "100 GB–1 TB");
        assert_eq!(bucket_size(2 * TB).0, "Over 1 TB");
        assert_eq!(bucket_size(2 * TB).1, (2 * TB) as i64);
    }

    #[test]
    fn bucket_name_assigns_expected_ranges() {
        assert_eq!(bucket_name("apple").0, "name:a-f");
        assert_eq!(bucket_name("kite").0, "name:g-l");
        assert_eq!(bucket_name("moon").0, "name:m-r");
        assert_eq!(bucket_name("zeta").0, "name:s-z");
        assert_eq!(bucket_name("7zip").0, "name:0-9");
        assert_eq!(bucket_name("_tmp").0, "name:other");
    }
}
