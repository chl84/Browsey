use super::{
    error::{CloudCommandError, CloudCommandErrorCode, CloudCommandResult},
    limits::with_cloud_remote_permits,
    map_spawn_result, parse_cloud_path_arg,
    provider::CloudProvider,
    providers::rclone::RcloneCloudProvider,
    register_cloud_cancel,
    types::CloudEntryKind,
};
use crate::commands::fs::open_path_without_recent;
use crate::runtime_lifecycle;
use crate::tasks::CancelState;
use blake3::Hasher;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tracing::{debug, info};

const CLOUD_OPEN_CACHE_DIRNAME: &str = "cloud-open";
const CLOUD_OPEN_PART_SUFFIX: &str = ".part";

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
        Ok(()) => info!(
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

fn materialize_and_open_cloud_file(
    path: &super::path::CloudPath,
    app: &tauri::AppHandle,
    progress_event: Option<&str>,
    cancel: Option<&AtomicBool>,
) -> CloudCommandResult<()> {
    let provider = RcloneCloudProvider::default();
    let entry = provider.stat_path(path)?.ok_or_else(|| {
        CloudCommandError::new(
            CloudCommandErrorCode::NotFound,
            format!("Cloud file was not found: {path}"),
        )
    })?;
    if !matches!(entry.kind, CloudEntryKind::File) {
        return Err(CloudCommandError::new(
            CloudCommandErrorCode::Unsupported,
            format!("Only cloud files can be opened directly: {path}"),
        ));
    }

    let cache_path = cloud_open_cache_path(path, &entry.name)?;
    let metadata_path = cloud_open_metadata_path(&cache_path);
    let expected_meta = CloudOpenCacheMetadata {
        source_path: path.to_string(),
        size: entry.size,
        modified: entry.modified.clone(),
    };

    if cache_is_fresh(&cache_path, &metadata_path, &expected_meta) {
        emit_cloud_open_progress(
            app,
            progress_event,
            entry.size.unwrap_or(1),
            entry.size.unwrap_or(1),
            true,
        );
    } else {
        download_cloud_file_to_cache(
            &provider,
            path,
            &cache_path,
            &metadata_path,
            &expected_meta,
            app,
            progress_event,
            cancel,
        )?;
        emit_cloud_open_progress(
            app,
            progress_event,
            entry.size.unwrap_or(1),
            entry.size.unwrap_or(1),
            true,
        );
    }

    open_path_without_recent(&cache_path).map_err(|error| {
        CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            format!("Failed to open downloaded cloud file: {error}"),
        )
    })
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
    cache_path: &Path,
    metadata_path: &Path,
    metadata: &CloudOpenCacheMetadata,
    app: &tauri::AppHandle,
    progress_event: Option<&str>,
    cancel: Option<&AtomicBool>,
) -> CloudCommandResult<()> {
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            CloudCommandError::new(
                CloudCommandErrorCode::TaskFailed,
                format!("Failed to create cloud-open cache directory: {error}"),
            )
        })?;
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

fn cloud_open_cache_path(
    path: &super::path::CloudPath,
    original_name: &str,
) -> CloudCommandResult<PathBuf> {
    let base = dirs_next::cache_dir()
        .or_else(dirs_next::data_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("browsey")
        .join(CLOUD_OPEN_CACHE_DIRNAME);
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

fn cloud_open_metadata_path(cache_path: &Path) -> PathBuf {
    let file_name = cache_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("cache-entry");
    cache_path.with_file_name(format!("{file_name}.json"))
}

#[cfg(test)]
mod tests {
    use super::{cache_is_fresh, cloud_open_metadata_path, CloudOpenCacheMetadata};
    use std::fs;

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
}
