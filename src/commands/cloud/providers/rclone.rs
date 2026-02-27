use super::super::{
    error::{CloudCommandError, CloudCommandErrorCode, CloudCommandResult},
    path::CloudPath,
    provider::CloudProvider,
    rclone_cli::{RcloneCli, RcloneCliError, RcloneCommandSpec, RcloneSubcommand},
    rclone_rc::RcloneRcClient,
    types::{CloudCapabilities, CloudEntry, CloudEntryKind, CloudProviderKind, CloudRemote},
};
use chrono::{DateTime, Local};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    env,
    sync::{atomic::AtomicBool, OnceLock},
};
use tracing::{debug, info};

const MIN_RCLONE_VERSION: (u64, u64, u64) = (1, 67, 0);

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub(in crate::commands::cloud) struct RcloneCloudProvider {
    cli: RcloneCli,
    rc: RcloneRcClient,
    #[cfg(test)]
    force_async_unknown_deletefile_for_tests: bool,
    #[cfg(test)]
    force_async_unknown_copyfile_for_tests: bool,
}

static RCLONE_RUNTIME_PROBE: OnceLock<Result<(), CloudCommandError>> = OnceLock::new();
static RCLONE_REMOTE_POLICY: OnceLock<RcloneRemotePolicy> = OnceLock::new();

#[allow(dead_code)]
impl RcloneCloudProvider {
    pub fn new(cli: RcloneCli) -> Self {
        let rc = RcloneRcClient::new(cli.binary().to_os_string());
        Self {
            cli,
            rc,
            #[cfg(test)]
            force_async_unknown_deletefile_for_tests: false,
            #[cfg(test)]
            force_async_unknown_copyfile_for_tests: false,
        }
    }

    pub fn cli(&self) -> &RcloneCli {
        &self.cli
    }

    fn ensure_runtime_ready(&self) -> CloudCommandResult<()> {
        let result = RCLONE_RUNTIME_PROBE.get_or_init(|| probe_rclone_runtime(&self.cli));
        match result {
            Ok(_) => Ok(()),
            Err(error) => Err(error.clone()),
        }
    }

    fn list_remotes_via_rc(&self) -> Result<Vec<CloudRemote>, RcloneCliError> {
        let remotes_value = self.rc.list_remotes()?;
        let remote_ids = parse_listremotes_rc_json(&remotes_value)
            .map_err(|msg| RcloneCliError::Io(std::io::Error::other(msg)))?;
        let config_dump_value = self.rc.config_dump()?;
        let config_dump_text = serde_json::to_string(&config_dump_value).map_err(|error| {
            RcloneCliError::Io(std::io::Error::other(format!(
                "invalid rclone rc config dump payload: {error}"
            )))
        })?;
        let config_map = parse_config_dump_summaries(&config_dump_text).map_err(|msg| {
            RcloneCliError::Io(std::io::Error::other(format!(
                "Invalid rclone rc config dump payload: {msg}"
            )))
        })?;
        Ok(build_cloud_remotes(remote_ids, config_map))
    }

    #[cfg(test)]
    fn with_forced_async_unknown_deletefile_for_tests(mut self) -> Self {
        self.force_async_unknown_deletefile_for_tests = true;
        self
    }

    #[cfg(test)]
    fn with_forced_async_unknown_copyfile_for_tests(mut self) -> Self {
        self.force_async_unknown_copyfile_for_tests = true;
        self
    }

    fn list_dir_via_rc(&self, path: &CloudPath) -> Result<Vec<CloudEntry>, RcloneCliError> {
        let fs_spec = format!("{}:", path.remote());
        let response = self.rc.operations_list(&fs_spec, path.rel_path())?;
        let list = response
            .get("list")
            .ok_or_else(|| {
                RcloneCliError::Io(std::io::Error::other(
                    "Invalid rclone rc operations/list payload: missing `list` field",
                ))
            })?
            .clone();
        let list_json = serde_json::to_string(&list).map_err(|error| {
            RcloneCliError::Io(std::io::Error::other(format!(
                "Invalid rclone rc operations/list payload: {error}"
            )))
        })?;
        let items = parse_lsjson_items(&list_json).map_err(|message| {
            RcloneCliError::Io(std::io::Error::other(format!(
                "Invalid rclone rc operations/list item payload: {message}"
            )))
        })?;
        let mut entries = Vec::with_capacity(items.len());
        for item in items {
            let child_path = path.child_path(&item.name).map_err(|error| {
                RcloneCliError::Io(std::io::Error::other(format!(
                    "Invalid entry name from rclone rc operations/list: {error}"
                )))
            })?;
            entries.push(cloud_entry_from_item(&child_path, item));
        }
        entries.sort_by(|a, b| {
            let rank_a = if matches!(a.kind, CloudEntryKind::Dir) {
                0
            } else {
                1
            };
            let rank_b = if matches!(b.kind, CloudEntryKind::Dir) {
                0
            } else {
                1
            };
            rank_a.cmp(&rank_b).then_with(|| a.name.cmp(&b.name))
        });
        Ok(entries)
    }

    fn stat_path_via_rc(&self, path: &CloudPath) -> Result<Option<CloudEntry>, RcloneCliError> {
        let fs_spec = format!("{}:", path.remote());
        let response = self.rc.operations_stat(&fs_spec, path.rel_path())?;
        let item_value = response.get("item").cloned().unwrap_or(Value::Null);
        if item_value.is_null() {
            return Ok(None);
        }
        let item_json = serde_json::to_string(&item_value).map_err(|error| {
            RcloneCliError::Io(std::io::Error::other(format!(
                "Invalid rclone rc operations/stat payload: {error}"
            )))
        })?;
        let item = parse_lsjson_stat_item(&item_json).map_err(|message| {
            RcloneCliError::Io(std::io::Error::other(format!(
                "Invalid rclone rc operations/stat item payload: {message}"
            )))
        })?;
        Ok(Some(cloud_entry_from_item(path, item)))
    }
}

impl CloudProvider for RcloneCloudProvider {
    fn list_remotes(&self) -> CloudCommandResult<Vec<CloudRemote>> {
        self.ensure_runtime_ready()?;
        let mut fell_back_from_rc = false;
        let mut fallback_reason: Option<&'static str> = None;
        if self.rc.is_read_enabled() {
            match self.list_remotes_via_rc() {
                Ok(remotes) => {
                    log_backend_selected("cloud_list_remotes", "rc", false, None);
                    return Ok(remotes);
                }
                Err(error) => {
                    fell_back_from_rc = true;
                    fallback_reason = Some(classify_rc_fallback_reason(&error));
                    info!(
                        error = %error,
                        "rclone rc remote discovery failed; falling back to CLI listremotes"
                    );
                }
            }
        }
        let output = self
            .cli
            .run_capture_text(RcloneCommandSpec::new(RcloneSubcommand::ListRemotes))
            .map_err(map_rclone_error)?;
        let remote_ids = parse_listremotes_plain(&output.stdout).map_err(|message| {
            CloudCommandError::new(CloudCommandErrorCode::UnknownError, message)
        })?;
        let config_dump = self
            .cli
            .run_capture_text(RcloneCommandSpec::new(RcloneSubcommand::ConfigDump))
            .map_err(map_rclone_error)?;
        let config_map = parse_config_dump_summaries(&config_dump.stdout).map_err(|message| {
            CloudCommandError::new(CloudCommandErrorCode::InvalidConfig, message)
        })?;
        log_backend_selected(
            "cloud_list_remotes",
            "cli",
            fell_back_from_rc,
            fallback_reason,
        );
        Ok(build_cloud_remotes(remote_ids, config_map))
    }

