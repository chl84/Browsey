use super::{
    configured_rclone_provider,
    error::{CloudCommandError, CloudCommandErrorCode, CloudCommandResult},
    limits::with_cloud_remote_permits,
    map_spawn_result, parse_cloud_path_arg,
    provider::CloudProvider,
    providers::rclone::RcloneCloudProvider,
    register_cloud_cancel,
    types::CloudEntryKind,
    CloudMaterializeSnapshot,
};
use crate::commands::fs::open_path_without_recent;
use crate::runtime_lifecycle;
use crate::tasks::CancelState;
use blake3::Hasher;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};
use tracing::debug;

const CLOUD_OPEN_CACHE_DIRNAME: &str = "cloud-open";
const CLOUD_OPEN_PART_SUFFIX: &str = ".part";
#[cfg(not(test))]
const CLOUD_OPEN_CACHE_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);
#[cfg(test)]
const CLOUD_OPEN_CACHE_MAX_AGE: Duration = Duration::from_secs(60);
#[cfg(not(test))]
const MATERIALIZE_WAIT_TIMEOUT: Duration = Duration::from_secs(90);
#[cfg(test)]
const MATERIALIZE_WAIT_TIMEOUT: Duration = Duration::from_millis(50);

type MaterializeWaiters = Vec<mpsc::Sender<CloudCommandResult<PathBuf>>>;
static MATERIALIZE_INFLIGHT: once_cell::sync::Lazy<Mutex<HashMap<String, MaterializeWaiters>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct CloudOpenCacheMetadata {
    source_path: String,
    size: Option<u64>,
    modified: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CloudOpenProgressPayload {
    bytes: u64,
    total: u64,
    finished: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CloudOpenCacheClearResult {
    pub removed_files: u64,
    pub removed_bytes: u64,
}

struct CloudOpenDownloadContext<'a> {
    cache_path: &'a Path,
    metadata_path: &'a Path,
    metadata: &'a CloudOpenCacheMetadata,
    app: &'a tauri::AppHandle,
    progress_event: Option<&'a str>,
    cancel: Option<&'a AtomicBool>,
}

struct CloudMaterializeContext<'a> {
    provider: &'a RcloneCloudProvider,
    path: &'a super::path::CloudPath,
    original_name: &'a str,
    size: Option<u64>,
    modified: Option<&'a str>,
    app: &'a tauri::AppHandle,
    progress_event: Option<&'a str>,
    cancel: Option<&'a AtomicBool>,
}

pub(super) async fn open_cloud_entry_impl(
    path: String,
    app: tauri::AppHandle,
    cancel_state: CancelState,
    progress_event: Option<String>,
) -> CloudCommandResult<()> {
    let started = Instant::now();
    let path = parse_cloud_path_arg(path)?;
    let path_for_log = path.clone();
    let remote = path.remote().to_string();
    let cancel_guard = register_cloud_cancel(&cancel_state, &progress_event)?;
    let cancel_token = cancel_guard.as_ref().map(|guard| guard.token());
    let task = tauri::async_runtime::spawn_blocking(move || {
        with_cloud_remote_permits(vec![remote], || {
            materialize_and_open_cloud_file(
                &path,
                &app,
                progress_event.as_deref(),
                cancel_token.as_deref(),
            )
        })
    });
    let result = map_spawn_result(task.await, "cloud open task failed");
    let elapsed_ms = started.elapsed().as_millis() as u64;
    match &result {
        Ok(()) => debug!(
            op = "cloud_open_file",
            path = %path_for_log,
            elapsed_ms,
            "cloud command timing"
        ),
        Err(error) => debug!(
            op = "cloud_open_file",
            path = %path_for_log,
            elapsed_ms,
            error = %error,
            "cloud command failed"
        ),
    }
    result
}

pub(super) fn clear_cloud_open_cache_impl() -> CloudCommandResult<CloudOpenCacheClearResult> {
    let dir = cloud_open_cache_root_path();
    if !dir.exists() {
        prepare_cloud_open_cache_dir(&dir)?;
        return Ok(CloudOpenCacheClearResult {
            removed_files: 0,
            removed_bytes: 0,
        });
    }

    let (removed_files, removed_bytes) = cloud_open_cache_stats(&dir)?;
    fs::remove_dir_all(&dir).map_err(|error| {
        CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            format!("Failed to clear cloud-open cache: {error}"),
        )
    })?;
    prepare_cloud_open_cache_dir(&dir)?;

    Ok(CloudOpenCacheClearResult {
        removed_files,
        removed_bytes,
    })
}

