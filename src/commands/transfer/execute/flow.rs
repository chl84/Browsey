use super::*;

pub(super) fn execute_mixed_entry_to_blocking_with_cli(
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

pub(super) fn execute_mixed_entries_blocking_with_cli(
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
                progress::build_local_to_cloud_batch_progress_plan(&sources)?
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
                let target = local_to_cloud_target_path(&dest_dir, &src)?;
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
                    progress::execute_local_to_cloud_file_transfer_with_aggregate_progress(
                        cli,
                        op,
                        &src,
                        &target,
                        progress::AggregateTransferProgress {
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
                progress::build_cloud_to_local_batch_progress_plan(cli, &sources)?
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
                let target = cloud_to_local_target_path(&dest_dir, &src)?;
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
                    progress::execute_cloud_to_local_file_transfer_with_aggregate_progress(
                        cli,
                        op,
                        &src,
                        &target,
                        progress::AggregateTransferProgress {
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

fn local_to_cloud_target_path(
    dest_dir: &CloudPath,
    src: &std::path::Path,
) -> TransferResult<CloudPath> {
    let leaf = local_leaf_name(src)?;
    dest_dir.child_path(leaf).map_err(|e| {
        transfer_err(
            TransferErrorCode::InvalidPath,
            format!("Invalid cloud target path: {e}"),
        )
    })
}

fn cloud_to_local_target_path(
    dest_dir: &std::path::Path,
    src: &CloudPath,
) -> TransferResult<std::path::PathBuf> {
    let leaf = src.leaf_name().map_err(|e| {
        transfer_err(
            TransferErrorCode::InvalidPath,
            format!("Invalid cloud source path: {e}"),
        )
    })?;
    Ok(dest_dir.join(leaf))
}
