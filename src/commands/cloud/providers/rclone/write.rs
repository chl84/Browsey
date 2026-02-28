use super::{
    error::{map_rclone_error_for_paths, map_rclone_error_for_remote},
    logging::{classify_rc_fallback_reason, log_backend_selected},
    CloudCommandError, CloudCommandErrorCode, CloudCommandResult, CloudPath, CloudProvider,
    RcloneCliError, RcloneCloudProvider, RcloneCommandSpec, RcloneSubcommand,
};
use crate::commands::cloud::rclone_rc::{
    RcCopyFileFromLocalProgressSpec, RcCopyFileToLocalProgressSpec,
};
use serde_json::Value;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use tracing::debug;

impl RcloneCloudProvider {
    pub(super) fn upload_file_with_progress_impl<F>(
        &self,
        local_src: &Path,
        dst: &CloudPath,
        progress_group: &str,
        cancel: Option<&AtomicBool>,
        mut on_progress: F,
    ) -> CloudCommandResult<()>
    where
        F: FnMut(u64, u64),
    {
        self.ensure_runtime_ready()?;
        if is_cancelled(cancel) {
            return Err(cloud_write_cancelled_error());
        }

        let Some(local_parent) = local_src.parent() else {
            return Err(CloudCommandError::new(
                CloudCommandErrorCode::InvalidPath,
                format!(
                    "Local source must include a parent directory: {}",
                    local_src.display()
                ),
            ));
        };
        let Some(local_name) = local_src.file_name().and_then(|name| name.to_str()) else {
            return Err(CloudCommandError::new(
                CloudCommandErrorCode::InvalidPath,
                format!(
                    "Local source must include a valid file name: {}",
                    local_src.display()
                ),
            ));
        };

        let mut fell_back_from_rc = false;
        let mut fallback_reason: Option<&'static str> = None;
        if self.rc.is_write_enabled() {
            let local_parent_str = local_parent.to_string_lossy().to_string();
            let dst_fs = format!("{}:", dst.remote());
            match self.rc.operations_copyfile_from_local_with_progress(
                RcCopyFileFromLocalProgressSpec {
                    src_dir: &local_parent_str,
                    src_remote: local_name,
                    dst_fs: &dst_fs,
                    dst_remote: dst.rel_path(),
                    group: progress_group,
                    cancel_token: cancel,
                },
                |stats| {
                    if let Some((bytes, total)) = rc_stats_progress(&stats) {
                        on_progress(bytes, total);
                    }
                },
            ) {
                Ok(_) => {
                    let _ = self.rc.core_stats_delete(progress_group);
                    return Ok(());
                }
                Err(RcloneCliError::Cancelled { .. }) => return Err(cloud_write_cancelled_error()),
                Err(error) if !should_fallback_to_cli_after_rc_error(&error) => {
                    let _ = self.rc.core_stats_delete(progress_group);
                    return Err(map_rclone_error_for_remote(dst.remote(), error));
                }
                Err(error) => {
                    let _ = self.rc.core_stats_delete(progress_group);
                    fell_back_from_rc = true;
                    fallback_reason = Some(classify_rc_fallback_reason(&error));
                    debug!(
                        src = %local_src.display(),
                        dst = %dst,
                        error = %error,
                        "rclone rc upload failed; falling back to CLI copyto"
                    );
                }
            }
        }

        self.cli
            .run_capture_text_with_cancel(
                RcloneCommandSpec::new(RcloneSubcommand::CopyTo)
                    .arg(local_src.as_os_str())
                    .arg(dst.to_rclone_remote_spec()),
                cancel,
            )
            .map_err(|error| map_rclone_error_for_remote(dst.remote(), error))?;
        log_backend_selected(
            "cloud_upload_file_upload",
            "cli",
            fell_back_from_rc,
            if fell_back_from_rc {
                fallback_reason
            } else if cancel.is_some() {
                Some("cancelable_cli")
            } else {
                None
            },
        );
        Ok(())
    }