fn materialize_and_open_cloud_file(
    path: &super::path::CloudPath,
    app: &tauri::AppHandle,
    progress_event: Option<&str>,
    cancel: Option<&AtomicBool>,
) -> CloudCommandResult<()> {
    let cache_path = materialize_cloud_file_for_local_use(path, app, progress_event, cancel)?;
    open_path_without_recent(&cache_path).map_err(|error| {
        CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            format!("Failed to open downloaded cloud file: {error}"),
        )
    })
}

pub(crate) fn materialize_cloud_file_for_local_use(
    path: &super::path::CloudPath,
    app: &tauri::AppHandle,
    progress_event: Option<&str>,
    cancel: Option<&AtomicBool>,
) -> CloudCommandResult<PathBuf> {
    let provider = configured_rclone_provider().map_err(|error| {
        CloudCommandError::new(CloudCommandErrorCode::InvalidConfig, error.to_string())
    })?;
    let snapshot = resolve_cloud_materialize_snapshot(&provider, path)?;
    materialize_cloud_file_for_local_use_with_provider_and_snapshot(
        &provider,
        path,
        &snapshot,
        app,
        progress_event,
        cancel,
    )
}

pub(crate) fn materialize_cloud_file_for_local_use_with_snapshot(
    path: &super::path::CloudPath,
    snapshot: &CloudMaterializeSnapshot,
    app: &tauri::AppHandle,
    progress_event: Option<&str>,
    cancel: Option<&AtomicBool>,
) -> CloudCommandResult<PathBuf> {
    let provider = configured_rclone_provider().map_err(|error| {
        CloudCommandError::new(CloudCommandErrorCode::InvalidConfig, error.to_string())
    })?;
    materialize_cloud_file_for_local_use_with_provider_and_snapshot(
        &provider,
        path,
        snapshot,
        app,
        progress_event,
        cancel,
    )
}

fn materialize_cloud_file_for_local_use_with_provider_and_snapshot(
    provider: &RcloneCloudProvider,
    path: &super::path::CloudPath,
    snapshot: &CloudMaterializeSnapshot,
    app: &tauri::AppHandle,
    progress_event: Option<&str>,
    cancel: Option<&AtomicBool>,
) -> CloudCommandResult<PathBuf> {
    if !matches!(snapshot.kind, CloudEntryKind::File) {
        return Err(CloudCommandError::new(
            CloudCommandErrorCode::Unsupported,
            format!("Only cloud files can be opened directly: {path}"),
        ));
    }
    materialize_with_inflight_dedupe(path, snapshot, || {
        materialize_cloud_file_for_local_use_inner(CloudMaterializeContext {
            provider,
            path,
            original_name: &snapshot.name,
            size: snapshot.size,
            modified: snapshot.modified.as_deref(),
            app,
            progress_event,
            cancel,
        })
    })
}

fn resolve_cloud_materialize_snapshot(
    provider: &RcloneCloudProvider,
    path: &super::path::CloudPath,
) -> CloudCommandResult<CloudMaterializeSnapshot> {
    let entry = provider.stat_path(path)?.ok_or_else(|| {
        CloudCommandError::new(
            CloudCommandErrorCode::NotFound,
            format!("Cloud file was not found: {path}"),
        )
    })?;
    Ok(CloudMaterializeSnapshot {
        name: entry.name,
        size: entry.size,
        modified: entry.modified,
        kind: entry.kind,
    })
}

fn materialize_with_inflight_dedupe<F>(
    path: &super::path::CloudPath,
    snapshot: &CloudMaterializeSnapshot,
    do_materialize: F,
) -> CloudCommandResult<PathBuf>
where
    F: FnOnce() -> CloudCommandResult<PathBuf>,
{
    let key = materialize_inflight_key(path, snapshot.size, snapshot.modified.as_deref());
    if let Some(rx) = register_materialize_waiter(&key) {
        return wait_for_materialize_result(path, &key, rx);
    }
    let result = do_materialize();
    notify_materialize_waiters(&key, result.clone());
    result
}

