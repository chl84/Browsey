use super::{
    cache::{list_cloud_dir_cached_interactive_with_refresh_event, list_cloud_remotes_cached},
    configured_rclone_provider, ensure_cloud_enabled,
    error::{CloudCommandError, CloudCommandErrorCode, CloudCommandResult},
    limits::with_cloud_remote_permits,
    map_spawn_result, parse_cloud_path_arg,
    provider::CloudProvider,
    providers::rclone::RcloneReadOptions,
    register_cloud_cancel,
    types::{CloudEntry, CloudEntryKind, CloudRemote, CloudRootSelection},
    CancelState,
};
use crate::errors::domain::ErrorCode;
use std::time::Instant;
use tracing::{debug, warn};

const CLOUD_INTERACTIVE_RC_STAT_TIMEOUT_SECS: u64 = 10;
const CLOUD_INTERACTIVE_CLI_STAT_TIMEOUT_SECS: u64 = 20;

pub(super) async fn list_cloud_remotes_impl() -> CloudCommandResult<Vec<CloudRemote>> {
    ensure_cloud_enabled()?;
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
    ensure_cloud_enabled()?;
    let path = parse_cloud_path_arg(path)?;
    let task = tauri::async_runtime::spawn_blocking(move || {
        let provider = configured_rclone_provider().map_err(CloudCommandError::from)?;
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
            let stat = provider
                .stat_path_with_read_options(
                    &path,
                    RcloneReadOptions {
                        cancel: None,
                        rc_timeout: Some(std::time::Duration::from_secs(
                            CLOUD_INTERACTIVE_RC_STAT_TIMEOUT_SECS,
                        )),
                        cli_timeout: Some(std::time::Duration::from_secs(
                            CLOUD_INTERACTIVE_CLI_STAT_TIMEOUT_SECS,
                        )),
                    },
                )?
                .ok_or_else(|| {
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
    cancel_state: CancelState,
    progress_event: Option<String>,
) -> CloudCommandResult<Vec<CloudEntry>> {
    ensure_cloud_enabled()?;
    let started = Instant::now();
    let path = parse_cloud_path_arg(path)?;
    let path_for_log = path.clone();
    let cancel_guard = register_cloud_cancel(&cancel_state, &progress_event)?;
    let cancel_token = cancel_guard.as_ref().map(|guard| guard.token());
    let task = tauri::async_runtime::spawn_blocking(move || {
        list_cloud_dir_cached_interactive_with_refresh_event(
            &path,
            Some(app),
            cancel_token.as_deref(),
        )
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
    if let Err(error) = &result {
        warn!(
            op = "cloud_list_entries",
            path = %path_for_log,
            elapsed_ms,
            code = error.code().as_code_str(),
            error = %error,
            "interactive cloud directory load failed"
        );
    }
    result
}

pub(super) async fn stat_cloud_entry_impl(path: String) -> CloudCommandResult<Option<CloudEntry>> {
    ensure_cloud_enabled()?;
    let path = parse_cloud_path_arg(path)?;
    let remote = path.remote().to_string();
    let task = tauri::async_runtime::spawn_blocking(move || {
        with_cloud_remote_permits(vec![remote], || {
            let provider = configured_rclone_provider().map_err(CloudCommandError::from)?;
            provider.stat_path_with_read_options(
                &path,
                RcloneReadOptions {
                    cancel: None,
                    rc_timeout: Some(std::time::Duration::from_secs(
                        CLOUD_INTERACTIVE_RC_STAT_TIMEOUT_SECS,
                    )),
                    cli_timeout: Some(std::time::Duration::from_secs(
                        CLOUD_INTERACTIVE_CLI_STAT_TIMEOUT_SECS,
                    )),
                },
            )
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