    fn stat_path(&self, path: &CloudPath) -> CloudCommandResult<Option<CloudEntry>> {
        self.ensure_runtime_ready()?;
        let mut fell_back_from_rc = false;
        let mut fallback_reason: Option<&'static str> = None;
        if self.rc.is_read_enabled() {
            match self.stat_path_via_rc(path) {
                Ok(entry) => {
                    log_backend_selected("cloud_stat_entry", "rc", false, None);
                    return Ok(entry);
                }
                Err(error) => {
                    fell_back_from_rc = true;
                    fallback_reason = Some(classify_rc_fallback_reason(&error));
                    info!(
                        path = %path,
                        error = %error,
                        "rclone rc stat failed; falling back to CLI lsjson --stat"
                    );
                }
            }
        }
        let spec = RcloneCommandSpec::new(RcloneSubcommand::LsJson)
            .arg("--stat")
            .arg(path.to_rclone_remote_spec());
        match self.cli.run_capture_text(spec) {
            Ok(output) => {
                let item = parse_lsjson_stat_item(&output.stdout).map_err(|message| {
                    CloudCommandError::new(CloudCommandErrorCode::UnknownError, message)
                })?;
                log_backend_selected(
                    "cloud_stat_entry",
                    "cli",
                    fell_back_from_rc,
                    fallback_reason,
                );
                Ok(Some(cloud_entry_from_item(path, item)))
            }
            Err(RcloneCliError::NonZero { stderr, stdout, .. })
                if is_rclone_not_found_text(&stderr, &stdout) =>
            {
                log_backend_selected(
                    "cloud_stat_entry",
                    "cli",
                    fell_back_from_rc,
                    fallback_reason,
                );
                Ok(None)
            }
            Err(error) => Err(map_rclone_error_for_remote(path.remote(), error)),
        }
    }

    fn list_dir(&self, path: &CloudPath) -> CloudCommandResult<Vec<CloudEntry>> {
        self.ensure_runtime_ready()?;
        let mut fell_back_from_rc = false;
        let mut fallback_reason: Option<&'static str> = None;
        if self.rc.is_read_enabled() {
            match self.list_dir_via_rc(path) {
                Ok(entries) => {
                    log_backend_selected("cloud_list_entries", "rc", false, None);
                    return Ok(entries);
                }
                Err(error) => {
                    fell_back_from_rc = true;
                    fallback_reason = Some(classify_rc_fallback_reason(&error));
                    info!(
                        path = %path,
                        error = %error,
                        "rclone rc list failed; falling back to CLI lsjson"
                    );
                }
            }
        }
        let output = self.cli.run_capture_text(
            RcloneCommandSpec::new(RcloneSubcommand::LsJson).arg(path.to_rclone_remote_spec()),
        );
        let output = match output {
            Ok(output) => output,
            Err(error) => return Err(map_rclone_error_for_remote(path.remote(), error)),
        };
        let items = parse_lsjson_items(&output.stdout).map_err(|message| {
            CloudCommandError::new(CloudCommandErrorCode::UnknownError, message)
        })?;
        let mut entries = Vec::with_capacity(items.len());
        for item in items {
            let child_path = path.child_path(&item.name).map_err(|error| {
                CloudCommandError::new(
                    CloudCommandErrorCode::InvalidPath,
                    format!("Invalid entry name from rclone lsjson: {error}"),
                )
            })?;
            entries.push(cloud_entry_from_item(&child_path, item));
        }
        entries.sort_by(|a, b| {
            let rank_a = if matches!(a.kind, CloudEntryKind::Dir) {
                0
            } else {
                1
            };
            let rank_b = if matches!(b.kind, CloudEntryKind::Dir) {
                0
            } else {
                1
            };
            rank_a.cmp(&rank_b).then_with(|| a.name.cmp(&b.name))
        });
        log_backend_selected(
            "cloud_list_entries",
            "cli",
            fell_back_from_rc,
            fallback_reason,
        );
        Ok(entries)
    }