fn materialize_cloud_file_for_local_use_inner(
    ctx: CloudMaterializeContext<'_>,
) -> CloudCommandResult<PathBuf> {
    let CloudMaterializeContext {
        provider,
        path,
        original_name,
        size,
        modified,
        app,
        progress_event,
        cancel,
    } = ctx;
    let cache_path = cloud_open_cache_path(path, original_name)?;
    let metadata_path = cloud_open_metadata_path(&cache_path);
    let expected_meta = CloudOpenCacheMetadata {
        source_path: path.to_string(),
        size,
        modified: modified.map(str::to_string),
    };

    if cache_is_fresh(&cache_path, &metadata_path, &expected_meta) {
        set_owner_only_permissions(&cache_path, false)?;
        set_owner_only_permissions(&metadata_path, false)?;
        emit_cloud_open_progress(
            app,
            progress_event,
            size.unwrap_or(1),
            size.unwrap_or(1),
            true,
        );
    } else {
        download_cloud_file_to_cache(
            provider,
            path,
            CloudOpenDownloadContext {
                cache_path: &cache_path,
                metadata_path: &metadata_path,
                metadata: &expected_meta,
                app,
                progress_event,
                cancel,
            },
        )?;
        emit_cloud_open_progress(
            app,
            progress_event,
            size.unwrap_or(1),
            size.unwrap_or(1),
            true,
        );
    }

    Ok(cache_path)
}

fn cache_is_fresh(
    cache_path: &Path,
    metadata_path: &Path,
    expected: &CloudOpenCacheMetadata,
) -> bool {
    if !cache_path.is_file() || !metadata_path.is_file() {
        return false;
    }
    let stored = fs::read_to_string(metadata_path)
        .ok()
        .and_then(|raw| serde_json::from_str::<CloudOpenCacheMetadata>(&raw).ok());
    stored.as_ref() == Some(expected)
}

fn download_cloud_file_to_cache(
    provider: &RcloneCloudProvider,
    src: &super::path::CloudPath,
    ctx: CloudOpenDownloadContext<'_>,
) -> CloudCommandResult<()> {
    let CloudOpenDownloadContext {
        cache_path,
        metadata_path,
        metadata,
        app,
        progress_event,
        cancel,
    } = ctx;
    if let Some(parent) = cache_path.parent() {
        prepare_cloud_open_cache_dir(parent)?;
    }
    let file_name = cache_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            CloudCommandError::new(
                CloudCommandErrorCode::TaskFailed,
                "Failed to derive cloud-open cache filename",
            )
        })?;
    let part_path = cache_path.with_file_name(format!("{file_name}{CLOUD_OPEN_PART_SUFFIX}"));
    let part_meta_path = cloud_open_metadata_path(&part_path);
    let _ = fs::remove_file(&part_path);
    let _ = fs::remove_file(&part_meta_path);

    if let Some(event_name) = progress_event {
        provider.download_file_with_progress(
            src,
            &part_path,
            event_name,
            cancel,
            |bytes, total| emit_cloud_open_progress(app, Some(event_name), bytes, total, false),
        )?;
    } else {
        provider.download_file(src, &part_path, cancel)?;
    }

    let raw_meta = serde_json::to_vec(metadata).map_err(|error| {
        CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            format!("Failed to serialize cloud-open cache metadata: {error}"),
        )
    })?;
    fs::write(&part_meta_path, raw_meta).map_err(|error| {
        CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            format!("Failed to write cloud-open cache metadata: {error}"),
        )
    })?;
    set_owner_only_permissions(&part_path, false)?;
    set_owner_only_permissions(&part_meta_path, false)?;
    fs::rename(&part_path, cache_path).map_err(|error| {
        CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            format!("Failed to finalize downloaded cloud-open cache file: {error}"),
        )
    })?;
    fs::rename(&part_meta_path, metadata_path).map_err(|error| {
        CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            format!("Failed to finalize cloud-open cache metadata: {error}"),
        )
    })?;
    set_owner_only_permissions(cache_path, false)?;
    set_owner_only_permissions(metadata_path, false)?;
    Ok(())
}

fn emit_cloud_open_progress(
    app: &tauri::AppHandle,
    progress_event: Option<&str>,
    bytes: u64,
    total: u64,
    finished: bool,
) {
    let Some(event_name) = progress_event else {
        return;
    };
    if total == 0 {
        return;
    }
    let _ = runtime_lifecycle::emit_if_running(
        app,
        event_name,
        CloudOpenProgressPayload {
            bytes,
            total,
            finished,
        },
    );
}

fn materialize_inflight_key(
    path: &super::path::CloudPath,
    size: Option<u64>,
    modified: Option<&str>,
) -> String {
    let mut hasher = Hasher::new();
    hasher.update(path.to_string().as_bytes());
    match size {
        Some(value) => {
            hasher.update(&[1]);
            hasher.update(&value.to_le_bytes());
        }
        None => {
            hasher.update(&[0]);
        }
    }
    match modified {
        Some(value) => {
            hasher.update(&[1]);
            hasher.update(value.as_bytes());
        }
        None => {
            hasher.update(&[0]);
        }
    }
    hasher.finalize().to_hex().to_string()
}

