use super::error::{transfer_err, transfer_err_code as api_err, TransferErrorCode, TransferResult};
use super::logging::{log_mixed_execute_result, log_mixed_single_execute_result};
use super::route::{
    local_leaf_name, mixed_route_hint, validate_mixed_transfer_pair, validate_mixed_transfer_route,
    LocalOrCloudArg, MixedTransferPair, MixedTransferRoute,
};
use super::{MixedTransferOp, MixedTransferWriteOptions};
use crate::commands::cloud;
use crate::commands::cloud::path::CloudPath;
use crate::commands::cloud::provider::CloudProvider;
use crate::commands::cloud::providers::rclone::RcloneCloudProvider;
use crate::commands::cloud::rclone_cli::{
    RcloneCli, RcloneCliError, RcloneCommandSpec, RcloneSubcommand,
};
use crate::commands::cloud::types::{CloudEntryKind, CloudProviderKind};
use crate::runtime_lifecycle;
use crate::tasks::{CancelGuard, CancelState};
use serde::Serialize;
use std::fs;
use std::io::ErrorKind;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Instant;

#[derive(Clone)]
struct TransferProgressContext {
    app: tauri::AppHandle,
    event_name: String,
}

#[derive(Serialize, Clone)]
struct TransferProgressPayload {
    bytes: u64,
    total: u64,
    finished: bool,
}

struct CloudToLocalBatchProgressPlan {
    total_bytes: u64,
    file_sizes: Vec<u64>,
}

struct LocalToCloudBatchProgressPlan {
    total_bytes: u64,
    file_sizes: Vec<u64>,
}

struct RcloneTransferContext<'a> {
    cli: &'a RcloneCli,
    cloud_remote_for_error_mapping: Option<&'a str>,
    cancel: Option<&'a AtomicBool>,
    progress: Option<&'a TransferProgressContext>,
}

struct AggregateTransferProgress<'a> {
    cancel: Option<&'a AtomicBool>,
    progress: &'a TransferProgressContext,
    completed_before: u64,
    total_bytes: u64,
    file_size: u64,
}

pub(super) async fn execute_mixed_entries(
    op: MixedTransferOp,
    sources: Vec<String>,
    dest_dir: String,
    app: tauri::AppHandle,
    options: MixedTransferWriteOptions,
    cancel_state: CancelState,
    progress_event: Option<String>,
) -> TransferResult<Vec<String>> {
    let started = Instant::now();
    let source_count = sources.len();
    let route_hint = mixed_route_hint(&sources, &dest_dir);
    let route = match validate_mixed_transfer_route(sources, dest_dir).await {
        Ok(route) => route,
        Err(err) => {
            let result = Err(err);
            log_mixed_execute_result(op, &result, route_hint, source_count, started);
            return result;
        }
    };
    let cancel_guard = register_mixed_cancel(&cancel_state, &progress_event)?;
    let cancel_token = cancel_guard.as_ref().map(|guard| guard.token());
    let progress = progress_event
        .clone()
        .map(|event_name| TransferProgressContext { app, event_name });
    let task = tauri::async_runtime::spawn_blocking(move || {
        execute_mixed_entries_blocking(op, route, options, cancel_token, progress)
    });
    let result = match task.await {
        Ok(result) => result,
        Err(error) => Err(api_err(
            "task_failed",
            format!("Mixed transfer task failed: {error}"),
        )),
    };
    log_mixed_execute_result(op, &result, route_hint, source_count, started);
    result
}

pub(super) async fn execute_mixed_entry_to(
    op: MixedTransferOp,
    src: String,
    dst: String,
    app: tauri::AppHandle,
    options: MixedTransferWriteOptions,
    cancel_state: CancelState,
    progress_event: Option<String>,
) -> TransferResult<String> {
    let started = Instant::now();
    let route_hint = mixed_route_hint(std::slice::from_ref(&src), &dst);
    let pair = match validate_mixed_transfer_pair(src, dst).await {
        Ok(pair) => pair,
        Err(err) => {
            let result = Err(err);
            log_mixed_single_execute_result(op, &result, route_hint, started);
            return result;
        }
    };
    let cancel_guard = register_mixed_cancel(&cancel_state, &progress_event)?;
    let cancel_token = cancel_guard.as_ref().map(|guard| guard.token());
    let progress = progress_event
        .clone()
        .map(|event_name| TransferProgressContext { app, event_name });
    let task = tauri::async_runtime::spawn_blocking(move || {
        execute_mixed_entry_to_blocking(op, pair, options, cancel_token, progress)
    });
    let result = match task.await {
        Ok(result) => result,
        Err(error) => Err(api_err(
            "task_failed",
            format!("Mixed transfer task failed: {error}"),
        )),
    };
    log_mixed_single_execute_result(op, &result, route_hint, started);
    result
}

