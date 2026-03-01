use super::{
    cache::invalidate_cloud_dir_listing_cache_for_write_paths, configured_rclone_provider,
    error::CloudCommandResult, limits::with_cloud_remote_permits, map_spawn_result,
    parse_cloud_path_arg, provider::CloudProvider, register_cloud_cancel,
};
use crate::tasks::CancelState;
use std::time::Instant;
use tracing::debug;

pub(super) async fn create_cloud_folder_impl(
    path: String,
    cancel_state: CancelState,
    progress_event: Option<String>,
) -> CloudCommandResult<()> {
    let started = Instant::now();
    let path = parse_cloud_path_arg(path)?;
    let path_for_invalidate = path.clone();
    let path_for_log = path.clone();
    let remote = path.remote().to_string();
    let cancel_guard = register_cloud_cancel(&cancel_state, &progress_event)?;
    let cancel_token = cancel_guard.as_ref().map(|guard| guard.token());
    let task = tauri::async_runtime::spawn_blocking(move || {
        with_cloud_remote_permits(vec![remote], || {
            let provider = configured_rclone_provider().map_err(|error| {
                super::error::CloudCommandError::new(
                    super::error::CloudCommandErrorCode::InvalidConfig,
                    error,
                )
            })?;
            provider.mkdir(&path, cancel_token.as_deref())
        })
    });
    let result = match task.await {
        Ok(result) => {
            result?;
            invalidate_cloud_dir_listing_cache_for_write_paths(&[path_for_invalidate]);
            Ok(())
        }
        Err(error) => Err(super::error::CloudCommandError::new(
            super::error::CloudCommandErrorCode::TaskFailed,
            format!("cloud mkdir task failed: {error}"),
        )),
    };
    let elapsed_ms = started.elapsed().as_millis() as u64;
    match &result {
        Ok(()) => debug!(
            op = "cloud_write_mkdir",
            path = %path_for_log,
            elapsed_ms,
            "cloud command timing"
        ),
        Err(error) => debug!(
            op = "cloud_write_mkdir",
            path = %path_for_log,
            elapsed_ms,
            error = %error,
            "cloud command failed"
        ),
    }
    result
}

pub(super) async fn delete_cloud_file_impl(
    path: String,
    cancel_state: CancelState,
    progress_event: Option<String>,
) -> CloudCommandResult<()> {
    let started = Instant::now();
    let path = parse_cloud_path_arg(path)?;
    let path_for_invalidate = path.clone();
    let path_for_log = path.clone();
    let remote = path.remote().to_string();
    let cancel_guard = register_cloud_cancel(&cancel_state, &progress_event)?;
    let cancel_token = cancel_guard.as_ref().map(|guard| guard.token());
    let task = tauri::async_runtime::spawn_blocking(move || {
        with_cloud_remote_permits(vec![remote], || {
            let provider = configured_rclone_provider().map_err(|error| {
                super::error::CloudCommandError::new(
                    super::error::CloudCommandErrorCode::InvalidConfig,
                    error,
                )
            })?;
            provider.delete_file(&path, cancel_token.as_deref())
        })
    });
    let result = map_spawn_result(task.await, "cloud delete file task failed").map(|_| {
        invalidate_cloud_dir_listing_cache_for_write_paths(&[path_for_invalidate]);
    });
    let elapsed_ms = started.elapsed().as_millis() as u64;
    match &result {
        Ok(()) => debug!(
            op = "cloud_write_delete_file",
            path = %path_for_log,
            elapsed_ms,
            "cloud command timing"
        ),
        Err(error) => debug!(
            op = "cloud_write_delete_file",
            path = %path_for_log,
            elapsed_ms,
            error = %error,
            "cloud command failed"
        ),
    }
    result
}

pub(super) async fn delete_cloud_dir_recursive_impl(
    path: String,
    cancel_state: CancelState,
    progress_event: Option<String>,
) -> CloudCommandResult<()> {
    let started = Instant::now();
    let path = parse_cloud_path_arg(path)?;
    let path_for_invalidate = path.clone();
    let path_for_log = path.clone();
    let remote = path.remote().to_string();
    let cancel_guard = register_cloud_cancel(&cancel_state, &progress_event)?;
    let cancel_token = cancel_guard.as_ref().map(|guard| guard.token());
    let task = tauri::async_runtime::spawn_blocking(move || {
        with_cloud_remote_permits(vec![remote], || {
            let provider = configured_rclone_provider().map_err(|error| {
                super::error::CloudCommandError::new(
                    super::error::CloudCommandErrorCode::InvalidConfig,
                    error,
                )
            })?;
            provider.delete_dir_recursive(&path, cancel_token.as_deref())
        })
    });
    let result = map_spawn_result(task.await, "cloud delete dir task failed").map(|_| {
        invalidate_cloud_dir_listing_cache_for_write_paths(&[path_for_invalidate]);
    });
    let elapsed_ms = started.elapsed().as_millis() as u64;
    match &result {
        Ok(()) => debug!(
            op = "cloud_write_delete_dir_recursive",
            path = %path_for_log,
            elapsed_ms,
            "cloud command timing"
        ),
        Err(error) => debug!(
            op = "cloud_write_delete_dir_recursive",
            path = %path_for_log,
            elapsed_ms,
            error = %error,
            "cloud command failed"
        ),
    }
    result
}

