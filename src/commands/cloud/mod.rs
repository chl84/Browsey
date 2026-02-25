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
pub fn list_cloud_remotes() -> ApiResult<Vec<CloudRemote>> {
    map_api_result(list_cloud_remotes_impl())
}

fn list_cloud_remotes_impl() -> CloudCommandResult<Vec<CloudRemote>> {
    let provider = RcloneCloudProvider::default();
    provider.list_remotes()
}

#[tauri::command]
pub fn list_cloud_entries(path: String) -> ApiResult<Vec<CloudEntry>> {
    map_api_result(list_cloud_entries_impl(path))
}

fn list_cloud_entries_impl(path: String) -> CloudCommandResult<Vec<CloudEntry>> {
    let path = CloudPath::parse(&path).map_err(|error| {
        CloudCommandError::new(
            CloudCommandErrorCode::InvalidPath,
            format!("Invalid cloud path: {error}"),
        )
    })?;
    let provider = RcloneCloudProvider::default();
    provider.list_dir(&path)
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
