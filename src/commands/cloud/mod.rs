//! Cloud-provider Tauri commands (rclone-backed, CLI-first).

mod cache;
mod conflicts;
mod error;
mod events;
mod limits;
mod list;
pub mod path;
pub mod provider;
pub mod providers;
pub mod rclone_cli;
pub mod rclone_rc;
pub mod types;
mod write;

use crate::errors::api_error::ApiResult;
use crate::tasks::{CancelGuard, CancelState};
use cache::list_cloud_remotes_cached;
use error::{map_api_result, CloudCommandError, CloudCommandErrorCode, CloudCommandResult};
use path::CloudPath;
use tracing::warn;
use types::{CloudConflictInfo, CloudEntry, CloudProviderKind, CloudRemote, CloudRootSelection};

pub(crate) fn cloud_provider_kind_for_remote(remote_id: &str) -> Option<CloudProviderKind> {
    list_cloud_remotes_cached(false)
        .ok()
        .and_then(|remotes| remotes.into_iter().find(|remote| remote.id == remote_id))
        .map(|remote| remote.provider)
}

pub(crate) fn cloud_conflict_name_key(provider: Option<CloudProviderKind>, name: &str) -> String {
    match provider {
        // OneDrive is effectively case-insensitive for path conflicts.
        Some(CloudProviderKind::Onedrive) => name.to_ascii_lowercase(),
        _ => name.to_string(),
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

#[tauri::command]
pub fn cloud_rc_health() -> ApiResult<rclone_rc::RcloneRcHealth> {
    map_api_result(Ok(rclone_rc::health_snapshot()))
}

async fn list_cloud_remotes_impl() -> CloudCommandResult<Vec<CloudRemote>> {
    list::list_cloud_remotes_impl().await
}

#[tauri::command]
pub async fn validate_cloud_root(path: String) -> ApiResult<CloudRootSelection> {
    map_api_result(validate_cloud_root_impl(path).await)
}

async fn validate_cloud_root_impl(path: String) -> CloudCommandResult<CloudRootSelection> {
    list::validate_cloud_root_impl(path).await
}

#[tauri::command]
pub async fn list_cloud_entries(path: String, app: tauri::AppHandle) -> ApiResult<Vec<CloudEntry>> {
    map_api_result(list_cloud_entries_impl(path, app).await)
}

async fn list_cloud_entries_impl(
    path: String,
    app: tauri::AppHandle,
) -> CloudCommandResult<Vec<CloudEntry>> {
    list::list_cloud_entries_impl(path, app).await
}

#[tauri::command]
pub async fn stat_cloud_entry(path: String) -> ApiResult<Option<CloudEntry>> {
    map_api_result(stat_cloud_entry_impl(path).await)
}

async fn stat_cloud_entry_impl(path: String) -> CloudCommandResult<Option<CloudEntry>> {
    list::stat_cloud_entry_impl(path).await
}

#[tauri::command]
pub async fn create_cloud_folder(
    path: String,
    cancel: tauri::State<'_, CancelState>,
    progress_event: Option<String>,
) -> ApiResult<()> {
    map_api_result(create_cloud_folder_impl(path, cancel.inner().clone(), progress_event).await)
}

async fn create_cloud_folder_impl(
    path: String,
    cancel_state: CancelState,
    progress_event: Option<String>,
) -> CloudCommandResult<()> {
    write::create_cloud_folder_impl(path, cancel_state, progress_event).await
}

#[tauri::command]
pub async fn delete_cloud_file(
    path: String,
    cancel: tauri::State<'_, CancelState>,
    progress_event: Option<String>,
) -> ApiResult<()> {
    map_api_result(delete_cloud_file_impl(path, cancel.inner().clone(), progress_event).await)
}

async fn delete_cloud_file_impl(
    path: String,
    cancel_state: CancelState,
    progress_event: Option<String>,
) -> CloudCommandResult<()> {
    write::delete_cloud_file_impl(path, cancel_state, progress_event).await
}

#[tauri::command]
pub async fn delete_cloud_dir_recursive(
    path: String,
    cancel: tauri::State<'_, CancelState>,
    progress_event: Option<String>,
) -> ApiResult<()> {
    map_api_result(
        delete_cloud_dir_recursive_impl(path, cancel.inner().clone(), progress_event).await,
    )
}

async fn delete_cloud_dir_recursive_impl(
    path: String,
    cancel_state: CancelState,
    progress_event: Option<String>,
) -> CloudCommandResult<()> {
    write::delete_cloud_dir_recursive_impl(path, cancel_state, progress_event).await
}

#[tauri::command]
pub async fn delete_cloud_dir_empty(
    path: String,
    cancel: tauri::State<'_, CancelState>,
    progress_event: Option<String>,
) -> ApiResult<()> {
    map_api_result(delete_cloud_dir_empty_impl(path, cancel.inner().clone(), progress_event).await)
}

async fn delete_cloud_dir_empty_impl(
    path: String,
    cancel_state: CancelState,
    progress_event: Option<String>,
) -> CloudCommandResult<()> {
    write::delete_cloud_dir_empty_impl(path, cancel_state, progress_event).await
}

#[tauri::command]
pub async fn move_cloud_entry(
    src: String,
    dst: String,
    overwrite: Option<bool>,
    prechecked: Option<bool>,
    cancel: tauri::State<'_, CancelState>,
    progress_event: Option<String>,
) -> ApiResult<()> {
    map_api_result(
        move_cloud_entry_impl(
            src,
            dst,
            overwrite.unwrap_or(false),
            prechecked.unwrap_or(false),
            cancel.inner().clone(),
            progress_event,
        )
        .await,
    )
}

async fn move_cloud_entry_impl(
    src: String,
    dst: String,
    overwrite: bool,
    prechecked: bool,
    cancel_state: CancelState,
    progress_event: Option<String>,
) -> CloudCommandResult<()> {
    write::move_cloud_entry_impl(
        src,
        dst,
        overwrite,
        prechecked,
        cancel_state,
        progress_event,
    )
    .await
}

#[tauri::command]
pub async fn rename_cloud_entry(
    src: String,
    dst: String,
    overwrite: Option<bool>,
    prechecked: Option<bool>,
    cancel: tauri::State<'_, CancelState>,
    progress_event: Option<String>,
) -> ApiResult<()> {
    map_api_result(
        move_cloud_entry_impl(
            src,
            dst,
            overwrite.unwrap_or(false),
            prechecked.unwrap_or(false),
            cancel.inner().clone(),
            progress_event,
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
    cancel: tauri::State<'_, CancelState>,
    progress_event: Option<String>,
) -> ApiResult<()> {
    map_api_result(
        copy_cloud_entry_impl(
            src,
            dst,
            overwrite.unwrap_or(false),
            prechecked.unwrap_or(false),
            cancel.inner().clone(),
            progress_event,
        )
        .await,
    )
}

async fn copy_cloud_entry_impl(
    src: String,
    dst: String,
    overwrite: bool,
    prechecked: bool,
    cancel_state: CancelState,
    progress_event: Option<String>,
) -> CloudCommandResult<()> {
    write::copy_cloud_entry_impl(
        src,
        dst,
        overwrite,
        prechecked,
        cancel_state,
        progress_event,
    )
    .await
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
    conflicts::preview_cloud_conflicts_impl(sources, dest_dir).await
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

fn register_cloud_cancel(
    cancel_state: &CancelState,
    progress_event: &Option<String>,
) -> CloudCommandResult<Option<CancelGuard>> {
    progress_event
        .as_ref()
        .map(|event| cancel_state.register(event.clone()))
        .transpose()
        .map_err(|error| {
            CloudCommandError::new(
                CloudCommandErrorCode::TaskFailed,
                format!("Failed to register cloud cancel token: {error}"),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::{normalize_cloud_path, normalize_cloud_path_impl};

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
}
