use crate::commands::cloud;
use crate::commands::cloud::path::CloudPath;
use crate::commands::cloud::types::CloudEntryKind;
use crate::errors::api_error::{ApiError, ApiResult};
use crate::fs_utils::sanitize_path_follow;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub(super) enum MixedTransferRoute {
    LocalToCloud {
        sources: Vec<PathBuf>,
        dest_dir: CloudPath,
    },
    CloudToLocal {
        sources: Vec<CloudPath>,
        dest_dir: PathBuf,
    },
}

#[derive(Debug, Clone, Copy)]
pub(super) enum MixedRouteHint {
    LocalToCloud,
    CloudToLocal,
    LocalToLocal,
    CloudToCloud,
    MixedSelection,
    Unknown,
}

#[derive(Debug, Clone)]
pub(super) struct MixedTransferPair {
    pub(super) src: LocalOrCloudArg,
    pub(super) dst: LocalOrCloudArg,
    pub(super) cloud_remote_for_error_mapping: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) enum LocalOrCloudArg {
    Local(PathBuf),
    Cloud(CloudPath),
}

impl LocalOrCloudArg {
    pub(super) fn to_os_arg(&self) -> OsString {
        match self {
            Self::Local(path) => path.as_os_str().to_os_string(),
            Self::Cloud(path) => OsString::from(path.to_rclone_remote_spec()),
        }
    }

    pub(super) fn local_path(&self) -> Option<&Path> {
        match self {
            Self::Local(path) => Some(path.as_path()),
            Self::Cloud(_) => None,
        }
    }

    pub(super) fn cloud_path(&self) -> Option<&CloudPath> {
        match self {
            Self::Cloud(path) => Some(path),
            Self::Local(_) => None,
        }
    }
}

pub(super) fn api_err(code: &str, message: impl Into<String>) -> ApiError {
    ApiError::new(code, message.into())
}

pub(super) fn is_cloud_path(path: &str) -> bool {
    path.starts_with("rclone://")
}

pub(super) fn mixed_route_hint(sources: &[String], dest_dir: &str) -> MixedRouteHint {
    if sources.is_empty() {
        return MixedRouteHint::Unknown;
    }
    let dest_is_cloud = is_cloud_path(dest_dir);
    let source_cloud_count = sources.iter().filter(|p| is_cloud_path(p)).count();
    match (source_cloud_count, sources.len(), dest_is_cloud) {
        (0, _, true) => MixedRouteHint::LocalToCloud,
        (n, total, false) if n == total => MixedRouteHint::CloudToLocal,
        (0, _, false) => MixedRouteHint::LocalToLocal,
        (n, total, true) if n == total => MixedRouteHint::CloudToCloud,
        (n, total, _) if n > 0 && n < total => MixedRouteHint::MixedSelection,
        _ => MixedRouteHint::Unknown,
    }
}

pub(super) fn route_hint_label(hint: MixedRouteHint) -> &'static str {
    match hint {
        MixedRouteHint::LocalToCloud => "local_to_cloud",
        MixedRouteHint::CloudToLocal => "cloud_to_local",
        MixedRouteHint::LocalToLocal => "local_to_local",
        MixedRouteHint::CloudToCloud => "cloud_to_cloud",
        MixedRouteHint::MixedSelection => "mixed_selection",
        MixedRouteHint::Unknown => "unknown",
    }
}

pub(super) async fn validate_mixed_transfer_route(
    sources: Vec<String>,
    dest_dir: String,
) -> ApiResult<MixedTransferRoute> {
    if sources.is_empty() {
        return Err(api_err("invalid_input", "No sources provided"));
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
        (false, true) => validate_local_to_cloud_route(sources, dest_dir).await,
        (true, false) => validate_cloud_to_local_route(sources, dest_dir).await,
        (false, false) => Err(api_err(
            "unsupported",
            "Use local clipboard paste for local-to-local transfers",
        )),
        (true, true) => Err(api_err(
            "unsupported",
            "Use cloud clipboard paste for cloud-to-cloud transfers",
        )),
    }
}

