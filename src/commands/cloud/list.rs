use super::{
    cache::{list_cloud_dir_cached_with_refresh_event, list_cloud_remotes_cached},
    error::{CloudCommandError, CloudCommandErrorCode, CloudCommandResult},
    limits::with_cloud_remote_permits,
    map_spawn_result, parse_cloud_path_arg,
    provider::CloudProvider,
    providers::rclone::RcloneCloudProvider,
    types::{CloudEntry, CloudEntryKind, CloudRemote, CloudRootSelection},
};
use std::time::Instant;
use tracing::debug;

pub(super) async fn list_cloud_remotes_impl() -> CloudCommandResult<Vec<CloudRemote>> {
    let task = tauri::async_runtime::spawn_blocking(|| list_cloud_remotes_cached(false));
    match task.await {
        Ok(result) => result,
        Err(error) => Err(CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            format!("cloud remote list task failed: {error}"),
        )),
    }
}

pub(super) async fn validate_cloud_root_impl(
    path: String,
) -> CloudCommandResult<CloudRootSelection> {
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

pub(super) async fn list_cloud_entries_impl(
    path: String,
    app: tauri::AppHandle,
) -> CloudCommandResult<Vec<CloudEntry>> {
    let started = Instant::now();
    let path = parse_cloud_path_arg(path)?;
    let path_for_log = path.clone();
    let task = tauri::async_runtime::spawn_blocking(move || {
        list_cloud_dir_cached_with_refresh_event(&path, Some(app))
    });
    let result = match task.await {
        Ok(result) => result,
        Err(error) => Err(CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            format!("cloud list task failed: {error}"),
        )),
    };
    let elapsed_ms = started.elapsed().as_millis() as u64;
    match &result {
        Ok(entries) => debug!(
            op = "cloud_list_entries",
            path = %path_for_log,
            entry_count = entries.len(),
            elapsed_ms,
            "cloud command timing"
        ),
        Err(error) => debug!(
            op = "cloud_list_entries",
            path = %path_for_log,
            elapsed_ms,
            error = %error,
            "cloud command failed"
        ),
    }
    result
}

pub(super) async fn stat_cloud_entry_impl(path: String) -> CloudCommandResult<Option<CloudEntry>> {
    let path = parse_cloud_path_arg(path)?;
    let remote = path.remote().to_string();
    let task = tauri::async_runtime::spawn_blocking(move || {
        with_cloud_remote_permits(vec![remote], || {
            let provider = RcloneCloudProvider::default();
            provider.stat_path(&path)
        })
    });
    match task.await {
        Ok(result) => result,
        Err(error) => Err(CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            format!("cloud stat task failed: {error}"),
        )),
    }
}