fn register_materialize_waiter(key: &str) -> Option<mpsc::Receiver<CloudCommandResult<PathBuf>>> {
    let mut inflight = MATERIALIZE_INFLIGHT
        .lock()
        .expect("materialize inflight mutex poisoned");
    if let Some(waiters) = inflight.get_mut(key) {
        let (tx, rx) = mpsc::channel();
        waiters.push(tx);
        return Some(rx);
    }
    inflight.insert(key.to_string(), Vec::new());
    None
}

fn wait_for_materialize_result(
    path: &super::path::CloudPath,
    key: &str,
    rx: mpsc::Receiver<CloudCommandResult<PathBuf>>,
) -> CloudCommandResult<PathBuf> {
    match rx.recv_timeout(MATERIALIZE_WAIT_TIMEOUT) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            debug!(
                op = "cloud_materialize_wait",
                key,
                path = %path,
                timeout_ms = MATERIALIZE_WAIT_TIMEOUT.as_millis() as u64,
                "cloud materialization waiter timed out"
            );
            Err(CloudCommandError::new(
                CloudCommandErrorCode::Timeout,
                "Cloud materialization wait timed out",
            ))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            "Cloud file materialization task was cancelled",
        )),
    }
}

fn notify_materialize_waiters(key: &str, result: CloudCommandResult<PathBuf>) {
    let waiters = {
        let mut inflight = MATERIALIZE_INFLIGHT
            .lock()
            .expect("materialize inflight mutex poisoned");
        inflight.remove(key)
    };
    if let Some(waiters) = waiters {
        for tx in waiters {
            let _ = tx.send(result.clone());
        }
    }
}

fn cloud_open_cache_path(
    path: &super::path::CloudPath,
    original_name: &str,
) -> CloudCommandResult<PathBuf> {
    let base = cloud_open_cache_root_path();
    prepare_cloud_open_cache_dir(&base)?;
    let mut hasher = Hasher::new();
    hasher.update(path.to_string().as_bytes());
    let key = hasher.finalize().to_hex().to_string();
    let extension = Path::new(original_name)
        .extension()
        .and_then(|ext| ext.to_str())
        .filter(|ext| !ext.is_empty());
    let file_name = match extension {
        Some(ext) => format!("{key}.{ext}"),
        None => key,
    };
    Ok(base.join(file_name))
}

fn cloud_open_cache_root_path() -> PathBuf {
    dirs_next::cache_dir()
        .or_else(dirs_next::data_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("browsey")
        .join(CLOUD_OPEN_CACHE_DIRNAME)
}

fn prepare_cloud_open_cache_dir(path: &Path) -> CloudCommandResult<()> {
    fs::create_dir_all(path).map_err(|error| {
        CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            format!("Failed to create cloud-open cache directory: {error}"),
        )
    })?;
    set_owner_only_permissions(path, true)?;
    prune_cloud_open_cache_dir(path, CLOUD_OPEN_CACHE_MAX_AGE, SystemTime::now())?;
    Ok(())
}

fn prune_cloud_open_cache_dir(
    path: &Path,
    max_age: Duration,
    now: SystemTime,
) -> CloudCommandResult<()> {
    let read_dir = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(CloudCommandError::new(
                CloudCommandErrorCode::TaskFailed,
                format!("Failed to read cloud-open cache directory: {error}"),
            ));
        }
    };
    for entry in read_dir {
        let entry = entry.map_err(|error| {
            CloudCommandError::new(
                CloudCommandErrorCode::TaskFailed,
                format!("Failed to inspect cloud-open cache entry: {error}"),
            )
        })?;
        let entry_path = entry.path();
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(CloudCommandError::new(
                    CloudCommandErrorCode::TaskFailed,
                    format!("Failed to read cloud-open cache metadata: {error}"),
                ));
            }
        };
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        let age = match now.duration_since(modified) {
            Ok(age) => age,
            Err(_) => continue,
        };
        if age < max_age {
            continue;
        }

        if metadata.is_dir() {
            remove_stale_cache_dir(&entry_path)?;
            continue;
        }

        let Some(name) = entry_path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if name.ends_with(".json") {
            remove_stale_cache_file(&entry_path)?;
            let paired = entry_path.with_file_name(name.trim_end_matches(".json"));
            remove_stale_cache_file(&paired)?;
            continue;
        }

        // Keep managed data files while their metadata is still fresh.
        if cloud_open_metadata_path(&entry_path).is_file() {
            continue;
        }
        remove_stale_cache_file(&entry_path)?;
    }
    Ok(())
}