pub(super) async fn validate_mixed_transfer_pair(
    src: String,
    dst: String,
) -> ApiResult<MixedTransferPair> {
    let src_is_cloud = is_cloud_path(&src);
    let dst_is_cloud = is_cloud_path(&dst);
    match (src_is_cloud, dst_is_cloud) {
        (false, true) => {
            let src_path = sanitize_path_follow(&src, true)
                .map_err(|e| api_err("invalid_path", e.to_string()))?;
            let src_meta = fs::symlink_metadata(&src_path)
                .map_err(|e| api_err("io_error", format!("Failed to read source metadata: {e}")))?;
            if src_meta.file_type().is_symlink() {
                return Err(api_err(
                    "symlink_unsupported",
                    "Symlinks are not supported for mixed local/cloud transfers yet",
                ));
            }

            let dst_path = CloudPath::parse(&dst).map_err(|e| {
                api_err(
                    "invalid_path",
                    format!("Invalid cloud destination path: {e}"),
                )
            })?;
            if dst_path.is_root() {
                return Err(api_err(
                    "invalid_path",
                    "Cloud destination path must include a file or folder name",
                ));
            }
            let Some(parent) = dst_path.parent_dir_path() else {
                return Err(api_err(
                    "invalid_path",
                    "Invalid cloud destination parent path",
                ));
            };
            if !parent.is_root() {
                match cloud::stat_cloud_entry(parent.to_string()).await? {
                    Some(entry) if matches!(entry.kind, CloudEntryKind::Dir) => {}
                    Some(_) => {
                        return Err(api_err(
                            "invalid_path",
                            "Cloud destination parent must be a directory",
                        ))
                    }
                    None => {
                        return Err(api_err(
                            "not_found",
                            "Cloud destination parent was not found",
                        ))
                    }
                }
            }
            Ok(MixedTransferPair {
                src: LocalOrCloudArg::Local(src_path),
                dst: LocalOrCloudArg::Cloud(dst_path.clone()),
                cloud_remote_for_error_mapping: Some(dst_path.remote().to_string()),
            })
        }
        (true, false) => {
            let src_path = CloudPath::parse(&src)
                .map_err(|e| api_err("invalid_path", format!("Invalid cloud source path: {e}")))?;
            match cloud::stat_cloud_entry(src_path.to_string()).await? {
                Some(entry) if matches!(entry.kind, CloudEntryKind::File | CloudEntryKind::Dir) => {
                }
                Some(_) => return Err(api_err("unsupported", "Unsupported cloud entry type")),
                None => return Err(api_err("not_found", "Cloud source was not found")),
            }

            let dst_path = sanitize_local_target_path_allow_missing(&dst)?;
            let parent = dst_path.parent().ok_or_else(|| {
                api_err(
                    "invalid_path",
                    "Local destination path must include a parent directory",
                )
            })?;
            let parent_meta = fs::symlink_metadata(parent).map_err(|e| {
                api_err(
                    "io_error",
                    format!("Failed to read destination parent metadata: {e}"),
                )
            })?;
            if !parent_meta.is_dir() {
                return Err(api_err(
                    "invalid_path",
                    "Local destination parent must be a directory",
                ));
            }

            Ok(MixedTransferPair {
                src: LocalOrCloudArg::Cloud(src_path.clone()),
                dst: LocalOrCloudArg::Local(dst_path),
                cloud_remote_for_error_mapping: Some(src_path.remote().to_string()),
            })
        }
        (false, false) => Err(api_err(
            "unsupported",
            "Use local clipboard paste for local-to-local transfers",
        )),
        (true, true) => Err(api_err(
            "unsupported",
            "Use cloud clipboard paste for cloud-to-cloud transfers",
        )),
    }
}

pub(super) fn sanitize_local_target_path_allow_missing(raw: &str) -> ApiResult<PathBuf> {
    let pb = PathBuf::from(raw);
    let file_name = pb
        .file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty() && *s != "." && *s != "..")
        .ok_or_else(|| {
            api_err(
                "invalid_path",
                "Local destination path must include a file or folder name",
            )
        })?;
    let parent_raw = pb.parent().ok_or_else(|| {
        api_err(
            "invalid_path",
            "Local destination path must include a parent directory",
        )
    })?;
    let parent = sanitize_path_follow(&parent_raw.to_string_lossy(), false)
        .map_err(|e| api_err("invalid_path", e.to_string()))?;
    Ok(parent.join(file_name))
}