fn execute_mixed_entries_blocking(
    op: MixedTransferOp,
    route: MixedTransferRoute,
    options: MixedTransferWriteOptions,
    cancel: Option<Arc<AtomicBool>>,
    progress: Option<TransferProgressContext>,
) -> TransferResult<Vec<String>> {
    let cli = cloud::configured_rclone_cli().map_err(|error| {
        let code = match error.code() {
            cloud::RclonePathErrorCode::InvalidConfig => TransferErrorCode::InvalidConfig,
            cloud::RclonePathErrorCode::DbOpenFailed | cloud::RclonePathErrorCode::DbReadFailed => {
                TransferErrorCode::TaskFailed
            }
        };
        transfer_err(code, format!("{error}"))
    })?;
    execute_mixed_entries_blocking_with_cli(&cli, op, route, options, cancel, progress)
}

fn execute_mixed_entry_to_blocking(
    op: MixedTransferOp,
    pair: MixedTransferPair,
    options: MixedTransferWriteOptions,
    cancel: Option<Arc<AtomicBool>>,
    progress: Option<TransferProgressContext>,
) -> TransferResult<String> {
    let cli = cloud::configured_rclone_cli().map_err(|error| {
        let code = match error.code() {
            cloud::RclonePathErrorCode::InvalidConfig => TransferErrorCode::InvalidConfig,
            cloud::RclonePathErrorCode::DbOpenFailed | cloud::RclonePathErrorCode::DbReadFailed => {
                TransferErrorCode::TaskFailed
            }
        };
        transfer_err(code, format!("{error}"))
    })?;
    execute_mixed_entry_to_blocking_with_cli(&cli, op, pair, options, cancel, progress)
}

fn execute_mixed_entry_to_blocking_with_cli(
    cli: &RcloneCli,
    op: MixedTransferOp,
    pair: MixedTransferPair,
    options: MixedTransferWriteOptions,
    cancel: Option<Arc<AtomicBool>>,
    progress: Option<TransferProgressContext>,
) -> TransferResult<String> {
    if transfer_cancelled(cancel.as_deref()) {
        return Err(transfer_err(
            TransferErrorCode::Cancelled,
            "Transfer cancelled",
        ));
    }
    let MixedTransferPair {
        src,
        dst,
        cloud_remote_for_error_mapping,
    } = pair;
    let invalidate_source = match (&src, op) {
        (LocalOrCloudArg::Cloud(path), MixedTransferOp::Move) => Some(path.clone()),
        _ => None,
    };
    let invalidate_target = match &dst {
        LocalOrCloudArg::Cloud(path) => Some(path.clone()),
        LocalOrCloudArg::Local(_) => None,
    };
    let out = match &dst {
        LocalOrCloudArg::Local(path) => path.to_string_lossy().to_string(),
        LocalOrCloudArg::Cloud(path) => path.to_string(),
    };
    execute_rclone_transfer(
        RcloneTransferContext {
            cli,
            cloud_remote_for_error_mapping: cloud_remote_for_error_mapping.as_deref(),
            cancel: cancel.as_deref(),
            progress: progress.as_ref(),
        },
        op,
        src,
        dst,
        options,
    )?;
    if let Some(path) = invalidate_target.as_ref() {
        cloud::invalidate_cloud_write_paths(std::slice::from_ref(path));
    } else if let Some(path) = invalidate_source.as_ref() {
        cloud::invalidate_cloud_write_paths(std::slice::from_ref(path));
    }
    Ok(out)
}

