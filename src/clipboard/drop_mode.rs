use crate::fs_utils::sanitize_path_follow;
use std::{fs, path::Path};

use super::{
    error::{map_api_result, ClipboardError, ClipboardErrorCode, ClipboardResult},
    ClipboardMode,
};

#[cfg(unix)]
fn filesystem_key(path: &Path) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    fs::metadata(path).ok().map(|meta| meta.dev())
}

#[cfg(target_os = "windows")]
fn filesystem_key(path: &Path) -> Option<String> {
    use std::path::Component;

    let canon = path.canonicalize().ok()?;
    let mut comps = canon.components();
    match comps.next() {
        Some(Component::Prefix(prefix)) => {
            let mut key = prefix.as_os_str().to_string_lossy().to_string();
            key.make_ascii_lowercase();
            Some(key)
        }
        _ => None,
    }
}

fn should_copy_for_drop(src: &Path, dest: &Path) -> bool {
    match (filesystem_key(src), filesystem_key(dest)) {
        (Some(src_key), Some(dest_key)) => src_key != dest_key,
        _ => true,
    }
}

pub(super) fn resolve_drop_clipboard_mode_impl(
    paths: Vec<String>,
    dest: String,
    prefer_copy: bool,
) -> ClipboardResult<ClipboardMode> {
    if prefer_copy {
        return Ok(ClipboardMode::Copy);
    }
    if paths.is_empty() {
        return Err(ClipboardError::invalid_input("No source paths provided"));
    }

    let dest = sanitize_path_follow(&dest, false).map_err(ClipboardError::from_external_message)?;
    let dest_meta = fs::symlink_metadata(&dest).map_err(|e| {
        ClipboardError::new(
            ClipboardErrorCode::IoError,
            format!("Failed to read destination: {e}"),
        )
    })?;
    if !dest_meta.is_dir() {
        return Err(ClipboardError::new(
            ClipboardErrorCode::NotDirectory,
            "Drop destination must be a directory",
        ));
    }

    let mut src_paths = Vec::with_capacity(paths.len());
    for raw in paths {
        src_paths
            .push(sanitize_path_follow(&raw, true).map_err(ClipboardError::from_external_message)?);
    }

    if src_paths.iter().any(|src| should_copy_for_drop(src, &dest)) {
        Ok(ClipboardMode::Copy)
    } else {
        Ok(ClipboardMode::Cut)
    }
}

#[tauri::command]
pub fn resolve_drop_clipboard_mode(
    paths: Vec<String>,
    dest: String,
    prefer_copy: bool,
) -> crate::errors::api_error::ApiResult<String> {
    map_api_result(
        resolve_drop_clipboard_mode_impl(paths, dest, prefer_copy).map(|mode| {
            match mode {
                ClipboardMode::Copy => "copy",
                ClipboardMode::Cut => "cut",
            }
            .to_string()
        }),
    )
}
