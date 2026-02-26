use crate::commands::cloud;
use crate::commands::cloud::path::CloudPath;
use crate::commands::cloud::types::CloudEntryKind;
use crate::errors::api_error::{ApiError, ApiResult};
use crate::fs_utils::sanitize_path_follow;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MixedTransferConflictInfo {
    pub src: String,
    pub target: String,
    pub exists: bool,
    pub is_dir: bool,
}

#[tauri::command]
pub async fn preview_mixed_transfer_conflicts(
    sources: Vec<String>,
    dest_dir: String,
) -> ApiResult<Vec<MixedTransferConflictInfo>> {
    if sources.is_empty() {
        return Ok(Vec::new());
    }

    let dest_is_cloud = is_cloud_path(&dest_dir);
    let source_cloud_count = sources.iter().filter(|p| is_cloud_path(p)).count();
    if source_cloud_count > 0 && source_cloud_count < sources.len() {
        return Err(api_err(
            "unsupported",
            "Mixed local/cloud selection is not supported",
        ));
    }

    match (source_cloud_count == sources.len(), dest_is_cloud) {
        (false, true) => preview_local_to_cloud_conflicts(sources, dest_dir).await,
        (true, false) => preview_cloud_to_local_conflicts(sources, dest_dir),
        (false, false) => Err(api_err(
            "unsupported",
            "Use local clipboard preview for local-to-local transfers",
        )),
        (true, true) => Err(api_err(
            "unsupported",
            "Use cloud clipboard preview for cloud-to-cloud transfers",
        )),
    }
}

fn is_cloud_path(path: &str) -> bool {
    path.starts_with("rclone://")
}

fn api_err(code: &str, message: impl Into<String>) -> ApiError {
    ApiError::new(code, message.into())
}

async fn preview_local_to_cloud_conflicts(
    sources: Vec<String>,
    dest_dir: String,
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
    let dest_entries = cloud::list_cloud_entries(dest.to_string()).await?;
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

fn local_leaf_name(path: &Path) -> ApiResult<&str> {
    path.file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            api_err(
                "invalid_path",
                "Local source path does not have a file name",
            )
        })
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

    #[test]
    fn local_leaf_name_rejects_root_like_path() {
        let err = local_leaf_name(Path::new("/")).expect_err("should fail");
        assert_eq!(err.code, "invalid_path");
    }

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
}