fn execute_mixed_entries_blocking_with_cli(
    cli: &RcloneCli,
    op: MixedTransferOp,
    route: MixedTransferRoute,
    options: MixedTransferWriteOptions,
    cancel: Option<Arc<AtomicBool>>,
    progress: Option<TransferProgressContext>,
) -> TransferResult<Vec<String>> {
    let mut created = Vec::new();
    let result = match route {
        MixedTransferRoute::LocalToCloud { sources, dest_dir } => {
            let batch_source_count = sources.len();
            let progress_plan = if batch_source_count > 1 && progress.is_some() {
                build_local_to_cloud_batch_progress_plan(&sources)?
            } else {
                None
            };
            let mut completed_bytes = 0_u64;
            for (index, src) in sources.into_iter().enumerate() {
                if transfer_cancelled(cancel.as_deref()) {
                    return Err(transfer_err(
                        TransferErrorCode::Cancelled,
                        "Transfer cancelled",
                    ));
                }
                let leaf = local_leaf_name(&src)?;
                let target = dest_dir.child_path(leaf).map_err(|e| {
                    transfer_err(
                        TransferErrorCode::InvalidPath,
                        format!("Invalid cloud target path: {e}"),
                    )
                })?;
                if let Some(plan) = progress_plan.as_ref() {
                    if !options.overwrite
                        && !options.prechecked
                        && mixed_target_exists(
                            cli,
                            &LocalOrCloudArg::Cloud(target.clone()),
                            Some(dest_dir.remote()),
                            cancel.as_deref(),
                        )?
                    {
                        return Err(api_err(
                            "destination_exists",
                            "A file or folder with the same name already exists",
                        ));
                    }
                    execute_local_to_cloud_file_transfer_with_aggregate_progress(
                        cli,
                        op,
                        &src,
                        &target,
                        AggregateTransferProgress {
                            cancel: cancel.as_deref(),
                            progress: progress.as_ref().expect("progress context for batch plan"),
                            completed_before: completed_bytes,
                            total_bytes: plan.total_bytes,
                            file_size: plan.file_sizes[index],
                        },
                    )?;
                    completed_bytes = completed_bytes.saturating_add(plan.file_sizes[index]);
                } else {
                    execute_rclone_transfer(
                        RcloneTransferContext {
                            cli,
                            cloud_remote_for_error_mapping: Some(dest_dir.remote()),
                            cancel: cancel.as_deref(),
                            progress: if batch_source_count == 1 {
                                progress.as_ref()
                            } else {
                                None
                            },
                        },
                        op,
                        LocalOrCloudArg::Local(src.clone()),
                        LocalOrCloudArg::Cloud(target.clone()),
                        options,
                    )?;
                }
                cloud::invalidate_cloud_write_paths(std::slice::from_ref(&target));
                created.push(target.to_string());
            }
            Ok(())
        }
        MixedTransferRoute::CloudToLocal { sources, dest_dir } => {
            let batch_source_count = sources.len();
            let progress_plan = if batch_source_count > 1 && progress.is_some() {
                build_cloud_to_local_batch_progress_plan(cli, &sources)?
            } else {
                None
            };
            let mut completed_bytes = 0_u64;
            for (index, src) in sources.into_iter().enumerate() {
                if transfer_cancelled(cancel.as_deref()) {
                    return Err(transfer_err(
                        TransferErrorCode::Cancelled,
                        "Transfer cancelled",
                    ));
                }
                let leaf = src.leaf_name().map_err(|e| {
                    transfer_err(
                        TransferErrorCode::InvalidPath,
                        format!("Invalid cloud source path: {e}"),
                    )
                })?;
                let target = dest_dir.join(leaf);
                if let Some(plan) = progress_plan.as_ref() {
                    if !options.overwrite
                        && !options.prechecked
                        && mixed_target_exists(
                            cli,
                            &LocalOrCloudArg::Local(target.clone()),
                            Some(src.remote()),
                            cancel.as_deref(),
                        )?
                    {
                        return Err(api_err(
                            "destination_exists",
                            "A file or folder with the same name already exists",
                        ));
                    }
                    execute_cloud_to_local_file_transfer_with_aggregate_progress(
                        cli,
                        op,
                        &src,
                        &target,
                        AggregateTransferProgress {
                            cancel: cancel.as_deref(),
                            progress: progress.as_ref().expect("progress context for batch plan"),
                            completed_before: completed_bytes,
                            total_bytes: plan.total_bytes,
                            file_size: plan.file_sizes[index],
                        },
                    )?;
                    completed_bytes = completed_bytes.saturating_add(plan.file_sizes[index]);
                } else {
                    execute_rclone_transfer(
                        RcloneTransferContext {
                            cli,
                            cloud_remote_for_error_mapping: Some(src.remote()),
                            cancel: cancel.as_deref(),
                            progress: if batch_source_count == 1 {
                                progress.as_ref()
                            } else {
                                None
                            },
                        },
                        op,
                        LocalOrCloudArg::Cloud(src.clone()),
                        LocalOrCloudArg::Local(target.clone()),
                        options,
                    )?;
                }
                if op == MixedTransferOp::Move {
                    cloud::invalidate_cloud_write_paths(std::slice::from_ref(&src));
                }
                created.push(target.to_string_lossy().to_string());
            }
            Ok(())
        }
    };
    result.map(|_| created)
}