fn remove_stale_cache_file(path: &Path) -> CloudCommandResult<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            format!("Failed to prune stale cloud-open cache entry: {error}"),
        )),
    }
}

fn remove_stale_cache_dir(path: &Path) -> CloudCommandResult<()> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            format!("Failed to prune stale cloud-open cache entry: {error}"),
        )),
    }
}

fn cloud_open_cache_stats(path: &Path) -> CloudCommandResult<(u64, u64)> {
    let read_dir = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok((0, 0)),
        Err(error) => {
            return Err(CloudCommandError::new(
                CloudCommandErrorCode::TaskFailed,
                format!("Failed to read cloud-open cache directory: {error}"),
            ));
        }
    };
    let mut files = 0u64;
    let mut bytes = 0u64;
    for entry in read_dir {
        let entry = entry.map_err(|error| {
            CloudCommandError::new(
                CloudCommandErrorCode::TaskFailed,
                format!("Failed to inspect cloud-open cache entry: {error}"),
            )
        })?;
        let metadata = entry.metadata().map_err(|error| {
            CloudCommandError::new(
                CloudCommandErrorCode::TaskFailed,
                format!("Failed to read cloud-open cache entry metadata: {error}"),
            )
        })?;
        if metadata.is_file() {
            files += 1;
            bytes += metadata.len();
        }
    }
    Ok((files, bytes))
}

fn cloud_open_metadata_path(cache_path: &Path) -> PathBuf {
    let file_name = cache_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("cache-entry");
    cache_path.with_file_name(format!("{file_name}.json"))
}

