use super::{
    error::{map_rclone_error_for_paths, map_rclone_error_for_remote},
    logging::{classify_rc_fallback_reason, log_backend_selected},
    CloudCommandError, CloudCommandErrorCode, CloudCommandResult, CloudPath, CloudProvider,
    RcloneCliError, RcloneCloudProvider, RcloneCommandSpec, RcloneSubcommand,
};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use tracing::info;

impl RcloneCloudProvider {
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
                    info!(
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
                    info!(
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
                    info!(
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
                    info!(
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
                    info!(
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
                    info!(
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