fn execute_rclone_transfer(
    ctx: RcloneTransferContext<'_>,
    op: MixedTransferOp,
    src: LocalOrCloudArg,
    dst: LocalOrCloudArg,
    options: MixedTransferWriteOptions,
) -> TransferResult<()> {
    let RcloneTransferContext {
        cli,
        cloud_remote_for_error_mapping,
        cancel,
        progress,
    } = ctx;
    if transfer_cancelled(cancel) {
        return Err(transfer_err(
            TransferErrorCode::Cancelled,
            "Transfer cancelled",
        ));
    }
    if !options.overwrite
        && !options.prechecked
        && mixed_target_exists(cli, &dst, cloud_remote_for_error_mapping, cancel)?
    {
        return Err(api_err(
            "destination_exists",
            "A file or folder with the same name already exists",
        ));
    }

    if let Some(result) = try_execute_cloud_to_local_file_transfer_with_progress(
        cli, op, &src, &dst, cancel, progress,
    )? {
        return result;
    }

    if let Some(result) = try_execute_local_to_cloud_file_transfer_with_progress(
        cli, op, &src, &dst, cancel, progress,
    )? {
        return result;
    }

    let subcommand = match op {
        MixedTransferOp::Copy => RcloneSubcommand::CopyTo,
        MixedTransferOp::Move => RcloneSubcommand::MoveTo,
    };

    let spec = RcloneCommandSpec::new(subcommand)
        .arg(src.to_os_arg())
        .arg(dst.to_os_arg());

    cli.run_capture_text_with_cancel(spec, cancel)
        .map_err(|error| map_rclone_cli_error(error, cloud_remote_for_error_mapping))?;
    Ok(())
}

fn try_execute_cloud_to_local_file_transfer_with_progress(
    cli: &RcloneCli,
    op: MixedTransferOp,
    src: &LocalOrCloudArg,
    dst: &LocalOrCloudArg,
    cancel: Option<&AtomicBool>,
    progress: Option<&TransferProgressContext>,
) -> TransferResult<Option<TransferResult<()>>> {
    let Some(progress) = progress else {
        return Ok(None);
    };
    let (Some(src_path), Some(dst_path)) = (src.cloud_path(), dst.local_path()) else {
        return Ok(None);
    };
    let provider = mixed_cloud_provider_for_cli(cli);
    let Some(entry) = provider
        .stat_path(src_path)
        .map_err(map_cloud_error_to_transfer)?
    else {
        return Err(transfer_err(
            TransferErrorCode::NotFound,
            "Cloud source was not found",
        ));
    };
    if !matches!(entry.kind, CloudEntryKind::File) {
        return Ok(None);
    }

    let total = entry.size.unwrap_or(1);
    let result = match op {
        MixedTransferOp::Copy => provider
            .download_file_with_progress(
                src_path,
                dst_path,
                &progress.event_name,
                cancel,
                |bytes, total| {
                    emit_transfer_progress(progress, bytes, total, false);
                },
            )
            .map_err(map_cloud_error_to_transfer),
        MixedTransferOp::Move => {
            provider
                .download_file_with_progress(
                    src_path,
                    dst_path,
                    &progress.event_name,
                    cancel,
                    |bytes, total| {
                        emit_transfer_progress(progress, bytes, total, false);
                    },
                )
                .map_err(map_cloud_error_to_transfer)?;
            provider
                .delete_file(src_path, cancel)
                .map_err(map_cloud_error_to_transfer)
        }
    }
    .map(|_| {
        emit_transfer_progress(progress, total, total, true);
    });

    Ok(Some(result))
}