    pub(super) fn download_file_with_progress_impl<F>(
        &self,
        src: &CloudPath,
        local_dest: &Path,
        progress_group: &str,
        cancel: Option<&AtomicBool>,
        mut on_progress: F,
    ) -> CloudCommandResult<()>
    where
        F: FnMut(u64, u64),
    {
        self.ensure_runtime_ready()?;
        if is_cancelled(cancel) {
            return Err(cloud_write_cancelled_error());
        }

        let Some(local_parent) = local_dest.parent() else {
            return Err(CloudCommandError::new(
                CloudCommandErrorCode::InvalidPath,
                format!(
                    "Local destination must include a parent directory: {}",
                    local_dest.display()
                ),
            ));
        };
        let Some(local_name) = local_dest.file_name().and_then(|name| name.to_str()) else {
            return Err(CloudCommandError::new(
                CloudCommandErrorCode::InvalidPath,
                format!(
                    "Local destination must include a valid file name: {}",
                    local_dest.display()
                ),
            ));
        };

        let mut fell_back_from_rc = false;
        let mut fallback_reason: Option<&'static str> = None;
        if self.rc.is_write_enabled() {
            let src_fs = format!("{}:", src.remote());
            let local_parent_str = local_parent.to_string_lossy().to_string();
            match self.rc.operations_copyfile_to_local_with_progress(
                RcCopyFileToLocalProgressSpec {
                    src_fs: &src_fs,
                    src_remote: src.rel_path(),
                    dst_dir: &local_parent_str,
                    dst_remote: local_name,
                    group: progress_group,
                    cancel_token: cancel,
                },
                |stats| {
                    if let Some((bytes, total)) = rc_stats_progress(&stats) {
                        on_progress(bytes, total);
                    }
                },
            ) {
                Ok(_) => {
                    let _ = self.rc.core_stats_delete(progress_group);
                    return Ok(());
                }
                Err(RcloneCliError::Cancelled { .. }) => return Err(cloud_write_cancelled_error()),
                Err(error) if !should_fallback_to_cli_after_rc_error(&error) => {
                    let _ = self.rc.core_stats_delete(progress_group);
                    return Err(map_rclone_error_for_remote(src.remote(), error));
                }
                Err(error) => {
                    let _ = self.rc.core_stats_delete(progress_group);
                    fell_back_from_rc = true;
                    fallback_reason = Some(classify_rc_fallback_reason(&error));
                    debug!(
                        src = %src,
                        dst = %local_dest.display(),
                        error = %error,
                        "rclone rc download failed; falling back to CLI copyto"
                    );
                }
            }
        }

        self.download_file_impl(src, local_dest, cancel)?;
        log_backend_selected(
            "cloud_open_file_download",
            "cli",
            fell_back_from_rc,
            if fell_back_from_rc {
                fallback_reason
            } else if cancel.is_some() {
                Some("cancelable_cli")
            } else {
                None
            },
        );
        Ok(())
    }

    pub(super) fn download_file_impl(
        &self,
        src: &CloudPath,
        local_dest: &Path,
        cancel: Option<&AtomicBool>,
    ) -> CloudCommandResult<()> {
        self.ensure_runtime_ready()?;
        if is_cancelled(cancel) {
            return Err(cloud_write_cancelled_error());
        }
        self.cli
            .run_capture_text_with_cancel(
                RcloneCommandSpec::new(RcloneSubcommand::CopyTo)
                    .arg(src.to_rclone_remote_spec())
                    .arg(local_dest.as_os_str()),
                cancel,
            )
            .map_err(|error| map_rclone_error_for_remote(src.remote(), error))?;
        Ok(())
    }

