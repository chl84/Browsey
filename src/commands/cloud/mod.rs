//! Cloud-provider Tauri commands (rclone-backed, CLI-first).

mod error;
pub mod path;
pub mod provider;
pub mod providers;
pub mod rclone_cli;
pub mod types;

use crate::errors::api_error::ApiResult;
use error::{map_api_result, CloudCommandError, CloudCommandErrorCode, CloudCommandResult};
use path::CloudPath;
use provider::CloudProvider;
use providers::rclone::RcloneCloudProvider;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tracing::{debug, warn};
use types::{CloudConflictInfo, CloudEntry, CloudEntryKind, CloudRemote, CloudRootSelection};

const CLOUD_REMOTE_DISCOVERY_CACHE_TTL: Duration = Duration::from_secs(45);
const CLOUD_DIR_LISTING_CACHE_TTL: Duration = Duration::from_secs(4);
const CLOUD_DIR_LISTING_RETRY_BACKOFFS_MS: &[u64] = &[150, 400];

#[derive(Debug, Clone)]
struct CachedCloudRemoteDiscovery {
    fetched_at: Instant,
    remotes: Vec<CloudRemote>,
}

#[derive(Debug, Clone)]
struct CachedCloudDirListing {
    fetched_at: Instant,
    entries: Vec<CloudEntry>,
}

fn cloud_remote_discovery_cache() -> &'static Mutex<Option<CachedCloudRemoteDiscovery>> {
    static CACHE: OnceLock<Mutex<Option<CachedCloudRemoteDiscovery>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

fn cloud_dir_listing_cache() -> &'static Mutex<HashMap<String, CachedCloudDirListing>> {
    static CACHE: OnceLock<Mutex<HashMap<String, CachedCloudDirListing>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn list_cloud_remotes_cached(force_refresh: bool) -> CloudCommandResult<Vec<CloudRemote>> {
    let now = Instant::now();
    if !force_refresh {
        if let Ok(guard) = cloud_remote_discovery_cache().lock() {
            if let Some(cached) = guard.as_ref() {
                if now.duration_since(cached.fetched_at) <= CLOUD_REMOTE_DISCOVERY_CACHE_TTL {
                    return Ok(cached.remotes.clone());
                }
            }
        }
    }

    let provider = RcloneCloudProvider::default();
    let remotes = provider.list_remotes()?;
    if let Ok(mut guard) = cloud_remote_discovery_cache().lock() {
        *guard = Some(CachedCloudRemoteDiscovery {
            fetched_at: now,
            remotes: remotes.clone(),
        });
    }
    Ok(remotes)
}

fn list_cloud_dir_cached(path: &CloudPath) -> CloudCommandResult<Vec<CloudEntry>> {
    let now = Instant::now();
    let key = path.to_string();
    if let Ok(mut guard) = cloud_dir_listing_cache().lock() {
        guard.retain(|_, cached| {
            now.duration_since(cached.fetched_at) <= CLOUD_DIR_LISTING_CACHE_TTL
        });
        if let Some(cached) = guard.get(&key) {
            return Ok(cached.entries.clone());
        }
    }

    let entries = list_cloud_dir_with_retry(path)?;
    if let Ok(mut guard) = cloud_dir_listing_cache().lock() {
        guard.insert(
            key,
            CachedCloudDirListing {
                fetched_at: now,
                entries: entries.clone(),
            },
        );
    }
    Ok(entries)
}

fn list_cloud_dir_with_retry(path: &CloudPath) -> CloudCommandResult<Vec<CloudEntry>> {
    let provider = RcloneCloudProvider::default();
    let mut attempt = 0usize;
    loop {
        match provider.list_dir(path) {
            Ok(entries) => return Ok(entries),
            Err(error) if should_retry_cloud_dir_error(&error) => {
                let Some(backoff_ms) = CLOUD_DIR_LISTING_RETRY_BACKOFFS_MS.get(attempt).copied()
                else {
                    return Err(error);
                };
                attempt += 1;
                debug!(
                    attempt,
                    backoff_ms,
                    path = %path,
                    error = %error,
                    "retrying cloud directory listing after transient error"
                );
                std::thread::sleep(Duration::from_millis(backoff_ms));
            }
            Err(error) => return Err(error),
        }
    }
}

