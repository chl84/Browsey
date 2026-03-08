use super::{
    error::{is_rclone_not_found_text, map_rclone_error_for_remote},
    logging::{classify_rc_fallback_reason, log_backend_selected},
    parse::{
        parse_lsjson_items, parse_lsjson_items_value, parse_lsjson_stat_item,
        parse_lsjson_stat_item_value, LsJsonItem,
    },
    CloudCapabilities, CloudCommandError, CloudCommandErrorCode, CloudCommandResult, CloudEntry,
    CloudEntryKind, CloudPath, RcloneCliError, RcloneCloudProvider, RcloneCommandSpec,
    RcloneReadBackend, RcloneReadOptions, RcloneSubcommand,
};
use chrono::{DateTime, Local};
use serde_json::Value;
use tracing::warn;

impl RcloneCloudProvider {
    pub(super) fn list_dir_impl(
        &self,
        path: &CloudPath,
        options: RcloneReadOptions<'_>,
    ) -> CloudCommandResult<Vec<CloudEntry>> {
        self.ensure_runtime_ready()?;
        if matches!(options.backend, RcloneReadBackend::CliOnly) {
            let entries = self.list_dir_via_cli(path, options, false, None)?;
            log_backend_selected("cloud_list_entries", "cli", false, None);
            return Ok(entries);
        }
        let mut fell_back_from_rc = false;
        let mut fallback_reason: Option<&'static str> = None;
        if self.rc.is_read_enabled() {
            match self.list_dir_via_rc(path, options) {
                Ok(entries) => {
                    log_backend_selected("cloud_list_entries", "rc", false, None);
                    return Ok(entries);
                }
                Err(error) => {
                    if matches!(options.backend, RcloneReadBackend::RcOnly) {
                        return Err(map_rclone_error_for_remote(path.remote(), error));
                    }
                    fell_back_from_rc = true;
                    fallback_reason = Some(classify_rc_fallback_reason(&error));
                    warn!(
                        path = %path,
                        error = %error,
                        "rclone rc list failed; falling back to CLI lsjson"
                    );
                }
            }
        } else if matches!(options.backend, RcloneReadBackend::RcOnly) {
            return Err(CloudCommandError::new(
                CloudCommandErrorCode::TaskFailed,
                "rclone rc reads are disabled in this Browsey session",
            ));
        }
        if cloud_read_cancelled(options.cancel) {
            return Err(cloud_read_cancelled_error());
        }
        let entries = self.list_dir_via_cli(path, options, fell_back_from_rc, fallback_reason)?;
        log_backend_selected(
            "cloud_list_entries",
            "cli",
            fell_back_from_rc,
            fallback_reason,
        );
        Ok(entries)
    }

    pub(super) fn stat_path_impl(
        &self,
        path: &CloudPath,
        options: RcloneReadOptions<'_>,
    ) -> CloudCommandResult<Option<CloudEntry>> {
        self.ensure_runtime_ready()?;
        if matches!(options.backend, RcloneReadBackend::CliOnly) {
            let entry = self.stat_path_via_cli(path, options, false, None)?;
            log_backend_selected("cloud_stat_entry", "cli", false, None);
            return Ok(entry);
        }
        let mut fell_back_from_rc = false;
        let mut fallback_reason: Option<&'static str> = None;
        if self.rc.is_read_enabled() {
            match self.stat_path_via_rc(path, options) {
                Ok(entry) => {
                    log_backend_selected("cloud_stat_entry", "rc", false, None);
                    return Ok(entry);
                }
                Err(error) => {
                    if matches!(options.backend, RcloneReadBackend::RcOnly) {
                        return Err(map_rclone_error_for_remote(path.remote(), error));
                    }
                    fell_back_from_rc = true;
                    fallback_reason = Some(classify_rc_fallback_reason(&error));
                    warn!(
                        path = %path,
                        error = %error,
                        "rclone rc stat failed; falling back to CLI lsjson --stat"
                    );
                }
            }
        } else if matches!(options.backend, RcloneReadBackend::RcOnly) {
            return Err(CloudCommandError::new(
                CloudCommandErrorCode::TaskFailed,
                "rclone rc reads are disabled in this Browsey session",
            ));
        }
        if cloud_read_cancelled(options.cancel) {
            return Err(cloud_read_cancelled_error());
        }
        let entry = self.stat_path_via_cli(path, options, fell_back_from_rc, fallback_reason)?;
        log_backend_selected(
            "cloud_stat_entry",
            "cli",
            fell_back_from_rc,
            fallback_reason,
        );
        Ok(entry)
    }

    pub(super) fn list_dir_via_rc(
        &self,
        path: &CloudPath,
        options: RcloneReadOptions<'_>,
    ) -> Result<Vec<CloudEntry>, RcloneCliError> {
        let fs_spec = format!("{}:", path.remote());
        let response = self.rc.operations_list_with_options(
            &fs_spec,
            path.rel_path(),
            options.rc_timeout,
            options.cancel,
        )?;
        let list = response
            .get("list")
            .ok_or_else(|| {
                RcloneCliError::Io(std::io::Error::other(
                    "Invalid rclone rc operations/list payload: missing `list` field",
                ))
            })?
            .clone();
        let items = parse_lsjson_items_value(list).map_err(|error| {
            RcloneCliError::Io(std::io::Error::other(format!(
                "Invalid rclone rc operations/list item payload: {error}"
            )))
        })?;
        cloud_entries_from_lsjson_items(path, items, "rclone rc operations/list")
            .map_err(|error| RcloneCliError::Io(std::io::Error::other(error)))
    }

