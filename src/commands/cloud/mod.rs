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
use types::{CloudConflictInfo, CloudEntry, CloudEntryKind, CloudRemote, CloudRootSelection};

#[tauri::command]
pub async fn list_cloud_remotes() -> ApiResult<Vec<CloudRemote>> {
    map_api_result(list_cloud_remotes_impl().await)
}

async fn list_cloud_remotes_impl() -> CloudCommandResult<Vec<CloudRemote>> {
    let task = tauri::async_runtime::spawn_blocking(|| {
        let provider = RcloneCloudProvider::default();
        provider.list_remotes()
    });
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
                    format!("Cloud remote is not configured or unsupported: {}", path.remote()),
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
    let path = CloudPath::parse(&path).map_err(|error| {
        CloudCommandError::new(
            CloudCommandErrorCode::InvalidPath,
            format!("Invalid cloud path: {error}"),
        )
    })?;
    let task = tauri::async_runtime::spawn_blocking(move || {
        let provider = RcloneCloudProvider::default();
        provider.list_dir(&path)
    });
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
    let task = tauri::async_runtime::spawn_blocking(move || {
        let provider = RcloneCloudProvider::default();
        provider.mkdir(&path)
    });
    match task.await {
        Ok(result) => result,
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
    let task = tauri::async_runtime::spawn_blocking(move || {
        let provider = RcloneCloudProvider::default();
        provider.delete_file(&path)
    });
    map_spawn_result(task.await, "cloud delete file task failed")
}

#[tauri::command]
pub async fn delete_cloud_dir_recursive(path: String) -> ApiResult<()> {
    map_api_result(delete_cloud_dir_recursive_impl(path).await)
}

async fn delete_cloud_dir_recursive_impl(path: String) -> CloudCommandResult<()> {
    let path = parse_cloud_path_arg(path)?;
    let task = tauri::async_runtime::spawn_blocking(move || {
        let provider = RcloneCloudProvider::default();
        provider.delete_dir_recursive(&path)
    });
    map_spawn_result(task.await, "cloud delete dir task failed")
}

#[tauri::command]
pub async fn delete_cloud_dir_empty(path: String) -> ApiResult<()> {
    map_api_result(delete_cloud_dir_empty_impl(path).await)
}

async fn delete_cloud_dir_empty_impl(path: String) -> CloudCommandResult<()> {
    let path = parse_cloud_path_arg(path)?;
    let task = tauri::async_runtime::spawn_blocking(move || {
        let provider = RcloneCloudProvider::default();
        provider.delete_dir_empty(&path)
    });
    map_spawn_result(task.await, "cloud rmdir task failed")
}

#[tauri::command]
pub async fn move_cloud_entry(src: String, dst: String) -> ApiResult<()> {
    map_api_result(move_cloud_entry_impl(src, dst).await)
}

async fn move_cloud_entry_impl(src: String, dst: String) -> CloudCommandResult<()> {
    let src = parse_cloud_path_arg(src)?;
    let dst = parse_cloud_path_arg(dst)?;
    let task = tauri::async_runtime::spawn_blocking(move || {
        let provider = RcloneCloudProvider::default();
        provider.move_entry(&src, &dst)
    });
    map_spawn_result(task.await, "cloud move task failed")
}

#[tauri::command]
pub async fn rename_cloud_entry(src: String, dst: String) -> ApiResult<()> {
    map_api_result(move_cloud_entry_impl(src, dst).await)
}

#[tauri::command]
pub async fn copy_cloud_entry(src: String, dst: String) -> ApiResult<()> {
    map_api_result(copy_cloud_entry_impl(src, dst).await)
}

async fn copy_cloud_entry_impl(src: String, dst: String) -> CloudCommandResult<()> {
    let src = parse_cloud_path_arg(src)?;
    let dst = parse_cloud_path_arg(dst)?;
    let task = tauri::async_runtime::spawn_blocking(move || {
        let provider = RcloneCloudProvider::default();
        provider.copy_entry(&src, &dst)
    });
    map_spawn_result(task.await, "cloud copy task failed")
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
        let mut conflicts = Vec::new();
        for src in &sources {
            let name = src.leaf_name().map_err(|error| {
                CloudCommandError::new(
                    CloudCommandErrorCode::InvalidPath,
                    format!("Invalid source cloud path for conflict preview: {error}"),
                )
            })?;
            let target = dest_dir.child_path(name).map_err(|error| {
                CloudCommandError::new(
                    CloudCommandErrorCode::InvalidPath,
                    format!("Invalid target cloud path for conflict preview: {error}"),
                )
            })?;
            let existing = provider.stat_path(&target)?;
            let exists = existing.is_some();
            let is_dir = existing
                .as_ref()
                .map(|entry| matches!(entry.kind, CloudEntryKind::Dir))
                .unwrap_or(false);
            if exists {
                conflicts.push(CloudConflictInfo {
                    src: src.to_string(),
                    target: target.to_string(),
                    exists,
                    is_dir,
                });
            }
        }
        Ok(conflicts)
    });
    map_spawn_result(task.await, "cloud conflict preview task failed")
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