pub(super) async fn validate_local_to_cloud_route(
    sources: Vec<String>,
    dest_dir: String,
) -> ApiResult<MixedTransferRoute> {
    let dest = CloudPath::parse(&dest_dir).map_err(|e| {
        api_err(
            "invalid_path",
            format!("Invalid cloud destination path: {e}"),
        )
    })?;

    if !dest.is_root() {
        match cloud::stat_cloud_entry(dest.to_string()).await? {
            Some(entry) if matches!(entry.kind, CloudEntryKind::Dir) => {}
            Some(_) => {
                return Err(api_err(
                    "invalid_path",
                    "Cloud destination must be a directory",
                ))
            }
            None => {
                return Err(api_err(
                    "not_found",
                    "Cloud destination directory was not found",
                ))
            }
        }
    }

    let mut local_sources = Vec::with_capacity(sources.len());
    for raw in sources {
        let path =
            sanitize_path_follow(&raw, true).map_err(|e| api_err("invalid_path", e.to_string()))?;
        let meta = fs::symlink_metadata(&path)
            .map_err(|e| api_err("io_error", format!("Failed to read source metadata: {e}")))?;
        if meta.file_type().is_symlink() {
            return Err(api_err(
                "symlink_unsupported",
                "Symlinks are not supported for mixed local/cloud transfers yet",
            ));
        }
        local_sources.push(path);
    }

    Ok(MixedTransferRoute::LocalToCloud {
        sources: local_sources,
        dest_dir: dest,
    })
}

pub(super) async fn validate_cloud_to_local_route(
    sources: Vec<String>,
    dest_dir: String,
) -> ApiResult<MixedTransferRoute> {
    let dest = sanitize_path_follow(&dest_dir, false)
        .map_err(|e| api_err("invalid_path", e.to_string()))?;
    let dest_meta = fs::symlink_metadata(&dest).map_err(|e| {
        api_err(
            "io_error",
            format!("Failed to read destination metadata: {e}"),
        )
    })?;
    if !dest_meta.is_dir() {
        return Err(api_err(
            "invalid_path",
            "Local destination must be a directory",
        ));
    }

    let mut cloud_sources = Vec::with_capacity(sources.len());
    for raw in sources {
        let path = CloudPath::parse(&raw)
            .map_err(|e| api_err("invalid_path", format!("Invalid cloud source path: {e}")))?;
        match cloud::stat_cloud_entry(path.to_string()).await? {
            Some(entry) if matches!(entry.kind, CloudEntryKind::File | CloudEntryKind::Dir) => {
                cloud_sources.push(path)
            }
            None => return Err(api_err("not_found", "Cloud source was not found")),
            Some(_) => return Err(api_err("unsupported", "Unsupported cloud entry type")),
        }
    }

    Ok(MixedTransferRoute::CloudToLocal {
        sources: cloud_sources,
        dest_dir: dest,
    })
}

pub(super) fn local_leaf_name(path: &Path) -> ApiResult<&str> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    #[test]
    fn local_leaf_name_rejects_root_like_path() {
        let err = local_leaf_name(Path::new("/")).expect_err("should fail");
        assert_eq!(err.code, "invalid_path");
    }

    #[test]
    fn sanitize_local_target_path_allow_missing_accepts_missing_leaf_when_parent_exists() {
        let base = std::env::temp_dir().join(format!(
            "browsey-transfer-target-sanitize-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&base).expect("create temp dir");
        let target = base.join("new-file.txt");

        let out = sanitize_local_target_path_allow_missing(&target.to_string_lossy())
            .expect("should allow missing leaf");
        assert_eq!(out, target);

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn validate_local_to_cloud_route_allows_directory_source() {
        let base = std::env::temp_dir().join(format!(
            "browsey-transfer-dir-reject-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("epoch")
                .as_nanos()
        ));
        let src_dir = base.join("folder");
        fs::create_dir_all(&src_dir).expect("create dir source");
        let result = tauri::async_runtime::block_on(validate_local_to_cloud_route(
            vec![src_dir.to_string_lossy().to_string()],
            "rclone://work".to_string(),
        ));
        let route = result.expect("directory mixed local->cloud should be accepted");
        match route {
            MixedTransferRoute::LocalToCloud { sources, .. } => {
                assert_eq!(sources, vec![src_dir.clone()]);
            }
            _ => panic!("expected local->cloud route"),
        }
        fs::remove_dir_all(&base).ok();
    }
}
