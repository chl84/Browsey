use crate::fs_utils::sanitize_path_follow;
use std::{fs, path::Path};

use super::ClipboardMode;

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
) -> Result<ClipboardMode, String> {
    if prefer_copy {
        return Ok(ClipboardMode::Copy);
    }
    if paths.is_empty() {
        return Err("No source paths provided".into());
    }

    let dest = sanitize_path_follow(&dest, false)?;
    let dest_meta =
        fs::symlink_metadata(&dest).map_err(|e| format!("Failed to read destination: {e}"))?;
    if !dest_meta.is_dir() {
        return Err("Drop destination must be a directory".into());
    }

    let mut src_paths = Vec::with_capacity(paths.len());
    for raw in paths {
        src_paths.push(sanitize_path_follow(&raw, true)?);
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
) -> Result<String, String> {
    let mode = resolve_drop_clipboard_mode_impl(paths, dest, prefer_copy)?;
    Ok(match mode {
        ClipboardMode::Copy => "copy",
        ClipboardMode::Cut => "cut",
    }
    .to_string())
}