fn try_execute_local_to_cloud_file_transfer_with_progress(
    cli: &RcloneCli,
    op: MixedTransferOp,
    src: &LocalOrCloudArg,
    dst: &LocalOrCloudArg,
    cancel: Option<&AtomicBool>,
    progress: Option<&TransferProgressContext>,
) -> TransferResult<Option<TransferResult<()>>> {
    let Some(progress) = progress else {
        return Ok(None);
    };
    let (Some(src_path), Some(dst_path)) = (src.local_path(), dst.cloud_path()) else {
        return Ok(None);
    };
    let metadata = fs::symlink_metadata(src_path).map_err(|error| {
        transfer_err(
            TransferErrorCode::IoError,
            format!("Failed to read source metadata: {error}"),
        )
    })?;
    if !metadata.is_file() {
        return Ok(None);
    }

    let provider = mixed_cloud_provider_for_cli(cli);
    let total = metadata.len();
    let result = match op {
        MixedTransferOp::Copy => provider
            .upload_file_with_progress(
                src_path,
                dst_path,
                &progress.event_name,
                cancel,
                |bytes, total| {
                    emit_transfer_progress(progress, bytes, total, false);
                },
            )
            .map_err(map_cloud_error_to_transfer),
        MixedTransferOp::Move => {
            provider
                .upload_file_with_progress(
                    src_path,
                    dst_path,
                    &progress.event_name,
                    cancel,
                    |bytes, total| {
                        emit_transfer_progress(progress, bytes, total, false);
                    },
                )
                .map_err(map_cloud_error_to_transfer)?;
            remove_local_source_after_mixed_file_move(src_path)
        }
    }
    .map(|_| {
        emit_transfer_progress(progress, total, total, true);
    });

    Ok(Some(result))
}

fn build_cloud_to_local_batch_progress_plan(
    cli: &RcloneCli,
    sources: &[CloudPath],
) -> TransferResult<Option<CloudToLocalBatchProgressPlan>> {
    let provider = mixed_cloud_provider_for_cli(cli);
    let mut file_sizes = Vec::with_capacity(sources.len());
    let mut total_bytes = 0_u64;
    for src in sources {
        let Some(entry) = provider
            .stat_path(src)
            .map_err(map_cloud_error_to_transfer)?
        else {
            return Err(transfer_err(
                TransferErrorCode::NotFound,
                "Cloud source was not found",
            ));
        };
        if !matches!(entry.kind, CloudEntryKind::File) {
            return Ok(None);
        }
        let size = entry.size.unwrap_or(1);
        file_sizes.push(size);
        total_bytes = total_bytes.saturating_add(size);
    }
    if total_bytes == 0 {
        return Ok(None);
    }
    Ok(Some(CloudToLocalBatchProgressPlan {
        total_bytes,
        file_sizes,
    }))
}

fn build_local_to_cloud_batch_progress_plan(
    sources: &[std::path::PathBuf],
) -> TransferResult<Option<LocalToCloudBatchProgressPlan>> {
    let mut file_sizes = Vec::with_capacity(sources.len());
    let mut total_bytes = 0_u64;
    for src in sources {
        let metadata = fs::symlink_metadata(src).map_err(|error| {
            transfer_err(
                TransferErrorCode::IoError,
                format!("Failed to read source metadata: {error}"),
            )
        })?;
        if !metadata.is_file() {
            return Ok(None);
        }
        let size = metadata.len();
        file_sizes.push(size);
        total_bytes = total_bytes.saturating_add(size);
    }
    if total_bytes == 0 {
        return Ok(None);
    }
    Ok(Some(LocalToCloudBatchProgressPlan {
        total_bytes,
        file_sizes,
    }))
}