fn should_retry_cloud_dir_error(error: &CloudCommandError) -> bool {
    matches!(
        error.code(),
        CloudCommandErrorCode::Timeout
            | CloudCommandErrorCode::NetworkError
            | CloudCommandErrorCode::RateLimited
    )
}

fn invalidate_cloud_dir_listing_cache_path(path: &CloudPath) {
    if let Ok(mut guard) = cloud_dir_listing_cache().lock() {
        guard.remove(&path.to_string());
    }
}

fn invalidate_cloud_dir_listing_cache_for_write_paths(paths: &[CloudPath]) {
    for path in paths {
        invalidate_cloud_dir_listing_cache_path(path);
        if let Some(parent) = path.parent_dir_path() {
            invalidate_cloud_dir_listing_cache_path(&parent);
        }
    }
}

pub(crate) fn list_cloud_remotes_sync_best_effort(force_refresh: bool) -> Vec<CloudRemote> {
    match list_cloud_remotes_cached(force_refresh) {
        Ok(remotes) => remotes,
        Err(error) => {
            warn!(error = %error, "cloud remote discovery failed; omitting cloud remotes from Network view");
            Vec::new()
        }
    }
}

#[tauri::command]
pub async fn list_cloud_remotes() -> ApiResult<Vec<CloudRemote>> {
    map_api_result(list_cloud_remotes_impl().await)
}

async fn list_cloud_remotes_impl() -> CloudCommandResult<Vec<CloudRemote>> {
    let task = tauri::async_runtime::spawn_blocking(|| list_cloud_remotes_cached(false));
    match task.await {
        Ok(result) => result,
        Err(error) => Err(CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            format!("cloud remote list task failed: {error}"),
        )),
    }
}

#[tauri::command]
pub async fn validate_cloud_root(path: String) -> ApiResult<CloudRootSelection> {
    map_api_result(validate_cloud_root_impl(path).await)
}

async fn validate_cloud_root_impl(path: String) -> CloudCommandResult<CloudRootSelection> {
    let path = parse_cloud_path_arg(path)?;
    let task = tauri::async_runtime::spawn_blocking(move || {
        let provider = RcloneCloudProvider::default();
        let remotes = provider.list_remotes()?;
        let remote = remotes
            .into_iter()
            .find(|remote| remote.id == path.remote())
            .ok_or_else(|| {
                CloudCommandError::new(
                    CloudCommandErrorCode::InvalidConfig,
                    format!(
                        "Cloud remote is not configured or unsupported: {}",
                        path.remote()
                    ),
                )
            })?;

        if !path.is_root() {
            let stat = provider.stat_path(&path)?.ok_or_else(|| {
                CloudCommandError::new(
                    CloudCommandErrorCode::NotFound,
                    format!("Cloud root path does not exist: {path}"),
                )
            })?;
            if !matches!(stat.kind, CloudEntryKind::Dir) {
                return Err(CloudCommandError::new(
                    CloudCommandErrorCode::InvalidPath,
                    format!("Cloud root path must be a directory: {path}"),
                ));
            }
        }

        Ok(CloudRootSelection {
            remote,
            root_path: path.to_string(),
            is_remote_root: path.is_root(),
        })
    });
    map_spawn_result(task.await, "cloud root validation task failed")
}

#[tauri::command]
pub async fn list_cloud_entries(path: String) -> ApiResult<Vec<CloudEntry>> {
    map_api_result(list_cloud_entries_impl(path).await)
}

async fn list_cloud_entries_impl(path: String) -> CloudCommandResult<Vec<CloudEntry>> {
    let path = parse_cloud_path_arg(path)?;
    let task = tauri::async_runtime::spawn_blocking(move || list_cloud_dir_cached(&path));
    match task.await {
        Ok(result) => result,
        Err(error) => Err(CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            format!("cloud list task failed: {error}"),
        )),
    }
}

#[tauri::command]
pub async fn stat_cloud_entry(path: String) -> ApiResult<Option<CloudEntry>> {
    map_api_result(stat_cloud_entry_impl(path).await)
}