    fn mkdir(&self, path: &CloudPath, cancel: Option<&AtomicBool>) -> CloudCommandResult<()> {
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

    fn delete_file(&self, path: &CloudPath, cancel: Option<&AtomicBool>) -> CloudCommandResult<()> {
        self.ensure_runtime_ready()?;
        if is_cancelled(cancel) {
            return Err(cloud_write_cancelled_error());
        }
        let mut fell_back_from_rc = false;
        let mut fallback_reason: Option<&'static str> = None;
        if self.rc.is_write_enabled() {
            let fs_spec = format!("{}:", path.remote());
            let rc_result = {
                #[cfg(test)]
                {
                    if self.force_async_unknown_deletefile_for_tests {
                        Err(RcloneCliError::AsyncJobStateUnknown {
                            subcommand: RcloneSubcommand::Rc,
                            operation: "operations/deletefile".to_string(),
                            job_id: 4201,
                            reason: "injected test fault after async rc kickoff".to_string(),
                        })
                    } else {
                        self.rc
                            .operations_deletefile(&fs_spec, path.rel_path(), cancel)
                    }
                }
                #[cfg(not(test))]
                {
                    self.rc
                        .operations_deletefile(&fs_spec, path.rel_path(), cancel)
                }
            };
            match rc_result {
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

    fn delete_dir_recursive(
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

    fn delete_dir_empty(
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

    fn move_entry(
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

    fn copy_entry(
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
            let rc_result = {
                #[cfg(test)]
                {
                    if self.force_async_unknown_copyfile_for_tests {
                        Err(RcloneCliError::AsyncJobStateUnknown {
                            subcommand: RcloneSubcommand::Rc,
                            operation: "operations/copyfile".to_string(),
                            job_id: 4202,
                            reason: "injected test fault after async rc kickoff".to_string(),
                        })
                    } else {
                        self.rc.operations_copyfile(
                            &src_fs,
                            src.rel_path(),
                            &dst_fs,
                            dst.rel_path(),
                            cancel,
                        )
                    }
                }
                #[cfg(not(test))]
                {
                    self.rc.operations_copyfile(
                        &src_fs,
                        src.rel_path(),
                        &dst_fs,
                        dst.rel_path(),
                        cancel,
                    )
                }
            };
            match rc_result {
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

fn log_backend_selected(
    op: &'static str,
    backend: &'static str,
    fallback: bool,
    reason: Option<&'static str>,
) {
    info!(
        op,
        backend,
        fallback_from_rc = fallback,
        reason = reason.unwrap_or(""),
        "cloud provider backend selected"
    );
}

fn classify_rc_fallback_reason(error: &RcloneCliError) -> &'static str {
    match error {
        RcloneCliError::Timeout { .. } => "rc_timeout",
        RcloneCliError::Shutdown { .. } => "rc_shutdown",
        RcloneCliError::Cancelled { .. } => "rc_cancelled",
        RcloneCliError::AsyncJobStateUnknown { .. } => "rc_async_job_unknown",
        RcloneCliError::NonZero { .. } => "rc_nonzero",
        RcloneCliError::Io(io) => match io.kind() {
            std::io::ErrorKind::WouldBlock => "rc_startup_cooldown",
            std::io::ErrorKind::TimedOut => "rc_io_timeout",
            std::io::ErrorKind::ConnectionRefused => "rc_connect_refused",
            std::io::ErrorKind::NotConnected => "rc_not_connected",
            std::io::ErrorKind::Unsupported => "rc_unsupported",
            std::io::ErrorKind::PermissionDenied => "rc_permission_denied",
            std::io::ErrorKind::NotFound => "rc_not_found",
            _ => "rc_io_error",
        },
    }
}

fn probe_rclone_runtime(cli: &RcloneCli) -> CloudCommandResult<()> {
    let output = cli
        .run_capture_text(RcloneCommandSpec::new(RcloneSubcommand::Version))
        .map_err(map_rclone_error)?;
    let version = parse_rclone_version_stdout(&output.stdout).ok_or_else(|| {
        CloudCommandError::new(
            CloudCommandErrorCode::Unsupported,
            "Unexpected `rclone version` output; cannot verify rclone runtime",
        )
    })?;
    let numeric = parse_rclone_version_triplet(&version).ok_or_else(|| {
        CloudCommandError::new(
            CloudCommandErrorCode::Unsupported,
            format!("Unsupported rclone version format: {version}"),
        )
    })?;
    if numeric < MIN_RCLONE_VERSION {
        return Err(CloudCommandError::new(
            CloudCommandErrorCode::Unsupported,
            format!(
                "rclone v{version} is too old; Browsey requires rclone v{}.{}.{} or newer",
                MIN_RCLONE_VERSION.0, MIN_RCLONE_VERSION.1, MIN_RCLONE_VERSION.2
            ),
        ));
    }
    debug!(version = %version, "rclone runtime probe succeeded");
    Ok(())
}

#[derive(Debug, Clone, Default)]
struct RcloneRemotePolicy {
    allowlist: Option<HashSet<String>>,
    prefix: Option<String>,
}

fn remote_allowed_by_policy(remote_id: &str) -> bool {
    let policy = RCLONE_REMOTE_POLICY.get_or_init(load_remote_policy_from_env);
    remote_allowed_by_policy_with(policy, remote_id)
}

fn remote_allowed_by_policy_with(policy: &RcloneRemotePolicy, remote_id: &str) -> bool {
    if let Some(allowlist) = &policy.allowlist {
        if !allowlist.contains(remote_id) {
            return false;
        }
    }
    if let Some(prefix) = &policy.prefix {
        if !remote_id.starts_with(prefix) {
            return false;
        }
    }
    true
}

fn load_remote_policy_from_env() -> RcloneRemotePolicy {
    let allowlist = env::var("BROWSEY_RCLONE_REMOTE_ALLOWLIST")
        .ok()
        .and_then(|raw| {
            let set = raw
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned)
                .collect::<HashSet<_>>();
            if set.is_empty() {
                None
            } else {
                Some(set)
            }
        });
    let prefix = env::var("BROWSEY_RCLONE_REMOTE_PREFIX")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());
    RcloneRemotePolicy { allowlist, prefix }
}

fn parse_rclone_version_stdout(stdout: &str) -> Option<String> {
    let line = stdout
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default();
    let version = line.strip_prefix("rclone v")?.trim();
    if version.is_empty() {
        return None;
    }
    Some(version.to_string())
}

fn parse_rclone_version_triplet(version: &str) -> Option<(u64, u64, u64)> {
    let mut parts = version.split('.');
    let major = parse_leading_digits(parts.next()?)?;
    let minor = parse_leading_digits(parts.next()?)?;
    let patch = parse_leading_digits(parts.next()?)?;
    Some((major, minor, patch))
}

fn parse_leading_digits(part: &str) -> Option<u64> {
    let digits: String = part.chars().take_while(|ch| ch.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
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

fn cloud_entry_from_item(path: &CloudPath, item: LsJsonItem) -> CloudEntry {
    CloudEntry {
        name: item.name,
        path: path.to_string(),
        kind: if item.is_dir {
            CloudEntryKind::Dir
        } else {
            CloudEntryKind::File
        },
        size: if item.is_dir { None } else { item.size },
        modified: normalize_cloud_modified_time(item.mod_time),
        capabilities: CloudCapabilities::v1_core_rw(),
    }
}

fn normalize_cloud_modified_time(raw: Option<String>) -> Option<String> {
    raw.map(|value| normalize_cloud_modified_time_value(&value))
}

fn normalize_cloud_modified_time_value(value: &str) -> String {
    // rclone lsjson uses RFC3339 timestamps; normalize to Browsey's local display format
    // to keep sorting/filtering/facet bucketing consistent with local filesystem entries.
    DateTime::parse_from_rfc3339(value)
        .map(|dt| {
            dt.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|_| value.to_string())
}

fn map_rclone_error(error: RcloneCliError) -> CloudCommandError {
    map_rclone_error_for_provider(CloudProviderKind::Onedrive, error)
}

fn provider_kind_for_remote(remote_id: &str) -> Option<CloudProviderKind> {
    crate::commands::cloud::list_cloud_remotes_sync_best_effort(false)
        .into_iter()
        .find(|remote| remote.id == remote_id)
        .map(|remote| remote.provider)
}

fn map_rclone_error_for_remote(remote_id: &str, error: RcloneCliError) -> CloudCommandError {
    let providers = provider_kind_for_remote(remote_id)
        .into_iter()
        .collect::<Vec<_>>();
    map_rclone_error_for_providers(&providers, error)
}

fn map_rclone_error_for_paths(paths: &[&CloudPath], error: RcloneCliError) -> CloudCommandError {
    let mut providers = Vec::new();
    for path in paths {
        if let Some(kind) = provider_kind_for_remote(path.remote()) {
            if !providers.contains(&kind) {
                providers.push(kind);
            }
        }
    }
    map_rclone_error_for_providers(&providers, error)
}

fn map_rclone_error_for_providers(
    providers: &[CloudProviderKind],
    error: RcloneCliError,
) -> CloudCommandError {
    match error {
        RcloneCliError::Io(io) if io.kind() == std::io::ErrorKind::NotFound => {
            CloudCommandError::new(
                CloudCommandErrorCode::BinaryMissing,
                "rclone not found in PATH",
            )
        }
        RcloneCliError::Io(io) => CloudCommandError::new(
            CloudCommandErrorCode::NetworkError,
            format!("Failed to run rclone: {io}"),
        ),
        RcloneCliError::Shutdown { .. } => CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            "Application is shutting down; cloud operation was cancelled",
        ),
        RcloneCliError::Cancelled { .. } => cloud_write_cancelled_error(),
        RcloneCliError::AsyncJobStateUnknown {
            operation,
            job_id,
            reason,
            ..
        } => CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            format!(
                "Cloud operation status is unknown after rclone rc {operation} job {job_id}; Browsey did not retry automatically to avoid duplicate operations. Refresh and verify the destination before retrying. Cause: {}",
                reason.trim()
            ),
        ),
        RcloneCliError::Timeout {
            subcommand,
            timeout,
            ..
        } => CloudCommandError::new(
            CloudCommandErrorCode::Timeout,
            format!(
                "rclone {} timed out after {}s",
                subcommand.as_str(),
                timeout.as_secs()
            ),
        ),
        RcloneCliError::NonZero { stderr, stdout, .. } => {
            let msg = if !stderr.trim().is_empty() {
                stderr
            } else {
                stdout
            };
            let code = classify_rclone_message_code_for_providers(providers, &msg);
            CloudCommandError::new(code, msg.trim())
        }
    }
}

fn should_fallback_to_cli_after_rc_error(error: &RcloneCliError) -> bool {
    !matches!(error, RcloneCliError::AsyncJobStateUnknown { .. })
}

fn is_cancelled(cancel: Option<&AtomicBool>) -> bool {
    cancel
        .map(|token| token.load(std::sync::atomic::Ordering::SeqCst))
        .unwrap_or(false)
}

fn cloud_write_cancelled_error() -> CloudCommandError {
    CloudCommandError::new(
        CloudCommandErrorCode::TaskFailed,
        "Cloud operation cancelled",
    )
}

fn map_rclone_error_for_provider(
    provider: CloudProviderKind,
    error: RcloneCliError,
) -> CloudCommandError {
    map_rclone_error_for_providers(&[provider], error)
}

#[allow(dead_code)]
fn classify_rclone_message_code(
    provider: CloudProviderKind,
    message: &str,
) -> CloudCommandErrorCode {
    classify_rclone_message_code_for_providers(&[provider], message)
}

fn classify_rclone_message_code_for_providers(
    providers: &[CloudProviderKind],
    message: &str,
) -> CloudCommandErrorCode {
    if let Some(code) = classify_common_rclone_message_code(message) {
        return code;
    }
    for provider in providers {
        if let Some(code) = classify_provider_rclone_message_code(*provider, message) {
            return code;
        }
    }
    CloudCommandErrorCode::UnknownError
}

fn classify_common_rclone_message_code(message: &str) -> Option<CloudCommandErrorCode> {
    let lower = message.to_ascii_lowercase();
    if lower.contains("didn't find section") || lower.contains("not configured") {
        return Some(CloudCommandErrorCode::InvalidConfig);
    }
    if lower.contains("already exists")
        || lower.contains("duplicate object")
        || lower.contains("destination exists")
    {
        return Some(CloudCommandErrorCode::DestinationExists);
    }
    if lower.contains("permission denied") || lower.contains("access denied") {
        return Some(CloudCommandErrorCode::PermissionDenied);
    }
    if lower.contains("x509:")
        || lower.contains("certificate verify failed")
        || lower.contains("self signed certificate")
        || lower.contains("unknown authority")
        || lower.contains("failed to verify certificate")
        || lower.contains("certificate has expired")
    {
        return Some(CloudCommandErrorCode::TlsCertificateError);
    }
    if lower.contains("too many requests")
        || lower.contains("rate limit")
        || lower.contains("rate-limited")
        || lower.contains("retry after")
        || lower.contains("status code 429")
        || lower.contains("http error 429")
    {
        return Some(CloudCommandErrorCode::RateLimited);
    }
    if lower.contains("unauthorized")
        || lower.contains("authentication failed")
        || lower.contains("login required")
        || lower.contains("invalid_grant")
        || lower.contains("token expired")
        || lower.contains("expired token")
        || lower.contains("status code 401")
        || lower.contains("http error 401")
    {
        return Some(CloudCommandErrorCode::AuthRequired);
    }
    if lower.contains("timed out") || lower.contains("timeout") {
        return Some(CloudCommandErrorCode::Timeout);
    }
    if lower.contains("connection")
        || lower.contains("network")
        || lower.contains("dial tcp")
        || lower.contains("no such host")
        || lower.contains("name resolution")
        || lower.contains("tls handshake")
    {
        return Some(CloudCommandErrorCode::NetworkError);
    }
    if is_rclone_not_found_text(message, "") {
        return Some(CloudCommandErrorCode::NotFound);
    }
    None
}

fn classify_provider_rclone_message_code(
    provider: CloudProviderKind,
    message: &str,
) -> Option<CloudCommandErrorCode> {
    let lower = message.to_ascii_lowercase();
    match provider {
        CloudProviderKind::Onedrive => {
            // OneDrive-specific provider messages can be mapped here as we encounter them.
            if lower.contains("activitylimitreached") {
                return Some(CloudCommandErrorCode::RateLimited);
            }
            None
        }
        CloudProviderKind::Gdrive => {
            // Google Drive-specific semantics/messages are intentionally isolated here.
            None
        }
        CloudProviderKind::Nextcloud => {
            // Nextcloud/WebDAV-specific semantics/messages are intentionally isolated here.
            None
        }
    }
}

fn is_rclone_not_found_text(stderr: &str, stdout: &str) -> bool {
    let hay = if !stderr.trim().is_empty() {
        stderr
    } else {
        stdout
    };
    let lower = hay.to_ascii_lowercase();
    lower.contains("not found")
        || lower.contains("object not found")
        || lower.contains("directory not found")
        || lower.contains("file not found")
}

fn parse_listremotes_plain(stdout: &str) -> Result<Vec<String>, String> {
    let mut remotes = Vec::new();
    for raw in stdout.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let Some(name) = line.strip_suffix(':') else {
            return Err(format!("Unexpected rclone listremotes output line: {line}"));
        };
        if name.is_empty() {
            return Err("Empty remote name in rclone listremotes output".to_string());
        }
        remotes.push(name.to_string());
    }
    Ok(remotes)
}

fn parse_listremotes_rc_json(value: &Value) -> Result<Vec<String>, String> {
    let remotes = value
        .get("remotes")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "Missing `remotes` array in rclone rc config/listremotes output".to_string()
        })?;
    let mut out = Vec::new();
    for entry in remotes {
        let Some(name) = entry.as_str() else {
            return Err(
                "Non-string remote entry in rclone rc config/listremotes output".to_string(),
            );
        };
        if name.trim().is_empty() {
            return Err("Empty remote name in rclone rc config/listremotes output".to_string());
        }
        out.push(name.to_string());
    }
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RcloneRemoteConfigSummary {
    backend_type: String,
    vendor: Option<String>,
    url: Option<String>,
    has_password: bool,
}

fn parse_config_dump_summaries(
    stdout: &str,
) -> Result<HashMap<String, RcloneRemoteConfigSummary>, String> {
    let value: Value = serde_json::from_str(stdout)
        .map_err(|e| format!("Invalid rclone config dump JSON: {e}"))?;
    let obj = value
        .as_object()
        .ok_or_else(|| "Expected top-level object from rclone config dump".to_string())?;
    let mut out = HashMap::new();
    for (remote, config) in obj {
        let Some(cfg) = config.as_object() else {
            continue;
        };
        let Some(backend_type) = cfg.get("type").and_then(|v| v.as_str()) else {
            continue;
        };
        let vendor = cfg
            .get("vendor")
            .and_then(|v| v.as_str())
            .map(|v| v.to_ascii_lowercase());
        let url = cfg
            .get("url")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string());
        let has_password = cfg.contains_key("pass") || cfg.contains_key("password");
        out.insert(
            remote.to_string(),
            RcloneRemoteConfigSummary {
                backend_type: backend_type.to_ascii_lowercase(),
                vendor,
                url,
                has_password,
            },
        );
    }
    Ok(out)
}

fn classify_provider_kind(rclone_type: &str) -> Option<CloudProviderKind> {
    match rclone_type.trim().to_ascii_lowercase().as_str() {
        "onedrive" => Some(CloudProviderKind::Onedrive),
        "drive" => Some(CloudProviderKind::Gdrive),
        "nextcloud" => Some(CloudProviderKind::Nextcloud),
        // Nextcloud is commonly configured through rclone's webdav backend; we avoid guessing here.
        _ => None,
    }
}

fn classify_provider_kind_from_config(
    cfg: &RcloneRemoteConfigSummary,
) -> Option<CloudProviderKind> {
    if let Some(kind) = classify_provider_kind(&cfg.backend_type) {
        return Some(kind);
    }
    if cfg.backend_type == "webdav" {
        if cfg.vendor.as_deref() == Some("nextcloud") {
            return Some(CloudProviderKind::Nextcloud);
        }
        if cfg
            .url
            .as_deref()
            .map(|url| url.to_ascii_lowercase().contains("nextcloud"))
            .unwrap_or(false)
        {
            return Some(CloudProviderKind::Nextcloud);
        }
    }
    None
}

fn format_remote_label(remote_id: &str, provider: CloudProviderKind) -> String {
    let provider_label = match provider {
        CloudProviderKind::Onedrive => "OneDrive",
        CloudProviderKind::Gdrive => "Google Drive",
        CloudProviderKind::Nextcloud => "Nextcloud",
    };
    format!("{remote_id} ({provider_label})")
}

fn build_cloud_remotes(
    remote_ids: Vec<String>,
    config_map: HashMap<String, RcloneRemoteConfigSummary>,
) -> Vec<CloudRemote> {
    let mut remotes = Vec::new();
    let mut seen = HashSet::new();
    for remote_id in remote_ids {
        if !seen.insert(remote_id.clone()) {
            continue;
        }
        if !remote_allowed_by_policy(&remote_id) {
            continue;
        }
        let Some(provider) = config_map
            .get(&remote_id)
            .and_then(classify_provider_kind_from_config)
        else {
            continue;
        };
        remotes.push(CloudRemote {
            id: remote_id.clone(),
            label: format_remote_label(&remote_id, provider),
            provider,
            root_path: format!("rclone://{remote_id}"),
            capabilities: CloudCapabilities::v1_for_provider(provider),
        });
    }
    remotes.sort_by(|a, b| a.label.cmp(&b.label));
    remotes
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
struct LsJsonItem {
    name: String,
    #[serde(default)]
    is_dir: bool,
    #[serde(default, deserialize_with = "deserialize_lsjson_size")]
    size: Option<u64>,
    #[serde(default)]
    mod_time: Option<String>,
}

fn deserialize_lsjson_size<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = <Option<i64> as serde::Deserialize>::deserialize(deserializer)?;
    Ok(raw.and_then(|n| u64::try_from(n).ok()))
}

fn parse_lsjson_items(stdout: &str) -> Result<Vec<LsJsonItem>, String> {
    serde_json::from_str(stdout).map_err(|e| format!("Invalid rclone lsjson output: {e}"))
}

fn parse_lsjson_stat_item(stdout: &str) -> Result<LsJsonItem, String> {
    serde_json::from_str(stdout).map_err(|e| format!("Invalid rclone lsjson --stat output: {e}"))
}

#[cfg(test)]
mod tests {
    use super::{
        classify_provider_kind, classify_provider_kind_from_config,
        classify_provider_rclone_message_code, classify_rclone_message_code,
        is_rclone_not_found_text, map_rclone_error, map_rclone_error_for_provider,
        normalize_cloud_modified_time_value, parse_config_dump_summaries, parse_listremotes_plain,
        parse_lsjson_items, parse_lsjson_stat_item, parse_rclone_version_stdout,
        parse_rclone_version_triplet, remote_allowed_by_policy_with,
        should_fallback_to_cli_after_rc_error, RcloneCloudProvider, RcloneRemotePolicy,
    };
    use crate::{
        commands::cloud::{
            error::CloudCommandErrorCode,
            path::CloudPath,
            provider::CloudProvider,
            rclone_cli::RcloneCli,
            rclone_cli::RcloneCliError,
            rclone_cli::RcloneSubcommand,
            rclone_rc::RcloneRcClient,
            types::{CloudEntryKind, CloudProviderKind},
        },
        errors::domain::{DomainError, ErrorCode},
    };
    use std::{process::ExitStatus, time::Duration};

    #[cfg(unix)]
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    #[cfg(unix)]
    fn fake_exit_status(code: i32) -> ExitStatus {
        use std::os::unix::process::ExitStatusExt;
        ExitStatus::from_raw(code << 8)
    }

    #[cfg(windows)]
    fn fake_exit_status(code: u32) -> ExitStatus {
        use std::os::windows::process::ExitStatusExt;
        ExitStatus::from_raw(code)
    }

    #[test]
    fn parses_listremotes_plain_output() {
        let out = parse_listremotes_plain("work:\npersonal:\n\n").expect("parse");
        assert_eq!(out, vec!["work".to_string(), "personal".to_string()]);
    }

    #[test]
    fn parses_config_dump_config_summaries() {
        let json = r#"{
          "work": {"type":"onedrive","token":"secret"},
          "photos": {"type":"drive"},
          "nc": {"type":"webdav","vendor":"nextcloud","url":"https://cloud.example/remote.php/dav/files/user","pass":"***"},
          "misc": {"provider":"something"}
        }"#;
        let map = parse_config_dump_summaries(json).expect("parse json");
        assert_eq!(
            map.get("work").map(|c| c.backend_type.as_str()),
            Some("onedrive")
        );
        assert_eq!(
            map.get("photos").map(|c| c.backend_type.as_str()),
            Some("drive")
        );
        assert_eq!(
            map.get("nc").and_then(|c| c.vendor.as_deref()),
            Some("nextcloud")
        );
        assert!(map.get("nc").map(|c| c.has_password).unwrap_or(false));
        assert!(!map.contains_key("misc"));
    }

    #[test]
    fn classifies_supported_provider_types() {
        assert_eq!(
            classify_provider_kind("onedrive"),
            Some(CloudProviderKind::Onedrive)
        );
        assert_eq!(
            classify_provider_kind("drive"),
            Some(CloudProviderKind::Gdrive)
        );
        assert_eq!(classify_provider_kind("webdav"), None);
    }

    #[test]
    fn classifies_nextcloud_from_webdav_config_metadata() {
        let map = parse_config_dump_summaries(
            r#"{
              "nc-vendor": {"type":"webdav","vendor":"nextcloud","url":"https://cloud.example/remote.php/dav/files/user"},
              "nc-url": {"type":"webdav","url":"https://nextcloud.example/remote.php/dav/files/user"},
              "plain-webdav": {"type":"webdav","url":"https://dav.example/remote.php/webdav"}
            }"#,
        )
        .expect("parse config dump");
        assert_eq!(
            classify_provider_kind_from_config(map.get("nc-vendor").expect("nc-vendor")),
            Some(CloudProviderKind::Nextcloud)
        );
        assert_eq!(
            classify_provider_kind_from_config(map.get("nc-url").expect("nc-url")),
            Some(CloudProviderKind::Nextcloud)
        );
        assert_eq!(
            classify_provider_kind_from_config(map.get("plain-webdav").expect("plain-webdav")),
            None
        );
    }

    #[test]
    fn parses_lsjson_items() {
        let json = r#"[
          {"Name":"Folder","IsDir":true,"Size":0,"ModTime":"2026-02-25T10:00:00Z"},
          {"Name":"note.txt","IsDir":false,"Size":12,"ModTime":"2026-02-25T10:01:00Z"}
        ]"#;
        let items = parse_lsjson_items(json).expect("parse lsjson");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].name, "Folder");
        assert!(items[0].is_dir);
        assert_eq!(items[1].name, "note.txt");
        assert_eq!(items[1].size, Some(12));
    }