    pub(super) fn stat_path_via_rc(
        &self,
        path: &CloudPath,
        options: RcloneReadOptions<'_>,
    ) -> Result<Option<CloudEntry>, RcloneCliError> {
        let fs_spec = format!("{}:", path.remote());
        let response = self.rc.operations_stat_with_options(
            &fs_spec,
            path.rel_path(),
            options.rc_timeout,
            options.cancel,
        )?;
        let item_value = response.get("item").cloned().unwrap_or(Value::Null);
        if item_value.is_null() {
            return Ok(None);
        }
        let item = parse_lsjson_stat_item_value(item_value).map_err(|error| {
            RcloneCliError::Io(std::io::Error::other(format!(
                "Invalid rclone rc operations/stat item payload: {error}"
            )))
        })?;
        Ok(Some(cloud_entry_from_item(path, item)))
    }

    fn list_dir_via_cli(
        &self,
        path: &CloudPath,
        options: RcloneReadOptions<'_>,
        fell_back_from_rc: bool,
        _fallback_reason: Option<&'static str>,
    ) -> CloudCommandResult<Vec<CloudEntry>> {
        let output = self.cli.run_capture_text_with_cancel_and_timeout(
            RcloneCommandSpec::new(RcloneSubcommand::LsJson).arg(path.to_rclone_remote_spec()),
            options.cancel,
            options.cli_timeout,
        );
        let output = match output {
            Ok(output) => output,
            Err(error) => {
                if fell_back_from_rc {
                    warn!(
                        path = %path,
                        error = %error,
                        "rclone CLI fallback failed after rc list degradation"
                    );
                }
                return Err(map_rclone_error_for_remote(path.remote(), error));
            }
        };
        let items = parse_lsjson_items(&output.stdout)?;
        cloud_entries_from_lsjson_items(path, items, "rclone lsjson")
    }

    fn stat_path_via_cli(
        &self,
        path: &CloudPath,
        options: RcloneReadOptions<'_>,
        fell_back_from_rc: bool,
        fallback_reason: Option<&'static str>,
    ) -> CloudCommandResult<Option<CloudEntry>> {
        let spec = RcloneCommandSpec::new(RcloneSubcommand::LsJson)
            .arg("--stat")
            .arg(path.to_rclone_remote_spec());
        match self.cli.run_capture_text_with_cancel_and_timeout(
            spec,
            options.cancel,
            options.cli_timeout,
        ) {
            Ok(output) => {
                let item = parse_lsjson_stat_item(&output.stdout)?;
                Ok(Some(cloud_entry_from_item(path, item)))
            }
            Err(RcloneCliError::NonZero { stderr, stdout, .. })
                if is_rclone_not_found_text(&stderr, &stdout) =>
            {
                Ok(None)
            }
            Err(error) => {
                if fell_back_from_rc {
                    warn!(
                        path = %path,
                        error = %error,
                        "rclone CLI fallback failed after rc stat degradation"
                    );
                }
                let _ = fallback_reason;
                Err(map_rclone_error_for_remote(path.remote(), error))
            }
        }
    }
}

pub(super) fn cloud_entry_from_item(path: &CloudPath, item: LsJsonItem) -> CloudEntry {
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

pub(super) fn normalize_cloud_modified_time_value(value: &str) -> String {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| {
            dt.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|_| value.to_string())
}

fn cloud_entries_from_lsjson_items(
    path: &CloudPath,
    items: Vec<LsJsonItem>,
    source: &str,
) -> CloudCommandResult<Vec<CloudEntry>> {
    let mut entries = Vec::with_capacity(items.len());
    for item in items {
        let child_path = path.child_path(&item.name).map_err(|error| {
            CloudCommandError::new(
                CloudCommandErrorCode::InvalidPath,
                format!("Invalid entry name from {source}: {error}"),
            )
        })?;
        entries.push(cloud_entry_from_item(&child_path, item));
    }
    sort_cloud_entries(&mut entries);
    Ok(entries)
}

fn normalize_cloud_modified_time(raw: Option<String>) -> Option<String> {
    raw.map(|value| normalize_cloud_modified_time_value(&value))
}

fn sort_cloud_entries(entries: &mut [CloudEntry]) {
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
}

fn cloud_read_cancelled(cancel: Option<&std::sync::atomic::AtomicBool>) -> bool {
    cancel
        .map(|token| token.load(std::sync::atomic::Ordering::SeqCst))
        .unwrap_or(false)
}

fn cloud_read_cancelled_error() -> CloudCommandError {
    CloudCommandError::new(
        CloudCommandErrorCode::Cancelled,
        "Cloud folder loading cancelled",
    )
}
