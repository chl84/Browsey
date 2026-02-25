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
use types::{CloudEntry, CloudRemote};

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
pub fn normalize_cloud_path(path: String) -> ApiResult<String> {
    map_api_result(normalize_cloud_path_impl(path))
}

fn normalize_cloud_path_impl(path: String) -> CloudCommandResult<String> {
    let path = CloudPath::parse(&path).map_err(|error| {
        CloudCommandError::new(
            CloudCommandErrorCode::InvalidPath,
            format!("Invalid cloud path: {error}"),
        )
    })?;
    Ok(path.to_string())
}