async fn stat_cloud_entry_impl(path: String) -> CloudCommandResult<Option<CloudEntry>> {
    let path = CloudPath::parse(&path).map_err(|error| {
        CloudCommandError::new(
            CloudCommandErrorCode::InvalidPath,
            format!("Invalid cloud path: {error}"),
        )
    })?;
    let task = tauri::async_runtime::spawn_blocking(move || {
        let provider = RcloneCloudProvider::default();
        provider.stat_path(&path)
    });
    match task.await {
        Ok(result) => result,
        Err(error) => Err(CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            format!("cloud stat task failed: {error}"),
        )),
    }
}

#[tauri::command]
pub async fn create_cloud_folder(path: String) -> ApiResult<()> {
    map_api_result(create_cloud_folder_impl(path).await)
}

async fn create_cloud_folder_impl(path: String) -> CloudCommandResult<()> {
    let path = CloudPath::parse(&path).map_err(|error| {
        CloudCommandError::new(
            CloudCommandErrorCode::InvalidPath,
            format!("Invalid cloud path: {error}"),
        )
    })?;
    let path_for_invalidate = path.clone();
    let task = tauri::async_runtime::spawn_blocking(move || {
        let provider = RcloneCloudProvider::default();
        provider.mkdir(&path)
    });
    match task.await {
        Ok(result) => {
            result?;
            invalidate_cloud_dir_listing_cache_for_write_paths(&[path_for_invalidate]);
            Ok(())
        }
        Err(error) => Err(CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            format!("cloud mkdir task failed: {error}"),
        )),
    }
}

#[tauri::command]
pub async fn delete_cloud_file(path: String) -> ApiResult<()> {
    map_api_result(delete_cloud_file_impl(path).await)
}

async fn delete_cloud_file_impl(path: String) -> CloudCommandResult<()> {
    let path = parse_cloud_path_arg(path)?;
    let path_for_invalidate = path.clone();
    let task = tauri::async_runtime::spawn_blocking(move || {
        let provider = RcloneCloudProvider::default();
        provider.delete_file(&path)
    });
    map_spawn_result(task.await, "cloud delete file task failed")?;
    invalidate_cloud_dir_listing_cache_for_write_paths(&[path_for_invalidate]);
    Ok(())
}

#[tauri::command]
pub async fn delete_cloud_dir_recursive(path: String) -> ApiResult<()> {
    map_api_result(delete_cloud_dir_recursive_impl(path).await)
}

async fn delete_cloud_dir_recursive_impl(path: String) -> CloudCommandResult<()> {
    let path = parse_cloud_path_arg(path)?;
    let path_for_invalidate = path.clone();
    let task = tauri::async_runtime::spawn_blocking(move || {
        let provider = RcloneCloudProvider::default();
        provider.delete_dir_recursive(&path)
    });
    map_spawn_result(task.await, "cloud delete dir task failed")?;
    invalidate_cloud_dir_listing_cache_for_write_paths(&[path_for_invalidate]);
    Ok(())
}

#[tauri::command]
pub async fn delete_cloud_dir_empty(path: String) -> ApiResult<()> {
    map_api_result(delete_cloud_dir_empty_impl(path).await)
}

async fn delete_cloud_dir_empty_impl(path: String) -> CloudCommandResult<()> {
    let path = parse_cloud_path_arg(path)?;
    let path_for_invalidate = path.clone();
    let task = tauri::async_runtime::spawn_blocking(move || {
        let provider = RcloneCloudProvider::default();
        provider.delete_dir_empty(&path)
    });
    map_spawn_result(task.await, "cloud rmdir task failed")?;
    invalidate_cloud_dir_listing_cache_for_write_paths(&[path_for_invalidate]);
    Ok(())
}

#[tauri::command]
pub async fn move_cloud_entry(
    src: String,
    dst: String,
    overwrite: Option<bool>,
    prechecked: Option<bool>,
) -> ApiResult<()> {
    map_api_result(
        move_cloud_entry_impl(
            src,
            dst,
            overwrite.unwrap_or(false),
            prechecked.unwrap_or(false),
        )
        .await,
    )
}

