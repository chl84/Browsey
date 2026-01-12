use std::fs;
use std::path::{Path, PathBuf};

pub fn sanitize_path_follow(raw: &str, forbid_root: bool) -> Result<PathBuf, String> {
    let pb = PathBuf::from(raw);
    if !pb.exists() {
        return Err("Path does not exist".into());
    }
    let canon = pb
        .canonicalize()
        .map_err(|e| format!("Failed to canonicalize path: {e}"))?;
    if forbid_root && canon.parent().is_none() {
        return Err("Refusing to operate on filesystem root".into());
    }
    Ok(canon)
}

pub fn sanitize_path_nofollow(raw: &str, forbid_root: bool) -> Result<PathBuf, String> {
    let pb = PathBuf::from(raw);
    let meta = fs::symlink_metadata(&pb).map_err(|e| format!("Path does not exist or unreadable: {e}"))?;
    if forbid_root && pb.parent().is_none() {
        return Err("Refusing to operate on filesystem root".into());
    }
    let _ = meta;
    Ok(pb)
}

pub fn unique_path(dest: &Path) -> PathBuf {
    if !dest.exists() {
        return dest.to_path_buf();
    }
    let mut idx = 1;
    let stem = dest
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "item".to_string());
    let ext = dest.extension().map(|e| e.to_string_lossy().to_string());
    let parent = dest.parent().unwrap_or_else(|| Path::new("."));
    loop {
        let mut candidate = parent.join(format!("{}-{}", stem, idx));
        if let Some(ext) = &ext {
            candidate.set_extension(ext);
        }
        if !candidate.exists() {
            return candidate;
        }
        idx += 1;
    }
}
