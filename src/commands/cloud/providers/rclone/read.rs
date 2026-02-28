use super::{
    parse::{parse_lsjson_items_value, parse_lsjson_stat_item_value, LsJsonItem},
    CloudCapabilities, CloudEntry, CloudEntryKind, CloudPath, RcloneCliError, RcloneCloudProvider,
    Value,
};
use chrono::{DateTime, Local};

impl RcloneCloudProvider {
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

fn normalize_cloud_modified_time(raw: Option<String>) -> Option<String> {
    raw.map(|value| normalize_cloud_modified_time_value(&value))
}

pub(super) fn normalize_cloud_modified_time_value(value: &str) -> String {
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