fn set_owner_only_permissions(path: &Path, is_dir: bool) -> CloudCommandResult<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = if is_dir { 0o700 } else { 0o600 };
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|error| {
            CloudCommandError::new(
                CloudCommandErrorCode::TaskFailed,
                format!(
                    "Failed to secure cloud-open cache {} permissions: {error}",
                    if is_dir { "directory" } else { "file" }
                ),
            )
        })?;
    }
    #[cfg(not(unix))]
    let _ = (path, is_dir);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        cache_is_fresh, clear_cloud_open_cache_impl, cloud_open_cache_root_path,
        cloud_open_cache_stats, cloud_open_metadata_path, notify_materialize_waiters,
        prepare_cloud_open_cache_dir, prune_cloud_open_cache_dir, register_materialize_waiter,
        wait_for_materialize_result, CloudOpenCacheMetadata,
    };
    use crate::commands::cloud::path::CloudPath;
    use crate::commands::cloud::CloudCommandErrorCode;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    #[test]
    fn cache_freshness_requires_matching_metadata_and_file() {
        let root =
            std::env::temp_dir().join(format!("browsey-cloud-open-test-{}", std::process::id()));
        let cache_path = root.join("entry.txt");
        let metadata_path = cloud_open_metadata_path(&cache_path);
        let expected = CloudOpenCacheMetadata {
            source_path: "rclone://work/docs/entry.txt".to_string(),
            size: Some(4),
            modified: Some("2026-02-28 12:00".to_string()),
        };

        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create temp dir");
        fs::write(&cache_path, b"data").expect("write cache file");
        fs::write(
            &metadata_path,
            serde_json::to_vec(&expected).expect("serialize metadata"),
        )
        .expect("write metadata");
        assert!(cache_is_fresh(&cache_path, &metadata_path, &expected));

        let stale = CloudOpenCacheMetadata {
            modified: Some("2026-02-28 12:01".to_string()),
            ..expected.clone()
        };
        assert!(!cache_is_fresh(&cache_path, &metadata_path, &stale));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn cache_prepare_prunes_stale_entries() {
        let root =
            std::env::temp_dir().join(format!("browsey-cloud-open-prune-{}", std::process::id()));
        let stale_path = root.join("stale.bin");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create temp dir");
        fs::write(&stale_path, b"stale").expect("write stale file");
        prune_cloud_open_cache_dir(&root, Duration::ZERO, SystemTime::now())
            .expect("prune stale cache");
        assert!(!stale_path.exists(), "stale cache file should be removed");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn prune_stale_metadata_removes_paired_data_file() {
        let root = std::env::temp_dir().join(format!(
            "browsey-cloud-open-prune-meta-{}",
            std::process::id()
        ));
        let data_path = root.join("entry.bin");
        let metadata_path = cloud_open_metadata_path(&data_path);

        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create temp dir");
        fs::write(&data_path, b"data").expect("write data");
        fs::write(&metadata_path, br#"{"source_path":"rclone://work/a.bin"}"#)
            .expect("write metadata");

        prune_cloud_open_cache_dir(&root, Duration::ZERO, SystemTime::now())
            .expect("prune stale metadata");
        assert!(!data_path.exists(), "paired data file should be removed");
        assert!(!metadata_path.exists(), "metadata file should be removed");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn prune_does_not_remove_managed_data_when_metadata_is_fresh() {
        let root = std::env::temp_dir().join(format!(
            "browsey-cloud-open-prune-fresh-{}",
            std::process::id()
        ));
        let data_path = root.join("entry.bin");
        let metadata_path = cloud_open_metadata_path(&data_path);

        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create temp dir");
        fs::write(&data_path, b"data").expect("write data");
        std::thread::sleep(Duration::from_millis(1200));
        fs::write(&metadata_path, br#"{"source_path":"rclone://work/a.bin"}"#)
            .expect("write metadata");

        let now = fs::metadata(&data_path)
            .expect("data metadata")
            .modified()
            .expect("data modified")
            .checked_add(Duration::from_millis(900))
            .expect("synthetic now");
        prune_cloud_open_cache_dir(&root, Duration::from_millis(800), now)
            .expect("prune should keep managed file");
        assert!(data_path.exists(), "managed data file should stay");
        assert!(metadata_path.exists(), "fresh metadata should stay");

        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn cache_prepare_enforces_user_only_directory_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let root =
            std::env::temp_dir().join(format!("browsey-cloud-open-secure-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        prepare_cloud_open_cache_dir(&root).expect("prepare cache dir");
        let mode = fs::metadata(&root)
            .expect("cache dir metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o700);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn clear_cloud_open_cache_reports_removed_files_and_bytes() {
        let root = cloud_open_cache_root_path();
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create cache root");
        fs::write(root.join("a.bin"), b"1234").expect("write cache file");
        fs::write(root.join("a.bin.json"), b"{}").expect("write metadata file");

        let result = clear_cloud_open_cache_impl().expect("clear cloud-open cache");
        assert_eq!(result.removed_files, 2);
        assert_eq!(result.removed_bytes, 6);
        assert!(root.exists(), "cache directory should be recreated");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn cloud_open_cache_stats_counts_only_files() {
        let root =
            std::env::temp_dir().join(format!("browsey-cloud-open-stats-{}", std::process::id()));
        let nested = root.join("nested");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&nested).expect("create nested dir");
        fs::write(root.join("one.bin"), b"12").expect("write file one");
        fs::write(root.join("two.bin"), b"123").expect("write file two");

        let (files, bytes) = cloud_open_cache_stats(&root).expect("cache stats");
        assert_eq!(files, 2);
        assert_eq!(bytes, 5);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn materialize_inflight_dedupe_delivers_shared_result_to_waiters() {
        let key = format!("inflight-{}-{}", std::process::id(), 1);
        assert!(
            register_materialize_waiter(&key).is_none(),
            "first caller should become leader"
        );
        let rx = register_materialize_waiter(&key).expect("waiter should subscribe");

        let expected = PathBuf::from("/tmp/cloud-open-test.bin");
        notify_materialize_waiters(&key, Ok(expected.clone()));

        let received = rx
            .recv_timeout(Duration::from_secs(1))
            .expect("waiter result");
        let received_path = received.expect("waiter should receive success");
        assert_eq!(received_path, expected);
    }

    #[test]
    fn materialize_waiter_timeout_returns_typed_timeout_error() {
        let key = format!("inflight-timeout-{}-{}", std::process::id(), 1);
        let path = CloudPath::parse("rclone://work/docs/file.txt").expect("cloud path");

        assert!(
            register_materialize_waiter(&key).is_none(),
            "first caller should become leader"
        );
        let rx = register_materialize_waiter(&key).expect("waiter should subscribe");
        let err = wait_for_materialize_result(&path, &key, rx).expect_err("waiter should timeout");
        assert_eq!(err.code(), CloudCommandErrorCode::Timeout);
        assert!(
            err.to_string().to_lowercase().contains("timed out"),
            "unexpected error: {}",
            err
        );
    }
}
