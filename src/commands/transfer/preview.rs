use super::logging::log_mixed_preview_result;
use super::route::{api_err, is_cloud_path, local_leaf_name, mixed_route_hint};
use super::MixedTransferConflictInfo;
use crate::commands::cloud;
use crate::commands::cloud::path::CloudPath;
use crate::commands::cloud::types::{CloudEntryKind, CloudProviderKind};
use crate::errors::api_error::ApiResult;
use crate::fs_utils::sanitize_path_follow;
use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub(super) async fn preview_mixed_transfer_conflicts(
    sources: Vec<String>,
    dest_dir: String,
    app: tauri::AppHandle,
) -> ApiResult<Vec<MixedTransferConflictInfo>> {
    let started = Instant::now();
    let source_count = sources.len();
    let route_hint = mixed_route_hint(&sources, &dest_dir);
    if sources.is_empty() {
        let result = Ok(Vec::new());
        log_mixed_preview_result(&result, route_hint, source_count, started);
        return result;
    }

    let dest_is_cloud = is_cloud_path(&dest_dir);
    let source_cloud_count = sources.iter().filter(|p| is_cloud_path(p)).count();
    if source_cloud_count > 0 && source_cloud_count < sources.len() {
        let result = Err(api_err(
            "unsupported",
            "Mixed local/cloud selection is not supported",
        ));
        log_mixed_preview_result(&result, route_hint, source_count, started);
        return result;
    }

    let result = match (source_cloud_count == sources.len(), dest_is_cloud) {
        (false, true) => preview_local_to_cloud_conflicts(sources, dest_dir, app).await,
        (true, false) => preview_cloud_to_local_conflicts(sources, dest_dir),
        (false, false) => Err(api_err(
            "unsupported",
            "Use local clipboard preview for local-to-local transfers",
        )),
        (true, true) => Err(api_err(
            "unsupported",
            "Use cloud clipboard preview for cloud-to-cloud transfers",
        )),
    };
    log_mixed_preview_result(&result, route_hint, source_count, started);
    result
}

async fn preview_local_to_cloud_conflicts(
    sources: Vec<String>,
    dest_dir: String,
    app: tauri::AppHandle,
) -> ApiResult<Vec<MixedTransferConflictInfo>> {
    let dest = CloudPath::parse(&dest_dir).map_err(|e| {
        api_err(
            "invalid_path",
            format!("Invalid cloud destination path: {e}"),
        )
    })?;

    let local_sources = sources
        .into_iter()
        .map(|raw| {
            sanitize_path_follow(&raw, true).map_err(|e| api_err("invalid_path", e.to_string()))
        })
        .collect::<ApiResult<Vec<PathBuf>>>()?;

    let provider = cloud::cloud_provider_kind_for_remote(dest.remote());
    let dest_entries = cloud::list_cloud_entries(dest.to_string(), app).await?;
    build_local_to_cloud_conflicts_from_entries(local_sources, &dest, provider, &dest_entries)
}

fn build_local_to_cloud_conflicts_from_entries(
    local_sources: Vec<PathBuf>,
    dest: &CloudPath,
    provider: Option<CloudProviderKind>,
    dest_entries: &[crate::commands::cloud::types::CloudEntry],
) -> ApiResult<Vec<MixedTransferConflictInfo>> {
    let mut name_to_is_dir: HashMap<String, bool> = HashMap::with_capacity(dest_entries.len());
    for entry in dest_entries {
        let key = cloud::cloud_conflict_name_key(provider, &entry.name);
        name_to_is_dir
            .entry(key)
            .or_insert(matches!(entry.kind, CloudEntryKind::Dir));
    }

    let mut conflicts = Vec::new();
    for src in local_sources {
        let name = local_leaf_name(&src)?;
        let key = cloud::cloud_conflict_name_key(provider, name);
        let Some(is_dir) = name_to_is_dir.get(&key).copied() else {
            continue;
        };
        let target = dest
            .child_path(name)
            .map_err(|e| api_err("invalid_path", format!("Invalid cloud target path: {e}")))?;
        conflicts.push(MixedTransferConflictInfo {
            src: src.to_string_lossy().to_string(),
            target: target.to_string(),
            exists: true,
            is_dir,
        });
    }

    Ok(conflicts)
}

fn preview_cloud_to_local_conflicts(
    sources: Vec<String>,
    dest_dir: String,
) -> ApiResult<Vec<MixedTransferConflictInfo>> {
    let dest = sanitize_path_follow(&dest_dir, false)
        .map_err(|e| api_err("invalid_path", e.to_string()))?;
    let cloud_sources = sources
        .into_iter()
        .map(|raw| {
            CloudPath::parse(&raw)
                .map_err(|e| api_err("invalid_path", format!("Invalid cloud source path: {e}")))
        })
        .collect::<ApiResult<Vec<CloudPath>>>()?;

    let dest_entries = list_local_dir_entries(&dest)?;
    let mut name_to_is_dir: HashMap<String, bool> = HashMap::with_capacity(dest_entries.len());
    for (name, is_dir) in dest_entries {
        name_to_is_dir.entry(name).or_insert(is_dir);
    }

    let mut conflicts = Vec::new();
    for src in cloud_sources {
        let name = src
            .leaf_name()
            .map_err(|e| api_err("invalid_path", format!("Invalid cloud source path: {e}")))?;
        let Some(is_dir) = name_to_is_dir.get(name).copied() else {
            continue;
        };
        let target = dest.join(name);
        conflicts.push(MixedTransferConflictInfo {
            src: src.to_string(),
            target: target.to_string_lossy().to_string(),
            exists: true,
            is_dir,
        });
    }

    Ok(conflicts)
}

