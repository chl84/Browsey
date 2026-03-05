use super::*;

pub(super) struct CloudToLocalBatchProgressPlan {
    pub(super) total_bytes: u64,
    pub(super) file_sizes: Vec<u64>,
}

pub(super) struct LocalToCloudBatchProgressPlan {
    pub(super) total_bytes: u64,
    pub(super) file_sizes: Vec<u64>,
}

pub(super) struct AggregateTransferProgress<'a> {
    pub(super) cancel: Option<&'a AtomicBool>,
    pub(super) progress: &'a TransferProgressContext,
    pub(super) completed_before: u64,
    pub(super) total_bytes: u64,
    pub(super) file_size: u64,
}

pub(super) fn try_execute_cloud_to_local_file_transfer_with_progress(
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

pub(super) fn try_execute_local_to_cloud_file_transfer_with_progress(
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

pub(super) fn build_cloud_to_local_batch_progress_plan(
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

pub(super) fn build_local_to_cloud_batch_progress_plan(
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

pub(super) fn execute_cloud_to_local_file_transfer_with_aggregate_progress(
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

pub(super) fn execute_local_to_cloud_file_transfer_with_aggregate_progress(
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