fn execute_cloud_to_local_file_transfer_with_aggregate_progress(
    cli: &RcloneCli,
    op: MixedTransferOp,
    src: &CloudPath,
    dst: &std::path::Path,
    aggregate: AggregateTransferProgress<'_>,
) -> TransferResult<()> {
    let AggregateTransferProgress {
        cancel,
        progress,
        completed_before,
        total_bytes,
        file_size,
    } = aggregate;
    let provider = mixed_cloud_provider_for_cli(cli);
    provider
        .download_file_with_progress(src, dst, &progress.event_name, cancel, |bytes, _| {
            let aggregate = completed_before.saturating_add(bytes.min(file_size));
            emit_transfer_progress(progress, aggregate, total_bytes, false);
        })
        .map_err(map_cloud_error_to_transfer)?;
    if op == MixedTransferOp::Move {
        provider
            .delete_file(src, cancel)
            .map_err(map_cloud_error_to_transfer)?;
    }
    emit_transfer_progress(
        progress,
        completed_before.saturating_add(file_size),
        total_bytes,
        false,
    );
    Ok(())
}

fn execute_local_to_cloud_file_transfer_with_aggregate_progress(
    cli: &RcloneCli,
    op: MixedTransferOp,
    src: &std::path::Path,
    dst: &CloudPath,
    aggregate: AggregateTransferProgress<'_>,
) -> TransferResult<()> {
    let AggregateTransferProgress {
        cancel,
        progress,
        completed_before,
        total_bytes,
        file_size,
    } = aggregate;
    let provider = mixed_cloud_provider_for_cli(cli);
    provider
        .upload_file_with_progress(src, dst, &progress.event_name, cancel, |bytes, _| {
            let aggregate = completed_before.saturating_add(bytes.min(file_size));
            emit_transfer_progress(progress, aggregate, total_bytes, false);
        })
        .map_err(map_cloud_error_to_transfer)?;
    if op == MixedTransferOp::Move {
        remove_local_source_after_mixed_file_move(src)?;
    }
    emit_transfer_progress(
        progress,
        completed_before.saturating_add(file_size),
        total_bytes,
        false,
    );
    Ok(())
}

fn mixed_target_exists(
    cli: &RcloneCli,
    dst: &LocalOrCloudArg,
    cloud_remote_for_error_mapping: Option<&str>,
    cancel: Option<&AtomicBool>,
) -> TransferResult<bool> {
    if transfer_cancelled(cancel) {
        return Err(transfer_err(
            TransferErrorCode::Cancelled,
            "Transfer cancelled",
        ));
    }
    if let Some(path) = dst.local_path() {
        return match fs::symlink_metadata(path) {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
            Err(e) => Err(api_err(
                "io_error",
                format!("Failed to read destination metadata: {e}"),
            )),
        };
    }

    let Some(cloud_path) = dst.cloud_path() else {
        return Ok(false);
    };
    let spec = RcloneCommandSpec::new(RcloneSubcommand::LsJson)
        .arg("--stat")
        .arg(cloud_path.to_rclone_remote_spec());
    match cli.run_capture_text_with_cancel(spec, cancel) {
        Ok(_) => Ok(true),
        Err(RcloneCliError::NonZero { stderr, stdout, .. })
            if is_rclone_not_found_text(&stderr, &stdout) =>
        {
            Ok(false)
        }
        Err(error) => Err(map_rclone_cli_error(error, cloud_remote_for_error_mapping)),
    }
}