pub(super) async fn delete_cloud_dir_empty_impl(
    path: String,
    cancel_state: CancelState,
    progress_event: Option<String>,
) -> CloudCommandResult<()> {
    let started = Instant::now();
    let path = parse_cloud_path_arg(path)?;
    let path_for_invalidate = path.clone();
    let path_for_log = path.clone();
    let remote = path.remote().to_string();
    let cancel_guard = register_cloud_cancel(&cancel_state, &progress_event)?;
    let cancel_token = cancel_guard.as_ref().map(|guard| guard.token());
    let task = tauri::async_runtime::spawn_blocking(move || {
        with_cloud_remote_permits(vec![remote], || {
            let provider = configured_rclone_provider().map_err(|error| {
                super::error::CloudCommandError::new(
                    super::error::CloudCommandErrorCode::InvalidConfig,
                    error,
                )
            })?;
            provider.delete_dir_empty(&path, cancel_token.as_deref())
        })
    });
    let result = map_spawn_result(task.await, "cloud rmdir task failed").map(|_| {
        invalidate_cloud_dir_listing_cache_for_write_paths(&[path_for_invalidate]);
    });
    let elapsed_ms = started.elapsed().as_millis() as u64;
    match &result {
        Ok(()) => debug!(
            op = "cloud_write_delete_dir_empty",
            path = %path_for_log,
            elapsed_ms,
            "cloud command timing"
        ),
        Err(error) => debug!(
            op = "cloud_write_delete_dir_empty",
            path = %path_for_log,
            elapsed_ms,
            error = %error,
            "cloud command failed"
        ),
    }
    result
}

pub(super) async fn move_cloud_entry_impl(
    src: String,
    dst: String,
    overwrite: bool,
    prechecked: bool,
    cancel_state: CancelState,
    progress_event: Option<String>,
) -> CloudCommandResult<()> {
    let started = Instant::now();
    let src = parse_cloud_path_arg(src)?;
    let dst = parse_cloud_path_arg(dst)?;
    let src_for_log = src.clone();
    let dst_for_log = dst.clone();
    let invalidate_paths = vec![src.clone(), dst.clone()];
    let remotes = vec![src.remote().to_string(), dst.remote().to_string()];
    let cancel_guard = register_cloud_cancel(&cancel_state, &progress_event)?;
    let cancel_token = cancel_guard.as_ref().map(|guard| guard.token());
    let task = tauri::async_runtime::spawn_blocking(move || {
        with_cloud_remote_permits(remotes, || {
            let provider = configured_rclone_provider().map_err(|error| {
                super::error::CloudCommandError::new(
                    super::error::CloudCommandErrorCode::InvalidConfig,
                    error,
                )
            })?;
            provider.move_entry(&src, &dst, overwrite, prechecked, cancel_token.as_deref())
        })
    });
    let result = map_spawn_result(task.await, "cloud move task failed").map(|_| {
        invalidate_cloud_dir_listing_cache_for_write_paths(&invalidate_paths);
    });
    let elapsed_ms = started.elapsed().as_millis() as u64;
    match &result {
        Ok(()) => debug!(
            op = "cloud_write_move",
            src = %src_for_log,
            dst = %dst_for_log,
            overwrite,
            prechecked,
            elapsed_ms,
            "cloud command timing"
        ),
        Err(error) => debug!(
            op = "cloud_write_move",
            src = %src_for_log,
            dst = %dst_for_log,
            overwrite,
            prechecked,
            elapsed_ms,
            error = %error,
            "cloud command failed"
        ),
    }
    result
}

pub(super) async fn copy_cloud_entry_impl(
    src: String,
    dst: String,
    overwrite: bool,
    prechecked: bool,
    cancel_state: CancelState,
    progress_event: Option<String>,
) -> CloudCommandResult<()> {
    let started = Instant::now();
    let src = parse_cloud_path_arg(src)?;
    let dst = parse_cloud_path_arg(dst)?;
    let src_for_log = src.clone();
    let dst_for_log = dst.clone();
    let invalidate_paths = vec![dst.clone()];
    let remotes = vec![src.remote().to_string(), dst.remote().to_string()];
    let cancel_guard = register_cloud_cancel(&cancel_state, &progress_event)?;
    let cancel_token = cancel_guard.as_ref().map(|guard| guard.token());
    let task = tauri::async_runtime::spawn_blocking(move || {
        with_cloud_remote_permits(remotes, || {
            let provider = configured_rclone_provider().map_err(|error| {
                super::error::CloudCommandError::new(
                    super::error::CloudCommandErrorCode::InvalidConfig,
                    error,
                )
            })?;
            provider.copy_entry(&src, &dst, overwrite, prechecked, cancel_token.as_deref())
        })
    });
    let result = map_spawn_result(task.await, "cloud copy task failed").map(|_| {
        invalidate_cloud_dir_listing_cache_for_write_paths(&invalidate_paths);
    });
    let elapsed_ms = started.elapsed().as_millis() as u64;
    match &result {
        Ok(()) => debug!(
            op = "cloud_write_copy",
            src = %src_for_log,
            dst = %dst_for_log,
            overwrite,
            prechecked,
            elapsed_ms,
            "cloud command timing"
        ),
        Err(error) => debug!(
            op = "cloud_write_copy",
            src = %src_for_log,
            dst = %dst_for_log,
            overwrite,
            prechecked,
            elapsed_ms,
            error = %error,
            "cloud command failed"
        ),
    }
    result
}
