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

    tracing::info!(
        op = if op == MixedTransferOp::Copy {
            "copy"
        } else {
            "move"
        },
        backend = "rclone-provider-download",
        kind = "file",
        staging = "direct",
        "transfer dispatch"
    );
    let total = entry.size.unwrap_or(0);
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
            #[cfg(feature = "native-test")]
            crate::native_test::probes::checkpoint(
                &src_path.to_string(), &dst_path.to_string_lossy(), "finalize", total,
                || transfer_cancelled(cancel),
            );
            if transfer_cancelled(cancel) {
                return Err(transfer_err(TransferErrorCode::Cancelled,
                    "Move cancelled after download; downloaded output retained and cloud source not removed"));
            }
            provider
                .delete_file(src_path, cancel)
                .map_err(map_cloud_error_to_transfer)
        }
    }
    .map(|_| {
        emit_transfer_progress(progress, total, total, false);
        #[cfg(feature = "native-test")]
        crate::native_test::probes::checkpoint(
            &src_path.to_string(),
            &dst_path.to_string_lossy(),
            "finalize",
            total,
            || transfer_cancelled(cancel),
        );
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
    options: MixedTransferWriteOptions,
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
    let source_snapshot = (op == MixedTransferOp::Move)
        .then(|| snapshot_mixed_move_source(src_path))
        .transpose()?;
    let on_progress = |bytes, total| emit_transfer_progress(progress, bytes, total, false);
    tracing::info!(
        op = if op == MixedTransferOp::Copy {
            "copy"
        } else {
            "move"
        },
        backend = "rclone-provider-upload",
        kind = "file",
        staging = "direct",
        "transfer dispatch"
    );
    let upload = if op == MixedTransferOp::Copy && !options.overwrite {
        provider.upload_new_file_with_progress(
            src_path,
            dst_path,
            &progress.event_name,
            cancel,
            on_progress,
        )
    } else {
        provider.upload_file_with_progress(
            src_path,
            dst_path,
            &progress.event_name,
            cancel,
            on_progress,
        )
    };
    let result = upload.map_err(map_cloud_error_to_transfer).and_then(|_| {
        emit_transfer_progress(progress, total, total, false);
        #[cfg(feature = "native-test")]
        crate::native_test::probes::checkpoint(
            &src_path.to_string_lossy(),
            &dst_path.to_string(),
            "finalize",
            total,
            || transfer_cancelled(cancel),
        );
        if op == MixedTransferOp::Move {
            remove_local_source_after_mixed_file_move(
                src_path,
                source_snapshot.as_ref().expect("move source snapshot"),
                cancel,
            )?;
        }
        emit_transfer_progress(progress, total, total, true);
        Ok(())
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
        let Some(size) = entry.size else {
            return Ok(None);
        };
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
        #[cfg(feature = "native-test")]
        crate::native_test::probes::checkpoint(
            &src.to_string(),
            &dst.to_string_lossy(),
            "finalize",
            file_size,
            || transfer_cancelled(cancel),
        );
        if transfer_cancelled(cancel) {
            return Err(transfer_err(TransferErrorCode::Cancelled,
                "Move cancelled after download; downloaded output retained and cloud source not removed"));
        }
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
    options: MixedTransferWriteOptions,
) -> TransferResult<()> {
    let AggregateTransferProgress {
        cancel,
        progress,
        completed_before,
        total_bytes,
        file_size,
    } = aggregate;
    let provider = mixed_cloud_provider_for_cli(cli);
    let source_snapshot = (op == MixedTransferOp::Move)
        .then(|| snapshot_mixed_move_source(src))
        .transpose()?;
    let on_progress = |bytes: u64, _| {
        let aggregate = completed_before.saturating_add(bytes.min(file_size));
        emit_transfer_progress(progress, aggregate, total_bytes, false);
    };
    let upload = if op == MixedTransferOp::Copy && !options.overwrite {
        provider.upload_new_file_with_progress(src, dst, &progress.event_name, cancel, on_progress)
    } else {
        provider.upload_file_with_progress(src, dst, &progress.event_name, cancel, on_progress)
    };
    upload.map_err(map_cloud_error_to_transfer)?;
    if op == MixedTransferOp::Move {
        #[cfg(feature = "native-test")]
        crate::native_test::probes::checkpoint(
            &src.to_string_lossy(),
            &dst.to_string(),
            "finalize",
            file_size,
            || transfer_cancelled(cancel),
        );
        remove_local_source_after_mixed_file_move(
            src,
            source_snapshot.as_ref().expect("move source snapshot"),
            cancel,
        )?;
    }
    emit_transfer_progress(
        progress,
        completed_before.saturating_add(file_size),
        total_bytes,
        false,
    );
    Ok(())
}