    pub(super) fn mkdir_impl(
        &self,
        path: &CloudPath,
        cancel: Option<&AtomicBool>,
    ) -> CloudCommandResult<()> {
        self.ensure_runtime_ready()?;
        if is_cancelled(cancel) {
            return Err(cloud_write_cancelled_error());
        }
        let mut fell_back_from_rc = false;
        let mut fallback_reason: Option<&'static str> = None;
        if self.rc.is_write_enabled() && cancel.is_none() {
            let fs_spec = format!("{}:", path.remote());
            match self.rc.operations_mkdir(&fs_spec, path.rel_path()) {
                Ok(_) => {
                    log_backend_selected("cloud_write_mkdir", "rc", false, None);
                    return Ok(());
                }
                Err(error) => {
                    fell_back_from_rc = true;
                    fallback_reason = Some(classify_rc_fallback_reason(&error));
                    debug!(
                        path = %path,
                        error = %error,
                        "rclone rc mkdir failed; falling back to CLI mkdir"
                    );
                }
            }
        }
        self.cli
            .run_capture_text_with_cancel(
                RcloneCommandSpec::new(RcloneSubcommand::Mkdir).arg(path.to_rclone_remote_spec()),
                cancel,
            )
            .map_err(|error| map_rclone_error_for_remote(path.remote(), error))?;
        log_backend_selected(
            "cloud_write_mkdir",
            "cli",
            fell_back_from_rc,
            if fell_back_from_rc {
                fallback_reason
            } else if cancel.is_some() {
                Some("cancelable_cli")
            } else {
                None
            },
        );
        Ok(())
    }

    pub(super) fn delete_file_impl(
        &self,
        path: &CloudPath,
        cancel: Option<&AtomicBool>,
    ) -> CloudCommandResult<()> {
        self.ensure_runtime_ready()?;
        if is_cancelled(cancel) {
            return Err(cloud_write_cancelled_error());
        }
        let mut fell_back_from_rc = false;
        let mut fallback_reason: Option<&'static str> = None;
        if self.rc.is_write_enabled() {
            let fs_spec = format!("{}:", path.remote());
            match self
                .rc
                .operations_deletefile(&fs_spec, path.rel_path(), cancel)
            {
                Ok(_) => {
                    log_backend_selected("cloud_write_delete_file", "rc", false, None);
                    return Ok(());
                }
                Err(RcloneCliError::Cancelled { .. }) => return Err(cloud_write_cancelled_error()),
                Err(error) if !should_fallback_to_cli_after_rc_error(&error) => {
                    return Err(map_rclone_error_for_remote(path.remote(), error));
                }
                Err(error) => {
                    fell_back_from_rc = true;
                    fallback_reason = Some(classify_rc_fallback_reason(&error));
                    debug!(
                        path = %path,
                        error = %error,
                        "rclone rc deletefile failed; falling back to CLI deletefile"
                    );
                }
            }
        }
        self.cli
            .run_capture_text_with_cancel(
                RcloneCommandSpec::new(RcloneSubcommand::DeleteFile)
                    .arg(path.to_rclone_remote_spec()),
                cancel,
            )
            .map_err(|error| map_rclone_error_for_remote(path.remote(), error))?;
        log_backend_selected(
            "cloud_write_delete_file",
            "cli",
            fell_back_from_rc,
            if fell_back_from_rc {
                fallback_reason
            } else if cancel.is_some() {
                Some("cancelable_cli")
            } else {
                None
            },
        );
        Ok(())
    }

    pub(super) fn delete_dir_recursive_impl(
        &self,
        path: &CloudPath,
        cancel: Option<&AtomicBool>,
    ) -> CloudCommandResult<()> {
        self.ensure_runtime_ready()?;
        if is_cancelled(cancel) {
            return Err(cloud_write_cancelled_error());
        }
        let mut fell_back_from_rc = false;
        let mut fallback_reason: Option<&'static str> = None;
        if self.rc.is_write_enabled() && cancel.is_none() {
            let fs_spec = format!("{}:", path.remote());
            match self.rc.operations_purge(&fs_spec, path.rel_path()) {
                Ok(_) => {
                    log_backend_selected("cloud_write_delete_dir_recursive", "rc", false, None);
                    return Ok(());
                }
                Err(error) => {
                    fell_back_from_rc = true;
                    fallback_reason = Some(classify_rc_fallback_reason(&error));
                    debug!(
                        path = %path,
                        error = %error,
                        "rclone rc purge failed; falling back to CLI purge"
                    );
                }
            }
        }
        self.cli
            .run_capture_text_with_cancel(
                RcloneCommandSpec::new(RcloneSubcommand::Purge).arg(path.to_rclone_remote_spec()),
                cancel,
            )
            .map_err(|error| map_rclone_error_for_remote(path.remote(), error))?;
        log_backend_selected(
            "cloud_write_delete_dir_recursive",
            "cli",
            fell_back_from_rc,
            if fell_back_from_rc {
                fallback_reason
            } else if cancel.is_some() {
                Some("cancelable_cli")
            } else {
                None
            },
        );
        Ok(())
    }

