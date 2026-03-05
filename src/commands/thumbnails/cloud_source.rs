use super::{
    thumb_kind, ThumbKind, ThumbnailError, ThumbnailErrorCode, ThumbnailResult,
    ThumbnailRuntimeSettings, MAX_FILE_BYTES,
};
use crate::commands::cloud::path::CloudPath;
use crate::commands::cloud::provider::CloudProvider;
use crate::commands::cloud::types::CloudEntryKind;
use crate::commands::cloud::{
    configured_rclone_provider, materialize_cloud_file_for_local_use_with_snapshot,
    CloudCommandError, CloudCommandErrorCode, CloudMaterializeSnapshot,
};
use blake3::Hasher;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::AppHandle;

#[derive(Clone, Debug)]
pub(super) struct CloudThumbnailSource {
    pub(super) cloud_path: CloudPath,
    pub(super) snapshot: CloudMaterializeSnapshot,
}

pub(super) fn precheck_cloud_thumbnail_source(
    raw_path: &str,
    settings: &ThumbnailRuntimeSettings,
) -> ThumbnailResult<CloudThumbnailSource> {
    let cloud_path = CloudPath::parse(raw_path).map_err(|error| {
        ThumbnailError::new(
            ThumbnailErrorCode::InvalidInput,
            format!("Invalid cloud path: {error}"),
        )
    })?;
    let leaf = cloud_path.leaf_name().map_err(|error| {
        ThumbnailError::new(
            ThumbnailErrorCode::InvalidInput,
            format!("Invalid cloud path: {error}"),
        )
    })?;
    let ext = Path::new(leaf)
        .extension()
        .and_then(|part| part.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    validate_cloud_thumb_extension(settings, &ext)?;

    let provider = configured_rclone_provider().map_err(|error| {
        ThumbnailError::new(
            ThumbnailErrorCode::UnknownError,
            format!("Cloud thumbnail provider unavailable: {error}"),
        )
    })?;
    let entry = provider
        .stat_path(&cloud_path)
        .map_err(|error| map_cloud_command_error("Cloud thumbnail stat failed", error))?
        .ok_or_else(|| {
            ThumbnailError::new(
                ThumbnailErrorCode::NotFound,
                format!("Cloud file was not found: {cloud_path}"),
            )
        })?;
    if !matches!(entry.kind, CloudEntryKind::File) {
        return Err(ThumbnailError::new(
            ThumbnailErrorCode::InvalidInput,
            "Target is not a file",
        ));
    }
    // Precheck-only policy: this cloud-size guard is best-effort and not a strict
    // transaction across provider consistency windows.
    validate_cloud_thumb_size(entry.size)?;
    let snapshot = CloudMaterializeSnapshot {
        name: entry.name,
        size: entry.size,
        modified: entry.modified,
        kind: entry.kind,
    };
    Ok(CloudThumbnailSource {
        cloud_path,
        snapshot,
    })
}

pub(super) async fn materialize_cloud_thumbnail_source(
    app_handle: &AppHandle,
    source: &CloudThumbnailSource,
) -> ThumbnailResult<(PathBuf, fs::Metadata, ThumbKind, Option<PathBuf>)> {
    let cloud_path_for_task = source.cloud_path.clone();
    let snapshot_for_task = source.snapshot.clone();
    let app_for_task = app_handle.clone();
    let target = tauri::async_runtime::spawn_blocking(move || {
        materialize_cloud_file_for_local_use_with_snapshot(
            &cloud_path_for_task,
            &snapshot_for_task,
            &app_for_task,
            None,
            None,
        )
    })
    .await
    .map_err(|error| {
        ThumbnailError::new(
            ThumbnailErrorCode::Cancelled,
            format!("Cloud thumbnail materialization task cancelled: {error}"),
        )
    })?
    .map_err(|error| map_cloud_command_error("Cloud thumbnail materialization failed", error))?;
    let meta = fs::metadata(&target).map_err(|error| {
        ThumbnailError::new(
            ThumbnailErrorCode::DecodeFailed,
            format!("Failed to read cloud thumbnail metadata: {error}"),
        )
    })?;
    let kind = thumb_kind(&target);
    if matches!(kind, ThumbKind::Video) {
        return Err(ThumbnailError::new(
            ThumbnailErrorCode::UnsupportedFormat,
            "Unsupported cloud thumbnail extension: video",
        ));
    }
    Ok((target, meta, kind, None))
}

fn map_cloud_command_error(context: &str, error: CloudCommandError) -> ThumbnailError {
    ThumbnailError::new(
        map_cloud_command_error_code(error.code()),
        format!("{context}: {error}"),
    )
}

pub(super) fn map_cloud_command_error_code(code: CloudCommandErrorCode) -> ThumbnailErrorCode {
    match code {
        CloudCommandErrorCode::InvalidPath | CloudCommandErrorCode::DestinationExists => {
            ThumbnailErrorCode::InvalidInput
        }
        CloudCommandErrorCode::NotFound => ThumbnailErrorCode::NotFound,
        CloudCommandErrorCode::PermissionDenied => ThumbnailErrorCode::PermissionDenied,
        CloudCommandErrorCode::Unsupported => ThumbnailErrorCode::UnsupportedFormat,
        CloudCommandErrorCode::TaskFailed => ThumbnailErrorCode::CacheFailed,
        CloudCommandErrorCode::Timeout
        | CloudCommandErrorCode::NetworkError
        | CloudCommandErrorCode::TlsCertificateError
        | CloudCommandErrorCode::RateLimited
        | CloudCommandErrorCode::AuthRequired
        | CloudCommandErrorCode::BinaryMissing
        | CloudCommandErrorCode::InvalidConfig
        | CloudCommandErrorCode::UnknownError => ThumbnailErrorCode::UnknownError,
    }
}

pub(super) fn cache_key_for_cloud_source(source: &CloudThumbnailSource, max_dim: u32) -> String {
    let mut hasher = Hasher::new();
    hasher.update(b"cloud-thumb-v1");
    hasher.update(source.cloud_path.to_string().as_bytes());
    match source.snapshot.size {
        Some(size) => {
            hasher.update(&[1]);
            hasher.update(&size.to_le_bytes());
        }
        None => {
            hasher.update(&[0]);
        }
    }
    match source.snapshot.modified.as_deref() {
        Some(modified) => {
            hasher.update(&[1]);
            hasher.update(modified.as_bytes());
        }
        None => {
            hasher.update(&[0]);
        }
    }
    hasher.update(&max_dim.to_le_bytes());
    hasher.finalize().to_hex().to_string()
}

fn is_cloud_thumb_extension_allowed(ext: &str) -> bool {
    matches!(
        ext,
        "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "bmp"
            | "webp"
            | "tif"
            | "tiff"
            | "avif"
            | "heic"
            | "heif"
            | "svg"
            | "pdf"
    )
}

pub(super) fn validate_cloud_thumb_extension(
    settings: &ThumbnailRuntimeSettings,
    ext: &str,
) -> ThumbnailResult<()> {
    if !settings.cloud_thumbs {
        return Err(ThumbnailError::new(
            ThumbnailErrorCode::InvalidInput,
            "Cloud thumbnails disabled",
        ));
    }
    if ext.is_empty() || !is_cloud_thumb_extension_allowed(ext) {
        return Err(ThumbnailError::new(
            ThumbnailErrorCode::UnsupportedFormat,
            format!("Unsupported cloud thumbnail extension: {ext}"),
        ));
    }
    Ok(())
}

pub(super) fn validate_cloud_thumb_size(size: Option<u64>) -> ThumbnailResult<u64> {
    let size = size.ok_or_else(|| {
        ThumbnailError::new(
            ThumbnailErrorCode::InvalidInput,
            "Cloud thumbnail requires known file size",
        )
    })?;
    if size > MAX_FILE_BYTES {
        return Err(ThumbnailError::new(
            ThumbnailErrorCode::InvalidInput,
            format!(
                "Cloud file too large for thumbnail (>{} MB)",
                MAX_FILE_BYTES / 1024 / 1024
            ),
        ));
    }
    Ok(size)
}

#[cfg(test)]
mod tests {
    use super::{
        map_cloud_command_error_code, validate_cloud_thumb_extension, validate_cloud_thumb_size,
    };
    use crate::commands::cloud::CloudCommandErrorCode;
    use crate::commands::thumbnails::{
        ThumbnailErrorCode, ThumbnailRuntimeSettings, MAX_FILE_BYTES,
    };
    use crate::errors::domain::DomainError;

    #[test]
    fn cloud_thumb_guard_disabled_returns_invalid_input() {
        let settings = ThumbnailRuntimeSettings {
            cloud_thumbs: false,
            ..ThumbnailRuntimeSettings::default()
        };
        let err = validate_cloud_thumb_extension(&settings, "png").expect_err("should fail");
        assert_eq!(err.code_str(), "invalid_input");
        assert!(err.to_string().to_lowercase().contains("disabled"));
    }

    #[test]
    fn cloud_thumb_guard_unsupported_extension_returns_unsupported_format() {
        let settings = ThumbnailRuntimeSettings {
            cloud_thumbs: true,
            ..ThumbnailRuntimeSettings::default()
        };
        let err = validate_cloud_thumb_extension(&settings, "txt").expect_err("should fail");
        assert_eq!(err.code_str(), "unsupported_format");
        assert!(err.to_string().to_lowercase().contains("unsupported"));
    }

    #[test]
    fn cloud_thumb_guard_unknown_size_returns_invalid_input() {
        let err = validate_cloud_thumb_size(None).expect_err("should fail");
        assert_eq!(err.code_str(), "invalid_input");
        assert!(err.to_string().to_lowercase().contains("known file size"));
    }

    #[test]
    fn cloud_thumb_guard_over_limit_returns_invalid_input() {
        let err = validate_cloud_thumb_size(Some(MAX_FILE_BYTES + 1)).expect_err("should fail");
        assert_eq!(err.code_str(), "invalid_input");
        assert!(err.to_string().to_lowercase().contains("too large"));
    }

    #[test]
    fn cloud_thumb_mapping_preserves_not_found() {
        assert_eq!(
            map_cloud_command_error_code(CloudCommandErrorCode::NotFound),
            ThumbnailErrorCode::NotFound
        );
    }

    #[test]
    fn cloud_thumb_mapping_preserves_permission_denied() {
        assert_eq!(
            map_cloud_command_error_code(CloudCommandErrorCode::PermissionDenied),
            ThumbnailErrorCode::PermissionDenied
        );
    }
}