async fn move_cloud_entry_impl(
    src: String,
    dst: String,
    overwrite: bool,
    prechecked: bool,
) -> CloudCommandResult<()> {
    let src = parse_cloud_path_arg(src)?;
    let dst = parse_cloud_path_arg(dst)?;
    let invalidate_paths = vec![src.clone(), dst.clone()];
    let task = tauri::async_runtime::spawn_blocking(move || {
        let provider = RcloneCloudProvider::default();
        provider.move_entry(&src, &dst, overwrite, prechecked)
    });
    map_spawn_result(task.await, "cloud move task failed")?;
    invalidate_cloud_dir_listing_cache_for_write_paths(&invalidate_paths);
    Ok(())
}

#[tauri::command]
pub async fn rename_cloud_entry(
    src: String,
    dst: String,
    overwrite: Option<bool>,
    prechecked: Option<bool>,
) -> ApiResult<()> {
    map_api_result(
        move_cloud_entry_impl(
            src,
            dst,
            overwrite.unwrap_or(false),
            prechecked.unwrap_or(false),
        )
        .await,
    )
}

#[tauri::command]
pub async fn copy_cloud_entry(
    src: String,
    dst: String,
    overwrite: Option<bool>,
    prechecked: Option<bool>,
) -> ApiResult<()> {
    map_api_result(
        copy_cloud_entry_impl(
            src,
            dst,
            overwrite.unwrap_or(false),
            prechecked.unwrap_or(false),
        )
        .await,
    )
}

async fn copy_cloud_entry_impl(
    src: String,
    dst: String,
    overwrite: bool,
    prechecked: bool,
) -> CloudCommandResult<()> {
    let src = parse_cloud_path_arg(src)?;
    let dst = parse_cloud_path_arg(dst)?;
    let invalidate_paths = vec![dst.clone()];
    let task = tauri::async_runtime::spawn_blocking(move || {
        let provider = RcloneCloudProvider::default();
        provider.copy_entry(&src, &dst, overwrite, prechecked)
    });
    map_spawn_result(task.await, "cloud copy task failed")?;
    invalidate_cloud_dir_listing_cache_for_write_paths(&invalidate_paths);
    Ok(())
}

#[tauri::command]
pub async fn preview_cloud_conflicts(
    sources: Vec<String>,
    dest_dir: String,
) -> ApiResult<Vec<CloudConflictInfo>> {
    map_api_result(preview_cloud_conflicts_impl(sources, dest_dir).await)
}

async fn preview_cloud_conflicts_impl(
    sources: Vec<String>,
    dest_dir: String,
) -> CloudCommandResult<Vec<CloudConflictInfo>> {
    let dest_dir = parse_cloud_path_arg(dest_dir)?;
    let sources = sources
        .into_iter()
        .map(parse_cloud_path_arg)
        .collect::<CloudCommandResult<Vec<_>>>()?;
    let task = tauri::async_runtime::spawn_blocking(move || {
        let provider = RcloneCloudProvider::default();
        let dest_entries = provider.list_dir(&dest_dir)?;
        build_conflicts_from_dest_listing(&sources, &dest_dir, &dest_entries)
    });
    map_spawn_result(task.await, "cloud conflict preview task failed")
}

fn build_conflicts_from_dest_listing(
    sources: &[CloudPath],
    dest_dir: &CloudPath,
    dest_entries: &[CloudEntry],
) -> CloudCommandResult<Vec<CloudConflictInfo>> {
    let mut name_to_is_dir: HashMap<&str, bool> = HashMap::with_capacity(dest_entries.len());
    for entry in dest_entries {
        name_to_is_dir
            .entry(entry.name.as_str())
            .or_insert(matches!(entry.kind, CloudEntryKind::Dir));
    }

    let mut conflicts = Vec::new();
    for src in sources {
        let name = src.leaf_name().map_err(|error| {
            CloudCommandError::new(
                CloudCommandErrorCode::InvalidPath,
                format!("Invalid source cloud path for conflict preview: {error}"),
            )
        })?;
        let Some(is_dir) = name_to_is_dir.get(name) else {
            continue;
        };
        let target = dest_dir.child_path(name).map_err(|error| {
            CloudCommandError::new(
                CloudCommandErrorCode::InvalidPath,
                format!("Invalid target cloud path for conflict preview: {error}"),
            )
        })?;
        conflicts.push(CloudConflictInfo {
            src: src.to_string(),
            target: target.to_string(),
            exists: true,
            is_dir: *is_dir,
        });
    }
    Ok(conflicts)
}