    pub(super) fn delete_dir_empty_impl(
        &self,
        path: &CloudPath,
        cancel: Option<&AtomicBool>,
    ) -> CloudCommandResult<()> {
        self.ensure_runtime_ready()?;
        if is_cancelled(cancel) {
            return Err(cloud_write_cancelled_error());
        }
        let mut fell_back_from_rc = false;
        let mut fallback_reason: Option<&'static str> = None;
        if self.rc.is_write_enabled() && cancel.is_none() {
            let fs_spec = format!("{}:", path.remote());
            match self.rc.operations_rmdir(&fs_spec, path.rel_path()) {
                Ok(_) => {
                    log_backend_selected("cloud_write_delete_dir_empty", "rc", false, None);
                    return Ok(());
                }
                Err(error) => {
                    fell_back_from_rc = true;
                    fallback_reason = Some(classify_rc_fallback_reason(&error));
                    debug!(
                        path = %path,
                        error = %error,
                        "rclone rc rmdir failed; falling back to CLI rmdir"
                    );
                }
            }
        }
        self.cli
            .run_capture_text_with_cancel(
                RcloneCommandSpec::new(RcloneSubcommand::Rmdir).arg(path.to_rclone_remote_spec()),
                cancel,
            )
            .map_err(|error| map_rclone_error_for_remote(path.remote(), error))?;
        log_backend_selected(
            "cloud_write_delete_dir_empty",
            "cli",
            fell_back_from_rc,
            if fell_back_from_rc {
                fallback_reason
            } else if cancel.is_some() {
                Some("cancelable_cli")
            } else {
                None
            },
        );
        Ok(())
    }

    pub(super) fn move_entry_impl(
        &self,
        src: &CloudPath,
        dst: &CloudPath,
        overwrite: bool,
        prechecked: bool,
        cancel: Option<&AtomicBool>,
    ) -> CloudCommandResult<()> {
        self.ensure_runtime_ready()?;
        if is_cancelled(cancel) {
            return Err(cloud_write_cancelled_error());
        }
        if !prechecked {
            ensure_destination_overwrite_policy(self, src, dst, overwrite)?;
        }
        let mut fell_back_from_rc = false;
        let mut fallback_reason: Option<&'static str> = None;
        if self.rc.is_write_enabled() && cancel.is_none() {
            let src_fs = format!("{}:", src.remote());
            let dst_fs = format!("{}:", dst.remote());
            match self
                .rc
                .operations_movefile(&src_fs, src.rel_path(), &dst_fs, dst.rel_path())
            {
                Ok(_) => {
                    log_backend_selected("cloud_write_move", "rc", false, None);
                    return Ok(());
                }
                Err(error) => {
                    fell_back_from_rc = true;
                    fallback_reason = Some(classify_rc_fallback_reason(&error));
                    debug!(
                        src = %src,
                        dst = %dst,
                        error = %error,
                        "rclone rc movefile failed; falling back to CLI moveto"
                    );
                }
            }
        }
        self.cli
            .run_capture_text_with_cancel(
                RcloneCommandSpec::new(RcloneSubcommand::MoveTo)
                    .arg(src.to_rclone_remote_spec())
                    .arg(dst.to_rclone_remote_spec()),
                cancel,
            )
            .map_err(|error| map_rclone_error_for_paths(&[src, dst], error))?;
        log_backend_selected(
            "cloud_write_move",
            "cli",
            fell_back_from_rc,
            if fell_back_from_rc {
                fallback_reason
            } else if cancel.is_some() {
                Some("cancelable_cli")
            } else {
                None
            },
        );
        Ok(())
    }