    #[test]
    fn parses_lsjson_items_with_negative_directory_size() {
        let json = r#"[
          {"Name":"Folder","IsDir":true,"Size":-1,"ModTime":"2026-02-25T10:00:00Z"}
        ]"#;
        let items = parse_lsjson_items(json).expect("parse lsjson with -1 dir size");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "Folder");
        assert!(items[0].is_dir);
        assert_eq!(items[0].size, None);
    }

    #[test]
    fn parses_lsjson_stat_item() {
        let json =
            r#"{"Name":"note.txt","IsDir":false,"Size":12,"ModTime":"2026-02-25T10:01:00Z"}"#;
        let item = parse_lsjson_stat_item(json).expect("parse lsjson stat");
        assert_eq!(item.name, "note.txt");
        assert!(!item.is_dir);
        assert_eq!(item.size, Some(12));
    }

    #[test]
    fn normalizes_rclone_rfc3339_mod_time_to_browsey_format() {
        let out = normalize_cloud_modified_time_value("2026-02-25T10:01:45Z");
        assert!(out.len() == 16, "expected YYYY-MM-DD HH:MM, got {out}");
        assert!(
            out.contains(' '),
            "expected local Browsey time format, got {out}"
        );
        assert!(!out.contains('T'), "expected normalized format, got {out}");
    }

    #[test]
    fn detects_not_found_rclone_messages() {
        assert!(is_rclone_not_found_text(
            "Failed to lsjson: object not found",
            ""
        ));
        assert!(is_rclone_not_found_text("", "directory not found"));
        assert!(!is_rclone_not_found_text("permission denied", ""));
    }

    #[test]
    fn classifies_common_rclone_error_messages() {
        assert_eq!(
            classify_rclone_message_code(
                CloudProviderKind::Onedrive,
                "Failed to move: destination exists"
            ),
            CloudCommandErrorCode::DestinationExists
        );
        assert_eq!(
            classify_rclone_message_code(CloudProviderKind::Onedrive, "Permission denied"),
            CloudCommandErrorCode::PermissionDenied
        );
        assert_eq!(
            classify_rclone_message_code(CloudProviderKind::Onedrive, "object not found"),
            CloudCommandErrorCode::NotFound
        );
        assert_eq!(
            classify_rclone_message_code(
                CloudProviderKind::Onedrive,
                "HTTP error 429: too many requests"
            ),
            CloudCommandErrorCode::RateLimited
        );
        assert_eq!(
            classify_rclone_message_code(
                CloudProviderKind::Onedrive,
                "authentication failed: token expired"
            ),
            CloudCommandErrorCode::AuthRequired
        );
        assert_eq!(
            classify_rclone_message_code(
                CloudProviderKind::Nextcloud,
                "x509: certificate signed by unknown authority"
            ),
            CloudCommandErrorCode::TlsCertificateError
        );
        assert_eq!(
            classify_rclone_message_code(CloudProviderKind::Onedrive, "dial tcp: i/o timeout"),
            CloudCommandErrorCode::Timeout
        );
    }

    #[test]
    fn provider_specific_rclone_message_mapping_is_isolated() {
        assert_eq!(
            classify_provider_rclone_message_code(
                CloudProviderKind::Onedrive,
                "graph returned ActivityLimitReached"
            ),
            Some(CloudCommandErrorCode::RateLimited)
        );
        assert_eq!(
            classify_provider_rclone_message_code(
                CloudProviderKind::Gdrive,
                "graph returned ActivityLimitReached"
            ),
            None
        );
    }

    #[test]
    fn parses_rclone_version_output() {
        let out = "rclone v1.69.1\n- os/version: fedora 41\n";
        assert_eq!(parse_rclone_version_stdout(out).as_deref(), Some("1.69.1"));
        assert_eq!(parse_rclone_version_stdout("not-rclone\n"), None);
    }

    #[test]
    fn parses_rclone_version_triplet_with_suffixes() {
        assert_eq!(parse_rclone_version_triplet("1.69.1"), Some((1, 69, 1)));
        assert_eq!(
            parse_rclone_version_triplet("1.68.0-beta.1"),
            Some((1, 68, 0))
        );
        assert_eq!(parse_rclone_version_triplet("v1.69.1"), None);
        assert_eq!(parse_rclone_version_triplet("1.69"), None);
    }

    #[test]
    fn maps_rclone_timeout_to_cloud_timeout_error_code() {
        let err = map_rclone_error(RcloneCliError::Timeout {
            subcommand: RcloneSubcommand::CopyTo,
            timeout: Duration::from_secs(10),
            stdout: String::new(),
            stderr: "timed out".to_string(),
        });
        assert_eq!(err.code_str(), CloudCommandErrorCode::Timeout.as_code_str());
        assert!(err.to_string().contains("timed out"));
    }

    #[test]
    fn maps_rclone_nonzero_stderr_to_cloud_error_code() {
        let err = map_rclone_error(RcloneCliError::NonZero {
            status: fake_exit_status(1),
            stdout: String::new(),
            stderr: "HTTP error 429: too many requests".to_string(),
        });
        assert_eq!(
            err.code_str(),
            CloudCommandErrorCode::RateLimited.as_code_str()
        );
    }

    #[test]
    fn maps_async_job_unknown_to_task_failed_with_guidance() {
        let err = map_rclone_error(RcloneCliError::AsyncJobStateUnknown {
            subcommand: RcloneSubcommand::Rc,
            operation: "operations/copyfile".to_string(),
            job_id: 42,
            reason: "job/status failed: connection reset".to_string(),
        });
        assert_eq!(
            err.code_str(),
            CloudCommandErrorCode::TaskFailed.as_code_str()
        );
        let msg = err.to_string();
        assert!(
            msg.contains("status is unknown"),
            "unexpected message: {msg}"
        );
        assert!(
            msg.contains("did not retry automatically"),
            "unexpected message: {msg}"
        );
        assert!(msg.contains("job 42"), "unexpected message: {msg}");
    }

    #[test]
    fn async_job_unknown_error_is_not_cli_fallback_safe() {
        let unknown = RcloneCliError::AsyncJobStateUnknown {
            subcommand: RcloneSubcommand::Rc,
            operation: "operations/deletefile".to_string(),
            job_id: 7,
            reason: "job/status timed out".to_string(),
        };
        assert!(!should_fallback_to_cli_after_rc_error(&unknown));

        let timeout = RcloneCliError::Timeout {
            subcommand: RcloneSubcommand::Rc,
            timeout: Duration::from_secs(5),
            stdout: String::new(),
            stderr: "timed out".to_string(),
        };
        assert!(should_fallback_to_cli_after_rc_error(&timeout));
    }

    #[test]
    fn provider_specific_error_mapping_does_not_leak_between_providers() {
        let onedrive_err = map_rclone_error_for_provider(
            CloudProviderKind::Onedrive,
            RcloneCliError::NonZero {
                status: fake_exit_status(1),
                stdout: String::new(),
                stderr: "ActivityLimitReached".to_string(),
            },
        );
        let gdrive_err = map_rclone_error_for_provider(
            CloudProviderKind::Gdrive,
            RcloneCliError::NonZero {
                status: fake_exit_status(1),
                stdout: String::new(),
                stderr: "ActivityLimitReached".to_string(),
            },
        );
        assert_eq!(
            onedrive_err.code_str(),
            CloudCommandErrorCode::RateLimited.as_code_str()
        );
        assert_eq!(
            gdrive_err.code_str(),
            CloudCommandErrorCode::UnknownError.as_code_str()
        );
    }

    #[test]
    fn remote_policy_filters_by_allowlist_and_prefix() {
        let policy = RcloneRemotePolicy {
            allowlist: Some(
                ["browsey-work", "browsey-personal"]
                    .into_iter()
                    .map(ToOwned::to_owned)
                    .collect(),
            ),
            prefix: Some("browsey-".to_string()),
        };
        assert!(remote_allowed_by_policy_with(&policy, "browsey-work"));
        assert!(!remote_allowed_by_policy_with(&policy, "work"));
        assert!(!remote_allowed_by_policy_with(&policy, "browsey-other"));
    }

    #[cfg(unix)]
    struct FakeRcloneSandbox {
        root: PathBuf,
        script_path: PathBuf,
        state_root: PathBuf,
        log_path: PathBuf,
    }

    #[cfg(unix)]
    impl FakeRcloneSandbox {
        fn new() -> Self {
            static NEXT_ID: AtomicU64 = AtomicU64::new(1);
            let unique = format!(
                "browsey-fake-rclone-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("time")
                    .as_nanos()
                    + u128::from(NEXT_ID.fetch_add(1, Ordering::Relaxed))
            );
            let root = std::env::temp_dir().join(unique);
            let state_root = root.join("state");
            let script_path = root.join("rclone");
            let log_path = root.join("fake-rclone.log");
            fs::create_dir_all(&state_root).expect("create state root");
            let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/fake-rclone.sh");
            fs::copy(&source, &script_path).expect("copy fake rclone script");
            let mut perms = fs::metadata(&script_path)
                .expect("script metadata")
                .permissions();
            use std::os::unix::fs::PermissionsExt;
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms).expect("chmod fake rclone");
            Self {
                root,
                script_path,
                state_root,
                log_path,
            }
        }

        fn provider(&self) -> RcloneCloudProvider {
            RcloneCloudProvider::new(RcloneCli::new(self.script_path.as_os_str()))
        }

        fn provider_with_forced_rc(&self) -> RcloneCloudProvider {
            crate::commands::cloud::rclone_rc::reset_state_for_tests();
            let cli = RcloneCli::new(self.script_path.as_os_str());
            let rc = RcloneRcClient::new(self.script_path.as_os_str())
                .with_enabled_override_for_tests(true);
            RcloneCloudProvider {
                cli,
                rc,
                force_async_unknown_deletefile_for_tests: false,
                force_async_unknown_copyfile_for_tests: false,
            }
        }

        fn remote_path(&self, remote: &str, rel: &str) -> PathBuf {
            let base = self.state_root.join(remote);
            if rel.is_empty() {
                base
            } else {
                base.join(rel)
            }
        }

        fn mkdir_remote(&self, remote: &str, rel: &str) {
            fs::create_dir_all(self.remote_path(remote, rel)).expect("mkdir remote path");
        }

        fn write_remote_file(&self, remote: &str, rel: &str, content: &str) {
            let path = self.remote_path(remote, rel);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("mkdir parent");
            }
            fs::write(path, content).expect("write remote file");
        }

        fn read_log(&self) -> String {
            fs::read_to_string(&self.log_path).unwrap_or_default()
        }
    }

    #[cfg(unix)]
    impl Drop for FakeRcloneSandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[cfg(unix)]
    fn cloud_path(raw: &str) -> CloudPath {
        CloudPath::parse(raw).expect("valid cloud path")
    }

    #[cfg(unix)]
    #[test]
    fn fake_rclone_shim_lists_remotes_and_directory_entries() {
        let sandbox = FakeRcloneSandbox::new();
        sandbox.mkdir_remote("work", "Docs");
        sandbox.write_remote_file("work", "note.txt", "hello cloud");
        let provider = sandbox.provider();

        let remotes = provider.list_remotes().expect("list remotes");
        assert_eq!(remotes.len(), 1);
        assert_eq!(remotes[0].id, "work");
        assert_eq!(remotes[0].provider, CloudProviderKind::Onedrive);
        assert_eq!(remotes[0].root_path, "rclone://work");

        let entries = provider
            .list_dir(&cloud_path("rclone://work"))
            .expect("list dir");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "Docs");
        assert_eq!(entries[0].kind, CloudEntryKind::Dir);
        assert_eq!(entries[1].name, "note.txt");
        assert_eq!(entries[1].kind, CloudEntryKind::File);
        assert_eq!(entries[1].size, Some("hello cloud".len() as u64));

        let log = sandbox.read_log();
        // `rclone version` may be skipped here because runtime probe is cached process-wide.
        assert!(log.contains("listremotes"));
        assert!(log.contains("config dump"));
        assert!(log.contains("lsjson work:"));
    }

    #[cfg(unix)]
    #[test]
    fn fake_rclone_shim_supports_copy_move_and_delete_operations() {
        let sandbox = FakeRcloneSandbox::new();
        sandbox.write_remote_file("work", "src/file.txt", "payload");
        sandbox.write_remote_file("work", "trash/sub/old.txt", "gone");
        let provider = sandbox.provider();

        provider
            .mkdir(&cloud_path("rclone://work/dst"), None)
            .expect("mkdir dst");
        provider
            .copy_entry(
                &cloud_path("rclone://work/src/file.txt"),
                &cloud_path("rclone://work/dst/copied.txt"),
                false,
                false,
                None,
            )
            .expect("copy file");
        assert!(sandbox.remote_path("work", "dst/copied.txt").is_file());

        let copied_stat = provider
            .stat_path(&cloud_path("rclone://work/dst/copied.txt"))
            .expect("stat copied")
            .expect("copied exists");
        assert_eq!(copied_stat.name, "copied.txt");
        assert_eq!(copied_stat.kind, CloudEntryKind::File);

        provider
            .move_entry(
                &cloud_path("rclone://work/dst/copied.txt"),
                &cloud_path("rclone://work/dst/moved.txt"),
                false,
                false,
                None,
            )
            .expect("move file");
        assert!(!sandbox.remote_path("work", "dst/copied.txt").exists());
        assert!(sandbox.remote_path("work", "dst/moved.txt").exists());

        provider
            .delete_file(&cloud_path("rclone://work/dst/moved.txt"), None)
            .expect("delete file");
        assert!(!sandbox.remote_path("work", "dst/moved.txt").exists());

        provider
            .delete_dir_empty(&cloud_path("rclone://work/dst"), None)
            .expect("delete empty dir");
        assert!(!sandbox.remote_path("work", "dst").exists());

        provider
            .delete_dir_recursive(&cloud_path("rclone://work/trash"), None)
            .expect("purge dir");
        assert!(!sandbox.remote_path("work", "trash").exists());

        let log = sandbox.read_log();
        assert!(log.contains("mkdir work:dst"));
        assert!(log.contains("copyto work:src/file.txt work:dst/copied.txt"));
        assert!(log.contains("moveto work:dst/copied.txt work:dst/moved.txt"));
        assert!(log.contains("deletefile work:dst/moved.txt"));
        assert!(log.contains("rmdir work:dst"));
        assert!(log.contains("purge work:trash"));
    }

    #[cfg(unix)]
    #[test]
    fn fake_rclone_shim_supports_case_only_rename() {
        let sandbox = FakeRcloneSandbox::new();
        sandbox.write_remote_file("work", "docs/report.txt", "payload");
        let provider = sandbox.provider();

        provider
            .move_entry(
                &cloud_path("rclone://work/docs/report.txt"),
                &cloud_path("rclone://work/docs/Report.txt"),
                false,
                false,
                None,
            )
            .expect("case-only rename");

        assert!(!sandbox.remote_path("work", "docs/report.txt").exists());
        assert!(sandbox.remote_path("work", "docs/Report.txt").exists());
    }

    #[cfg(unix)]
    #[test]
    fn read_path_falls_back_to_cli_when_rc_startup_fails() {
        let sandbox = FakeRcloneSandbox::new();
        sandbox.write_remote_file("work", "note.txt", "hello");
        let provider = sandbox.provider_with_forced_rc();

        let entries = provider
            .list_dir(&cloud_path("rclone://work"))
            .expect("list dir with fallback");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "note.txt");

        let log = sandbox.read_log();
        assert!(
            log.contains("rcd --rc-no-auth"),
            "expected rc daemon to start without auth on private unix socket, log:\n{log}"
        );
        assert!(
            log.contains("--rc-addr"),
            "expected rc daemon startup attempt before fallback, log:\n{log}"
        );
        assert!(
            log.contains("unix://"),
            "expected unix socket rc endpoint (no TCP listener), log:\n{log}"
        );
        assert!(
            log.contains("lsjson work:"),
            "expected CLI fallback list call after rc failure, log:\n{log}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn mkdir_falls_back_to_cli_when_rc_startup_fails() {
        let sandbox = FakeRcloneSandbox::new();
        let provider = sandbox.provider_with_forced_rc();

        provider
            .mkdir(&cloud_path("rclone://work/new-folder"), None)
            .expect("mkdir with fallback");
        assert!(sandbox.remote_path("work", "new-folder").is_dir());

        let log = sandbox.read_log();
        assert!(
            log.contains("rcd --rc-no-auth"),
            "expected rc daemon to start without auth on private unix socket, log:\n{log}"
        );
        assert!(
            log.contains("--rc-addr"),
            "expected rc daemon startup attempt before fallback, log:\n{log}"
        );
        assert!(
            log.contains("mkdir work:new-folder"),
            "expected CLI mkdir fallback call after rc failure, log:\n{log}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn delete_ops_fall_back_to_cli_when_rc_startup_fails() {
        let sandbox = FakeRcloneSandbox::new();
        sandbox.write_remote_file("work", "trash/file.txt", "payload");
        sandbox.write_remote_file("work", "trash-deep/sub/old.txt", "payload");
        let provider = sandbox.provider_with_forced_rc();

        provider
            .delete_file(&cloud_path("rclone://work/trash/file.txt"), None)
            .expect("delete file with fallback");
        assert!(!sandbox.remote_path("work", "trash/file.txt").exists());

        provider
            .delete_dir_empty(&cloud_path("rclone://work/trash"), None)
            .expect("delete empty dir with fallback");
        assert!(!sandbox.remote_path("work", "trash").exists());

        provider
            .delete_dir_recursive(&cloud_path("rclone://work/trash-deep"), None)
            .expect("delete recursive dir with fallback");
        assert!(!sandbox.remote_path("work", "trash-deep").exists());

        let log = sandbox.read_log();
        assert!(
            log.contains("rcd --rc-no-auth"),
            "expected rc daemon to start without auth on private unix socket, log:\n{log}"
        );
        assert!(
            log.contains("--rc-addr"),
            "expected rc daemon startup attempt before fallback, log:\n{log}"
        );
        assert!(
            log.contains("deletefile work:trash/file.txt"),
            "expected CLI deletefile fallback call, log:\n{log}"
        );
        assert!(
            log.contains("rmdir work:trash"),
            "expected CLI rmdir fallback call, log:\n{log}"
        );
        assert!(
            log.contains("purge work:trash-deep"),
            "expected CLI purge fallback call, log:\n{log}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn copy_move_ops_fall_back_to_cli_when_rc_startup_fails() {
        let sandbox = FakeRcloneSandbox::new();
        sandbox.write_remote_file("work", "src/file.txt", "payload");
        let provider = sandbox.provider_with_forced_rc();

        provider
            .copy_entry(
                &cloud_path("rclone://work/src/file.txt"),
                &cloud_path("rclone://work/dst/copied.txt"),
                false,
                false,
                None,
            )
            .expect("copy with fallback");
        assert!(sandbox.remote_path("work", "dst/copied.txt").exists());

        provider
            .move_entry(
                &cloud_path("rclone://work/dst/copied.txt"),
                &cloud_path("rclone://work/dst/moved.txt"),
                false,
                false,
                None,
            )
            .expect("move with fallback");
        assert!(!sandbox.remote_path("work", "dst/copied.txt").exists());
        assert!(sandbox.remote_path("work", "dst/moved.txt").exists());

        let log = sandbox.read_log();
        assert!(
            log.contains("rcd --rc-no-auth"),
            "expected rc daemon to start without auth on private unix socket, log:\n{log}"
        );
        assert!(
            log.contains("--rc-addr"),
            "expected rc daemon startup attempt before fallback, log:\n{log}"
        );
        assert!(
            log.contains("copyto work:src/file.txt work:dst/copied.txt"),
            "expected CLI copy fallback call, log:\n{log}"
        );
        assert!(
            log.contains("moveto work:dst/copied.txt work:dst/moved.txt"),
            "expected CLI move fallback call, log:\n{log}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn delete_file_does_not_fallback_to_cli_when_rc_async_status_is_unknown() {
        let sandbox = FakeRcloneSandbox::new();
        sandbox.write_remote_file("work", "dst/moved.txt", "payload");
        let provider = sandbox
            .provider_with_forced_rc()
            .with_forced_async_unknown_deletefile_for_tests();

        let err = provider
            .delete_file(&cloud_path("rclone://work/dst/moved.txt"), None)
            .expect_err("delete should fail with unknown async rc job state");
        assert_eq!(
            err.code_str(),
            CloudCommandErrorCode::TaskFailed.as_code_str()
        );
        let message = err.to_string();
        assert!(
            message.contains("status is unknown"),
            "unexpected message: {message}"
        );
        assert!(
            message.contains("did not retry automatically"),
            "unexpected message: {message}"
        );
        let log = sandbox.read_log();
        assert!(
            !log.contains("deletefile work:dst/moved.txt"),
            "CLI deletefile must not run after unknown async rc state, log:\n{log}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn copy_does_not_fallback_to_cli_when_rc_async_status_is_unknown() {
        let sandbox = FakeRcloneSandbox::new();
        sandbox.write_remote_file("work", "src/file.txt", "payload");
        let provider = sandbox
            .provider_with_forced_rc()
            .with_forced_async_unknown_copyfile_for_tests();

        let err = provider
            .copy_entry(
                &cloud_path("rclone://work/src/file.txt"),
                &cloud_path("rclone://work/dst/copied.txt"),
                false,
                true,
                None,
            )
            .expect_err("copy should fail with unknown async rc job state");
        assert_eq!(
            err.code_str(),
            CloudCommandErrorCode::TaskFailed.as_code_str()
        );
        let message = err.to_string();
        assert!(
            message.contains("status is unknown"),
            "unexpected message: {message}"
        );
        assert!(
            message.contains("did not retry automatically"),
            "unexpected message: {message}"
        );
        let log = sandbox.read_log();
        assert!(
            !log.contains("copyto work:src/file.txt work:dst/copied.txt"),
            "CLI copyto must not run after unknown async rc state, log:\n{log}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn copy_preserves_destination_exists_conflict_policy() {
        let sandbox = FakeRcloneSandbox::new();
        sandbox.write_remote_file("work", "src/file.txt", "source");
        sandbox.write_remote_file("work", "dst/file.txt", "existing");
        let provider = sandbox.provider_with_forced_rc();

        let err = provider
            .copy_entry(
                &cloud_path("rclone://work/src/file.txt"),
                &cloud_path("rclone://work/dst/file.txt"),
                false,
                false,
                None,
            )
            .expect_err("copy should fail when destination exists");
        assert_eq!(
            err.code_str(),
            CloudCommandErrorCode::DestinationExists.as_code_str()
        );
        let existing = fs::read_to_string(sandbox.remote_path("work", "dst/file.txt"))
            .expect("read existing destination");
        assert_eq!(existing, "existing");
    }

    #[cfg(unix)]
    #[test]
    fn move_preserves_destination_exists_conflict_policy() {
        let sandbox = FakeRcloneSandbox::new();
        sandbox.write_remote_file("work", "src/file.txt", "source");
        sandbox.write_remote_file("work", "dst/file.txt", "existing");
        let provider = sandbox.provider_with_forced_rc();

        let err = provider
            .move_entry(
                &cloud_path("rclone://work/src/file.txt"),
                &cloud_path("rclone://work/dst/file.txt"),
                false,
                false,
                None,
            )
            .expect_err("move should fail when destination exists");
        assert_eq!(
            err.code_str(),
            CloudCommandErrorCode::DestinationExists.as_code_str()
        );
        assert!(
            sandbox.remote_path("work", "src/file.txt").exists(),
            "source must stay in place after rejected move"
        );
        let existing = fs::read_to_string(sandbox.remote_path("work", "dst/file.txt"))
            .expect("read existing destination");
        assert_eq!(existing, "existing");
    }

    #[cfg(unix)]
    #[test]
    fn copy_with_overwrite_true_replaces_destination() {
        let sandbox = FakeRcloneSandbox::new();
        sandbox.write_remote_file("work", "src/file.txt", "source");
        sandbox.write_remote_file("work", "dst/file.txt", "existing");
        let provider = sandbox.provider_with_forced_rc();

        provider
            .copy_entry(
                &cloud_path("rclone://work/src/file.txt"),
                &cloud_path("rclone://work/dst/file.txt"),
                true,
                false,
                None,
            )
            .expect("copy should overwrite destination");
        let copied = fs::read_to_string(sandbox.remote_path("work", "dst/file.txt"))
            .expect("read overwritten destination");
        assert_eq!(copied, "source");
    }

    #[cfg(unix)]
    #[test]
    fn fake_rclone_shim_skips_destination_stat_when_copy_is_prechecked() {
        let sandbox = FakeRcloneSandbox::new();
        sandbox.write_remote_file("work", "src/file.txt", "payload");
        let provider = sandbox.provider();

        provider
            .copy_entry(
                &cloud_path("rclone://work/src/file.txt"),
                &cloud_path("rclone://work/dst/copied.txt"),
                false,
                true,
                None,
            )
            .expect("copy file");

        let log = sandbox.read_log();
        assert!(log.contains("copyto work:src/file.txt work:dst/copied.txt"));
        assert!(!log.contains("lsjson --stat work:dst/copied.txt"));
    }

    #[cfg(unix)]
    #[test]
    fn fake_rclone_shim_skips_destination_stat_when_move_is_prechecked() {
        let sandbox = FakeRcloneSandbox::new();
        sandbox.write_remote_file("work", "src/file.txt", "payload");
        let provider = sandbox.provider();

        provider
            .move_entry(
                &cloud_path("rclone://work/src/file.txt"),
                &cloud_path("rclone://work/dst/moved.txt"),
                false,
                true,
                None,
            )
            .expect("move file");

        let log = sandbox.read_log();
        assert!(log.contains("moveto work:src/file.txt work:dst/moved.txt"));
        assert!(!log.contains("lsjson --stat work:dst/moved.txt"));
    }
}
