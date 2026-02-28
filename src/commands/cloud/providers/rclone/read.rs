use super::{
    error::{is_rclone_not_found_text, map_rclone_error_for_remote},
    logging::{classify_rc_fallback_reason, log_backend_selected},
    parse::{
        parse_lsjson_items, parse_lsjson_items_value, parse_lsjson_stat_item,
        parse_lsjson_stat_item_value, LsJsonItem,
    },
    CloudCapabilities, CloudCommandError, CloudCommandErrorCode, CloudCommandResult, CloudEntry,
    CloudEntryKind, CloudPath, RcloneCliError, RcloneCloudProvider, RcloneCommandSpec,
    RcloneSubcommand,
};
use chrono::{DateTime, Local};
use serde_json::Value;
use tracing::debug;

impl RcloneCloudProvider {
    pub(super) fn list_dir_impl(&self, path: &CloudPath) -> CloudCommandResult<Vec<CloudEntry>> {
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
                    debug!(
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
        let entries =
            cloud_entries_from_lsjson_items(path, items, "rclone lsjson").map_err(|message| {
                CloudCommandError::new(CloudCommandErrorCode::InvalidPath, message)
            })?;
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
    ) -> CloudCommandResult<Option<CloudEntry>> {
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
                    debug!(
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

    pub(super) fn list_dir_via_rc(
        &self,
        path: &CloudPath,
    ) -> Result<Vec<CloudEntry>, RcloneCliError> {
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
        let items = parse_lsjson_items_value(list).map_err(|message| {
            RcloneCliError::Io(std::io::Error::other(format!(
                "Invalid rclone rc operations/list item payload: {message}"
            )))
        })?;
        cloud_entries_from_lsjson_items(path, items, "rclone rc operations/list")
            .map_err(|message| RcloneCliError::Io(std::io::Error::other(message)))
    }

    pub(super) fn stat_path_via_rc(
        &self,
        path: &CloudPath,
    ) -> Result<Option<CloudEntry>, RcloneCliError> {
        let fs_spec = format!("{}:", path.remote());
        let response = self.rc.operations_stat(&fs_spec, path.rel_path())?;
        let item_value = response.get("item").cloned().unwrap_or(Value::Null);
        if item_value.is_null() {
            return Ok(None);
        }
        let item = parse_lsjson_stat_item_value(item_value).map_err(|message| {
            RcloneCliError::Io(std::io::Error::other(format!(
                "Invalid rclone rc operations/stat item payload: {message}"
            )))
        })?;
        Ok(Some(cloud_entry_from_item(path, item)))
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
) -> Result<Vec<CloudEntry>, String> {
    let mut entries = Vec::with_capacity(items.len());
    for item in items {
        let child_path = path
            .child_path(&item.name)
            .map_err(|error| format!("Invalid entry name from {source}: {error}"))?;
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