    pub(super) fn copy_entry_impl(
        &self,
        src: &CloudPath,
        dst: &CloudPath,
        overwrite: bool,
        prechecked: bool,
        cancel: Option<&AtomicBool>,
    ) -> CloudCommandResult<()> {
        self.ensure_runtime_ready()?;
        if is_cancelled(cancel) {
            return Err(cloud_write_cancelled_error());
        }
        if !prechecked {
            ensure_destination_overwrite_policy(self, src, dst, overwrite)?;
        }
        let mut fell_back_from_rc = false;
        let mut fallback_reason: Option<&'static str> = None;
        if self.rc.is_write_enabled() {
            let src_fs = format!("{}:", src.remote());
            let dst_fs = format!("{}:", dst.remote());
            match self.rc.operations_copyfile(
                &src_fs,
                src.rel_path(),
                &dst_fs,
                dst.rel_path(),
                cancel,
            ) {
                Ok(_) => {
                    log_backend_selected("cloud_write_copy", "rc", false, None);
                    return Ok(());
                }
                Err(RcloneCliError::Cancelled { .. }) => return Err(cloud_write_cancelled_error()),
                Err(error) if !should_fallback_to_cli_after_rc_error(&error) => {
                    return Err(map_rclone_error_for_paths(&[src, dst], error));
                }
                Err(error) => {
                    fell_back_from_rc = true;
                    fallback_reason = Some(classify_rc_fallback_reason(&error));
                    debug!(
                        src = %src,
                        dst = %dst,
                        error = %error,
                        "rclone rc copyfile failed; falling back to CLI copyto"
                    );
                }
            }
        }
        self.cli
            .run_capture_text_with_cancel(
                RcloneCommandSpec::new(RcloneSubcommand::CopyTo)
                    .arg(src.to_rclone_remote_spec())
                    .arg(dst.to_rclone_remote_spec()),
                cancel,
            )
            .map_err(|error| map_rclone_error_for_paths(&[src, dst], error))?;
        log_backend_selected(
            "cloud_write_copy",
            "cli",
            fell_back_from_rc,
            if fell_back_from_rc {
                fallback_reason
            } else if cancel.is_some() {
                Some("cancelable_cli")
            } else {
                None
            },
        );
        Ok(())
    }
}

fn rc_stats_progress(stats: &Value) -> Option<(u64, u64)> {
    let bytes = stats.get("bytes").and_then(Value::as_u64)?;
    let total = stats
        .get("totalBytes")
        .and_then(Value::as_u64)
        .or_else(|| {
            stats
                .get("transferring")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(|item| item.get("size"))
                .and_then(Value::as_u64)
        })?;
    Some((bytes.min(total), total))
}

fn ensure_destination_overwrite_policy(
    provider: &impl CloudProvider,
    src: &CloudPath,
    dst: &CloudPath,
    overwrite: bool,
) -> CloudCommandResult<()> {
    if overwrite || src == dst {
        return Ok(());
    }
    if provider.stat_path(dst)?.is_some() {
        return Err(CloudCommandError::new(
            CloudCommandErrorCode::DestinationExists,
            format!("Destination already exists: {dst}"),
        ));
    }
    Ok(())
}

pub(super) fn should_fallback_to_cli_after_rc_error(error: &RcloneCliError) -> bool {
    !matches!(error, RcloneCliError::AsyncJobStateUnknown { .. })
}

fn is_cancelled(cancel: Option<&AtomicBool>) -> bool {
    cancel
        .map(|token| token.load(std::sync::atomic::Ordering::SeqCst))
        .unwrap_or(false)
}

pub(super) fn cloud_write_cancelled_error() -> CloudCommandError {
    CloudCommandError::new(
        CloudCommandErrorCode::TaskFailed,
        "Cloud operation cancelled",
    )
}
