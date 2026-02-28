use super::{
    cache::list_cloud_dir_cached,
    cloud_conflict_name_key, cloud_provider_kind_for_remote,
    error::{CloudCommandError, CloudCommandErrorCode, CloudCommandResult},
    map_spawn_result, parse_cloud_path_arg,
    path::CloudPath,
    types::{CloudConflictInfo, CloudEntry, CloudEntryKind, CloudProviderKind},
};
use std::collections::HashMap;
use std::time::Instant;
use tracing::{debug, info};

pub(super) async fn preview_cloud_conflicts_impl(
    sources: Vec<String>,
    dest_dir: String,
) -> CloudCommandResult<Vec<CloudConflictInfo>> {
    let started = Instant::now();
    let dest_dir = parse_cloud_path_arg(dest_dir)?;
    let sources = sources
        .into_iter()
        .map(parse_cloud_path_arg)
        .collect::<CloudCommandResult<Vec<_>>>()?;
    let source_count = sources.len();
    let dest_dir_for_log = dest_dir.clone();
    let task = tauri::async_runtime::spawn_blocking(move || {
        let provider = cloud_provider_kind_for_remote(dest_dir.remote());
        let dest_entries = list_cloud_dir_cached(&dest_dir)?;
        build_conflicts_from_dest_listing(&sources, &dest_dir, &dest_entries, provider)
    });
    let result = map_spawn_result(task.await, "cloud conflict preview task failed");
    let elapsed_ms = started.elapsed().as_millis() as u64;
    match &result {
        Ok(conflicts) => info!(
            op = "cloud_conflict_preview",
            dest_dir = %dest_dir_for_log,
            source_count,
            conflict_count = conflicts.len(),
            elapsed_ms,
            "cloud command timing"
        ),
        Err(error) => debug!(
            op = "cloud_conflict_preview",
            dest_dir = %dest_dir_for_log,
            source_count,
            elapsed_ms,
            error = %error,
            "cloud command failed"
        ),
    }
    result
}

pub(super) fn build_conflicts_from_dest_listing(
    sources: &[CloudPath],
    dest_dir: &CloudPath,
    dest_entries: &[CloudEntry],
    provider: Option<CloudProviderKind>,
) -> CloudCommandResult<Vec<CloudConflictInfo>> {
    let mut name_to_is_dir: HashMap<String, bool> = HashMap::with_capacity(dest_entries.len());
    for entry in dest_entries {
        name_to_is_dir
            .entry(cloud_conflict_name_key(provider, &entry.name))
            .or_insert(matches!(entry.kind, CloudEntryKind::Dir));
    }

    let mut conflicts = Vec::new();
    for src in sources {
        let name = src.leaf_name().map_err(|error| {
            CloudCommandError::new(
                CloudCommandErrorCode::InvalidPath,
                format!("Invalid source cloud path for conflict preview: {error}"),
            )
        })?;
        let key = cloud_conflict_name_key(provider, name);
        let Some(is_dir) = name_to_is_dir.get(&key) else {
            continue;
        };
        let target = dest_dir.child_path(name).map_err(|error| {
            CloudCommandError::new(
                CloudCommandErrorCode::InvalidPath,
                format!("Invalid target cloud path for conflict preview: {error}"),
            )
        })?;
        conflicts.push(CloudConflictInfo {
            src: src.to_string(),
            target: target.to_string(),
            exists: true,
            is_dir: *is_dir,
        });
    }
    Ok(conflicts)
}

#[cfg(test)]
mod tests {
    use super::build_conflicts_from_dest_listing;
    use crate::commands::cloud::{
        path::CloudPath,
        types::{CloudCapabilities, CloudEntry, CloudEntryKind, CloudProviderKind},
    };

    #[test]
    fn conflict_preview_uses_dest_listing_names() {
        let src_file = CloudPath::parse("rclone://work/src/report.txt").expect("src file");
        let src_dir = CloudPath::parse("rclone://work/src/Folder").expect("src dir");
        let src_missing = CloudPath::parse("rclone://work/src/notes.txt").expect("src missing");
        let dest_dir = CloudPath::parse("rclone://work/dest").expect("dest");
        let dest_entries = vec![
            CloudEntry {
                name: "report.txt".to_string(),
                path: "rclone://work/dest/report.txt".to_string(),
                kind: CloudEntryKind::File,
                size: Some(1),
                modified: None,
                capabilities: CloudCapabilities::v1_core_rw(),
            },
            CloudEntry {
                name: "Folder".to_string(),
                path: "rclone://work/dest/Folder".to_string(),
                kind: CloudEntryKind::Dir,
                size: None,
                modified: None,
                capabilities: CloudCapabilities::v1_core_rw(),
            },
        ];

        let conflicts = build_conflicts_from_dest_listing(
            &[src_file.clone(), src_dir.clone(), src_missing],
            &dest_dir,
            &dest_entries,
            None,
        )
        .expect("conflicts");

        assert_eq!(conflicts.len(), 2);
        assert_eq!(conflicts[0].src, src_file.to_string());
        assert_eq!(conflicts[0].target, "rclone://work/dest/report.txt");
        assert!(!conflicts[0].is_dir);
        assert_eq!(conflicts[1].src, src_dir.to_string());
        assert_eq!(conflicts[1].target, "rclone://work/dest/Folder");
        assert!(conflicts[1].is_dir);
    }

    #[test]
    fn conflict_preview_is_case_insensitive_for_onedrive_names() {
        let src_file = CloudPath::parse("rclone://work/src/report.txt").expect("src file");
        let dest_dir = CloudPath::parse("rclone://work/dest").expect("dest");
        let dest_entries = vec![CloudEntry {
            name: "Report.txt".to_string(),
            path: "rclone://work/dest/Report.txt".to_string(),
            kind: CloudEntryKind::File,
            size: Some(1),
            modified: None,
            capabilities: CloudCapabilities::v1_core_rw(),
        }];

        let conflicts = build_conflicts_from_dest_listing(
            std::slice::from_ref(&src_file),
            &dest_dir,
            &dest_entries,
            Some(CloudProviderKind::Onedrive),
        )
        .expect("conflicts");

        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].src, src_file.to_string());
        assert_eq!(conflicts[0].target, "rclone://work/dest/report.txt");
        assert!(!conflicts[0].is_dir);
    }
}