#[tauri::command]
pub fn normalize_cloud_path(path: String) -> ApiResult<String> {
    map_api_result(normalize_cloud_path_impl(path))
}

fn normalize_cloud_path_impl(path: String) -> CloudCommandResult<String> {
    let path = parse_cloud_path_arg(path)?;
    Ok(path.to_string())
}

fn parse_cloud_path_arg(path: String) -> CloudCommandResult<CloudPath> {
    CloudPath::parse(&path).map_err(|error| {
        CloudCommandError::new(
            CloudCommandErrorCode::InvalidPath,
            format!("Invalid cloud path: {error}"),
        )
    })
}

fn map_spawn_result<T>(
    result: Result<CloudCommandResult<T>, tauri::Error>,
    context: &str,
) -> CloudCommandResult<T> {
    match result {
        Ok(inner) => inner,
        Err(error) => Err(CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            format!("{context}: {error}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_conflicts_from_dest_listing, normalize_cloud_path, normalize_cloud_path_impl,
    };
    use crate::commands::cloud::{
        path::CloudPath,
        types::{CloudCapabilities, CloudEntry, CloudEntryKind},
    };

    #[test]
    fn normalize_cloud_path_command_returns_normalized_path() {
        let out = normalize_cloud_path("rclone://work/docs/file.txt".to_string())
            .expect("normalize_cloud_path should succeed");
        assert_eq!(out, "rclone://work/docs/file.txt");
    }

    #[test]
    fn normalize_cloud_path_command_maps_invalid_path_to_api_error() {
        let err = normalize_cloud_path("rclone://work/../docs".to_string())
            .expect_err("normalize_cloud_path should fail for relative segments");
        assert_eq!(err.code, "invalid_path");
        assert!(
            err.message.contains("Invalid cloud path"),
            "unexpected message: {}",
            err.message
        );
    }

    #[test]
    fn normalize_cloud_path_impl_rejects_non_rclone_paths() {
        let err =
            normalize_cloud_path_impl("/tmp/file.txt".to_string()).expect_err("should reject");
        assert_eq!(
            err.to_string(),
            "Invalid cloud path: Path must start with rclone://"
        );
    }

    #[test]
    fn conflict_preview_uses_dest_listing_names() {
        let src_file = CloudPath::parse("rclone://work/src/report.txt").expect("src file");
        let src_dir = CloudPath::parse("rclone://work/src/Folder").expect("src dir");
        let src_missing = CloudPath::parse("rclone://work/src/notes.txt").expect("src missing");
        let dest_dir = CloudPath::parse("rclone://work/dest").expect("dest");
        let dest_entries = vec![
            CloudEntry {
                name: "report.txt".to_string(),
                path: "rclone://work/dest/report.txt".to_string(),
                kind: CloudEntryKind::File,
                size: Some(1),
                modified: None,
                capabilities: CloudCapabilities::v1_core_rw(),
            },
            CloudEntry {
                name: "Folder".to_string(),
                path: "rclone://work/dest/Folder".to_string(),
                kind: CloudEntryKind::Dir,
                size: None,
                modified: None,
                capabilities: CloudCapabilities::v1_core_rw(),
            },
        ];

        let conflicts = build_conflicts_from_dest_listing(
            &[src_file.clone(), src_dir.clone(), src_missing],
            &dest_dir,
            &dest_entries,
        )
        .expect("conflicts");

        assert_eq!(conflicts.len(), 2);
        assert_eq!(conflicts[0].src, src_file.to_string());
        assert_eq!(conflicts[0].target, "rclone://work/dest/report.txt");
        assert!(!conflicts[0].is_dir);
        assert_eq!(conflicts[1].src, src_dir.to_string());
        assert_eq!(conflicts[1].target, "rclone://work/dest/Folder");
        assert!(conflicts[1].is_dir);
    }
}
