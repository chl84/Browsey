use std::{
    env,
    path::{Path, PathBuf},
};

use crate::fs_utils::sanitize_path_nofollow;

use super::{CompressError, CompressResult};

pub(super) fn ensure_same_parent(paths: &[PathBuf]) -> CompressResult<PathBuf> {
    let mut parent: Option<PathBuf> = None;
    for p in paths {
        match p.parent() {
            Some(par) => match parent {
                Some(ref prev) if prev != par => {
                    return Err(CompressError::from_external_message(
                        "All items must be in the same folder to compress together",
                    ))
                }
                Some(_) => {}
                None => parent = Some(par.to_path_buf()),
            },
            None => {
                return Err(CompressError::from_external_message(
                    "Cannot compress filesystem root",
                ))
            }
        }
    }
    parent.ok_or_else(|| CompressError::from_external_message("Missing parent for paths"))
}

pub(super) fn resolve_input_path(raw: &str) -> CompressResult<PathBuf> {
    let pb = sanitize_path_nofollow(raw, true).map_err(CompressError::from)?;
    let abs = if pb.is_absolute() {
        pb
    } else {
        env::current_dir()
            .map_err(|e| {
                CompressError::from_external_message(format!(
                    "Failed to resolve current directory: {e}"
                ))
            })?
            .join(pb)
    };
    Ok(abs)
}

fn safe_name(name: &str) -> CompressResult<String> {
    if name.trim().is_empty() {
        return Err(CompressError::from_external_message("Name cannot be empty"));
    }
    if name.contains(['/', '\\']) {
        return Err(CompressError::from_external_message(
            "Name cannot contain path separators",
        ));
    }
    Ok(name.to_string())
}

pub(super) fn destination_path(parent: &Path, name: &str, idx: usize) -> CompressResult<PathBuf> {
    let mut base = safe_name(name)?;
    let lower = base.to_lowercase();
    let has_zip = lower.ends_with(".zip");
    if !has_zip {
        base.push_str(".zip");
    }
    let stem = if has_zip {
        base[..base.len() - 4].to_string()
    } else {
        base.trim_end_matches(".zip").to_string()
    };
    let suffix = if idx == 0 {
        String::new()
    } else {
        format!(" ({idx})")
    };
    Ok(parent.join(format!("{stem}{suffix}.zip")))
}
