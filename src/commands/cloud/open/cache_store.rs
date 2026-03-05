use super::{CloudCommandError, CloudCommandErrorCode, CloudCommandResult};
use crate::commands::cloud::{
    path::CloudPath, provider::CloudProvider, providers::rclone::RcloneCloudProvider,
};
use blake3::Hasher;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, SystemTime};

const CLOUD_OPEN_CACHE_DIRNAME: &str = "cloud-open";
const CLOUD_OPEN_PART_SUFFIX: &str = ".part";
#[cfg(not(test))]
const CLOUD_OPEN_CACHE_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);
#[cfg(test)]
const CLOUD_OPEN_CACHE_MAX_AGE: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct CloudOpenCacheMetadata {
    pub(super) source_path: String,
    pub(super) size: Option<u64>,
    pub(super) modified: Option<String>,
}

pub(super) struct CloudOpenDownloadContext<'a> {
    pub(super) provider: &'a RcloneCloudProvider,
    pub(super) src: &'a CloudPath,
    pub(super) cache_path: &'a Path,
    pub(super) metadata_path: &'a Path,
    pub(super) metadata: &'a CloudOpenCacheMetadata,
    pub(super) progress_event: Option<&'a str>,
    pub(super) cancel: Option<&'a AtomicBool>,
}

pub(super) fn cloud_open_cache_path(
    path: &CloudPath,
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

pub(super) fn cloud_open_cache_root_path() -> PathBuf {
    dirs_next::cache_dir()
        .or_else(dirs_next::data_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("browsey")
        .join(CLOUD_OPEN_CACHE_DIRNAME)
}

pub(super) fn cloud_open_metadata_path(cache_path: &Path) -> PathBuf {
    let file_name = cache_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("cache-entry");
    cache_path.with_file_name(format!("{file_name}.json"))
}

pub(super) fn cache_is_fresh(
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

pub(super) fn download_cloud_file_to_cache<F>(
    ctx: CloudOpenDownloadContext<'_>,
    on_progress: F,
) -> CloudCommandResult<()>
where
    F: FnMut(u64, u64),
{
    let CloudOpenDownloadContext {
        provider,
        src,
        cache_path,
        metadata_path,
        metadata,
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
        provider.download_file_with_progress(src, &part_path, event_name, cancel, on_progress)?;
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

pub(super) fn prepare_cloud_open_cache_dir(path: &Path) -> CloudCommandResult<()> {
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

pub(super) fn prune_cloud_open_cache_dir(
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

pub(super) fn cloud_open_cache_stats(path: &Path) -> CloudCommandResult<(u64, u64)> {
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
