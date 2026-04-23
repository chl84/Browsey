use super::{
    configured_rclone_provider, ensure_cloud_enabled,
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
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tracing::debug;

mod cache_store;
mod inflight;

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
    ensure_cloud_enabled()?;
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
    let dir = cache_store::cloud_open_cache_root_path();
    if !dir.exists() {
        cache_store::prepare_cloud_open_cache_dir(&dir)?;
        return Ok(CloudOpenCacheClearResult {
            removed_files: 0,
            removed_bytes: 0,
        });
    }

    let (removed_files, removed_bytes) = cache_store::cloud_open_cache_stats(&dir)?;
    fs::remove_dir_all(&dir).map_err(|error| {
        CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            format!("Failed to clear cloud-open cache: {error}"),
        )
    })?;
    cache_store::prepare_cloud_open_cache_dir(&dir)?;

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
    let provider = configured_rclone_provider().map_err(CloudCommandError::from)?;
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
    let provider = configured_rclone_provider().map_err(CloudCommandError::from)?;
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
    inflight::materialize_with_inflight_dedupe(path, snapshot, || {
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
    let cache_path = cache_store::cloud_open_cache_path(path, original_name)?;
    let metadata_path = cache_store::cloud_open_metadata_path(&cache_path);
    let expected_meta = cache_store::CloudOpenCacheMetadata {
        source_path: path.to_string(),
        size,
        modified: modified.map(str::to_string),
    };

    if cache_store::cache_is_fresh(&cache_path, &metadata_path, &expected_meta) {
        emit_cloud_open_progress(
            app,
            progress_event,
            size.unwrap_or(1),
            size.unwrap_or(1),
            true,
        );
    } else {
        cache_store::download_cloud_file_to_cache(
            cache_store::CloudOpenDownloadContext {
                provider,
                src: path,
                cache_path: &cache_path,
                metadata_path: &metadata_path,
                metadata: &expected_meta,
                progress_event,
                cancel,
            },
            |bytes, total| emit_cloud_open_progress(app, progress_event, bytes, total, false),
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

#[cfg(test)]
mod tests {
    use super::{cache_store, clear_cloud_open_cache_impl, inflight};
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
        let metadata_path = cache_store::cloud_open_metadata_path(&cache_path);
        let expected = cache_store::CloudOpenCacheMetadata {
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
        assert!(cache_store::cache_is_fresh(
            &cache_path,
            &metadata_path,
            &expected
        ));

        let stale = cache_store::CloudOpenCacheMetadata {
            modified: Some("2026-02-28 12:01".to_string()),
            ..expected.clone()
        };
        assert!(!cache_store::cache_is_fresh(
            &cache_path,
            &metadata_path,
            &stale
        ));

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
        cache_store::prune_cloud_open_cache_dir(&root, Duration::ZERO, SystemTime::now())
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
        let metadata_path = cache_store::cloud_open_metadata_path(&data_path);

        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create temp dir");
        fs::write(&data_path, b"data").expect("write data");
        fs::write(&metadata_path, br#"{"source_path":"rclone://work/a.bin"}"#)
            .expect("write metadata");

        cache_store::prune_cloud_open_cache_dir(&root, Duration::ZERO, SystemTime::now())
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
        let metadata_path = cache_store::cloud_open_metadata_path(&data_path);

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
        cache_store::prune_cloud_open_cache_dir(&root, Duration::from_millis(800), now)
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
        cache_store::prepare_cloud_open_cache_dir(&root).expect("prepare cache dir");
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
        let root = cache_store::cloud_open_cache_root_path();
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

        let (files, bytes) = cache_store::cloud_open_cache_stats(&root).expect("cache stats");
        assert_eq!(files, 2);
        assert_eq!(bytes, 5);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn materialize_inflight_dedupe_delivers_shared_result_to_waiters() {
        let key = format!("inflight-{}-{}", std::process::id(), 1);
        assert!(
            inflight::register_materialize_waiter(&key).is_none(),
            "first caller should become leader"
        );
        let rx = inflight::register_materialize_waiter(&key).expect("waiter should subscribe");

        let expected = PathBuf::from("/tmp/cloud-open-test.bin");
        inflight::notify_materialize_waiters(&key, Ok(expected.clone()));

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
            inflight::register_materialize_waiter(&key).is_none(),
            "first caller should become leader"
        );
        let rx = inflight::register_materialize_waiter(&key).expect("waiter should subscribe");
        let err = inflight::wait_for_materialize_result(&path, &key, rx)
            .expect_err("waiter should timeout");
        assert_eq!(err.code(), CloudCommandErrorCode::Timeout);
        assert!(
            err.to_string().to_lowercase().contains("timed out"),
            "unexpected error: {}",
            err
        );
    }
}