fn map_rclone_cli_error(
    error: RcloneCliError,
    cloud_remote: Option<&str>,
) -> super::error::TransferError {
    match error {
        RcloneCliError::Io(io) if io.kind() == std::io::ErrorKind::NotFound => {
            transfer_err(TransferErrorCode::BinaryMissing, "rclone not found in PATH")
        }
        RcloneCliError::Io(io) => transfer_err(TransferErrorCode::NetworkError, format!("Failed to run rclone: {io}")),
        RcloneCliError::Shutdown { .. } => api_err(
            "task_failed",
            "Application is shutting down; transfer was cancelled",
        ),
        RcloneCliError::Cancelled { .. } => transfer_err(TransferErrorCode::Cancelled, "Transfer cancelled"),
        RcloneCliError::AsyncJobStateUnknown {
            operation,
            job_id,
            reason,
            ..
        } => api_err(
            "task_failed",
            format!(
                "Transfer status is unknown after rclone rc {operation} job {job_id}; Browsey did not retry automatically to avoid duplicate operations. Refresh and verify destination state before retrying. Cause: {}",
                reason.trim()
            ),
        ),
        RcloneCliError::Timeout {
            subcommand,
            timeout,
            ..
        } => api_err(
            "timeout",
            format!(
                "rclone {} timed out after {}s",
                subcommand.as_str(),
                timeout.as_secs()
            ),
        ),
        RcloneCliError::NonZero { stderr, stdout, .. } => {
            let msg_ref = if !stderr.trim().is_empty() {
                stderr.as_str()
            } else {
                stdout.as_str()
            };
            let lower = msg_ref.to_ascii_lowercase();
            let not_found = is_rclone_not_found_text(&stderr, &stdout);
            let provider = cloud_remote.and_then(cloud::cloud_provider_kind_for_remote);
            let provider_code = provider_specific_rclone_code(provider, &lower);
            let code = if lower.contains("quota exceeded")
                || lower.contains("rate_limit_exceeded")
                || lower.contains("too many requests")
            {
                "rate_limited"
            } else if lower.contains("unauthorized")
                || lower.contains("invalid_grant")
                || lower.contains("token") && lower.contains("expired")
            {
                "auth_required"
            } else if lower.contains("permission denied") || lower.contains("access denied") {
                "permission_denied"
            } else if lower.contains("already exists")
                || lower.contains("destination exists")
                || lower.contains("file exists")
            {
                "destination_exists"
            } else if not_found {
                "not_found"
            } else if lower.contains("x509") || lower.contains("certificate") {
                "tls_certificate_error"
            } else {
                provider_code.unwrap_or("unknown_error")
            };
            api_err(code, msg_ref.trim())
        }
    }
}

fn register_mixed_cancel(
    cancel_state: &CancelState,
    progress_event: &Option<String>,
) -> TransferResult<Option<CancelGuard>> {
    progress_event
        .as_ref()
        .map(|event| cancel_state.register(event.clone()))
        .transpose()
        .map_err(|error| {
            api_err(
                "task_failed",
                format!("Failed to register cancel token: {error}"),
            )
        })
}

fn transfer_cancelled(cancel: Option<&AtomicBool>) -> bool {
    cancel
        .map(|token| token.load(Ordering::SeqCst))
        .unwrap_or(false)
}

fn provider_specific_rclone_code(
    provider: Option<CloudProviderKind>,
    lower_message: &str,
) -> Option<&'static str> {
    match provider {
        Some(CloudProviderKind::Onedrive) => {
            if lower_message.contains("activitylimitreached") {
                return Some("rate_limited");
            }
            None
        }
        Some(CloudProviderKind::Gdrive) => {
            if lower_message.contains("userratelimitexceeded")
                || lower_message.contains("ratelimitexceeded")
            {
                return Some("rate_limited");
            }
            None
        }
        Some(CloudProviderKind::Nextcloud) | None => None,
    }
}

fn emit_transfer_progress(
    progress: &TransferProgressContext,
    bytes: u64,
    total: u64,
    finished: bool,
) {
    if total == 0 {
        return;
    }
    let _ = runtime_lifecycle::emit_if_running(
        &progress.app,
        &progress.event_name,
        TransferProgressPayload {
            bytes,
            total,
            finished,
        },
    );
}

fn map_cloud_error_to_transfer(
    error: crate::commands::cloud::CloudCommandError,
) -> super::error::TransferError {
    error.into()
}

fn remove_local_source_after_mixed_file_move(path: &std::path::Path) -> TransferResult<()> {
    fs::remove_file(path).map_err(|error| {
        transfer_err(
            TransferErrorCode::IoError,
            format!("Failed to remove moved source file: {error}"),
        )
    })
}

fn mixed_cloud_provider_for_cli(cli: &RcloneCli) -> RcloneCloudProvider {
    RcloneCloudProvider::from_cli(cli.clone())
}

fn is_rclone_not_found_text(stderr: &str, stdout: &str) -> bool {
    let combined = if !stderr.trim().is_empty() {
        stderr
    } else {
        stdout
    };
    let lower = combined.to_lowercase();
    lower.contains("not found")
        || lower.contains("object not found")
        || lower.contains("directory not found")
        || lower.contains("file not found")
        || lower.contains("404")
}

#[cfg(test)]
mod tests;