fn list_local_dir_entries(dest: &Path) -> ApiResult<Vec<(String, bool)>> {
    let mut out = Vec::new();
    let rd = fs::read_dir(dest).map_err(|e| {
        api_err(
            "io_error",
            format!("Failed to read destination directory: {e}"),
        )
    })?;
    for item in rd {
        let item =
            item.map_err(|e| api_err("io_error", format!("Failed to read directory entry: {e}")))?;
        let name = item.file_name();
        let Some(name) = name.to_str().map(|s| s.to_string()) else {
            continue;
        };
        let is_dir = match fs::symlink_metadata(item.path()) {
            Ok(meta) => meta.file_type().is_dir(),
            Err(e) if e.kind() == ErrorKind::NotFound => continue,
            Err(e) => {
                return Err(api_err(
                    "io_error",
                    format!("Failed to read destination entry metadata: {e}"),
                ))
            }
        };
        out.push((name, is_dir));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::cloud::types::{CloudCapabilities, CloudEntry};
    use std::fs;

    #[test]
    fn local_dir_entries_lists_files_and_dirs() {
        let base = std::env::temp_dir().join(format!(
            "browsey-transfer-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("epoch")
                .as_nanos()
        ));
        fs::create_dir(&base).expect("create temp base");
        let file_path = base.join("a.txt");
        let dir_path = base.join("folder");
        fs::write(&file_path, b"x").expect("write");
        fs::create_dir(&dir_path).expect("mkdir");

        let mut rows = list_local_dir_entries(&base).expect("list");
        rows.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(rows, vec![("a.txt".into(), false), ("folder".into(), true)]);

        fs::remove_file(&file_path).ok();
        fs::remove_dir(&dir_path).ok();
        fs::remove_dir(&base).ok();
    }

    fn cloud_entry(name: &str, kind: CloudEntryKind) -> CloudEntry {
        let provider = CloudProviderKind::Onedrive;
        CloudEntry {
            name: name.to_string(),
            path: format!("rclone://work/{name}"),
            kind,
            size: None,
            modified: None,
            capabilities: CloudCapabilities::v1_for_provider(provider),
        }
    }

    #[test]
    fn mixed_preview_local_to_cloud_matches_onedrive_case_insensitive_and_preserves_kind() {
        let base = std::env::temp_dir().join(format!(
            "browsey-transfer-preview-local-cloud-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&base).expect("create temp base");
        let local_file = base.join("Report.txt");
        let local_dir = base.join("FolderA");
        fs::write(&local_file, b"x").expect("write local file");
        fs::create_dir_all(&local_dir).expect("create local dir");

        let dest = CloudPath::parse("rclone://work/dest").expect("cloud path");
        let conflicts = build_local_to_cloud_conflicts_from_entries(
            vec![local_file.clone(), local_dir.clone()],
            &dest,
            Some(CloudProviderKind::Onedrive),
            &[
                cloud_entry("report.txt", CloudEntryKind::File),
                cloud_entry("FolderA", CloudEntryKind::Dir),
            ],
        )
        .expect("build conflicts");

        assert_eq!(conflicts.len(), 2);
        assert!(conflicts
            .iter()
            .any(|c| c.src == local_file.to_string_lossy() && !c.is_dir));
        assert!(conflicts
            .iter()
            .any(|c| c.src == local_dir.to_string_lossy() && c.is_dir));

        fs::remove_file(&local_file).ok();
        fs::remove_dir_all(&local_dir).ok();
        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn mixed_preview_cloud_to_local_reports_file_and_dir_conflicts() {
        let base = std::env::temp_dir().join(format!(
            "browsey-transfer-preview-cloud-local-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&base).expect("create temp base");
        let local_file = base.join("report.txt");
        let local_dir = base.join("FolderA");
        fs::write(&local_file, b"x").expect("write local file");
        fs::create_dir_all(&local_dir).expect("mkdir local dir");

        let conflicts = preview_cloud_to_local_conflicts(
            vec![
                "rclone://work/src/report.txt".to_string(),
                "rclone://work/src/FolderA".to_string(),
                "rclone://work/src/other.txt".to_string(),
            ],
            base.to_string_lossy().to_string(),
        )
        .expect("preview cloud->local conflicts");

        assert_eq!(conflicts.len(), 2);
        assert!(conflicts
            .iter()
            .any(|c| c.src.ends_with("/report.txt") && !c.is_dir));
        assert!(conflicts
            .iter()
            .any(|c| c.src.ends_with("/FolderA") && c.is_dir));

        fs::remove_file(&local_file).ok();
        fs::remove_dir_all(&local_dir).ok();
        fs::remove_dir_all(&base).ok();
    }
}
