use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::{debug, warn};

use crate::undo::UndoResult;

/// Best-effort cleanup of stale `.browsey-undo` directories. Runs at startup to
/// avoid leaving orphaned backups after a crash or restart (undo history is
/// in-memory only).
pub fn cleanup_stale_backups(max_age: Option<Duration>) {
    let _ = max_age; // keep the signature; we remove everything regardless.
    let base = base_undo_dir();

    if let Err(e) = validate_undo_dir(&base) {
        warn!("Skip cleanup; unsafe undo dir {:?}: {}", base, e);
        return;
    }

    if base.exists() {
        match fs::read_dir(&base) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let res = if path.is_dir() {
                        fs::remove_dir_all(&path)
                    } else {
                        fs::remove_file(&path)
                    };
                    if let Err(err) = res {
                        warn!("Failed to remove {:?}: {}", path, err);
                    }
                }
                debug!("Cleaned contents of backup directory {:?}", base);
            }
            Err(e) => warn!("Failed to read backup directory {:?}: {}", base, e),
        }
    }

    if let Err(e) = fs::create_dir_all(&base) {
        warn!("Failed to ensure backup directory {:?}: {}", base, e);
    }
}

#[allow(dead_code)]
pub fn temp_backup_path(original: &Path) -> PathBuf {
    let base = base_undo_dir();
    let _ = fs::create_dir_all(&base);

    // Use a hash of the full path to group files from the same directory while avoiding long names.
    let mut hasher = DefaultHasher::new();
    original.hash(&mut hasher);
    let bucket = format!("{:016x}", hasher.finish());

    let name = original
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_else(|| "item".into());

    let name_str: &str = name.as_ref();
    let mut candidate = base.join(&bucket).join(std::path::Path::new(name_str));
    let mut idx = 1u32;
    while candidate.exists() {
        let with_idx = format!("{}-{}", name, idx);
        candidate = base.join(&bucket).join(std::path::Path::new(&with_idx));
        idx += 1;
    }
    candidate
}

fn base_undo_dir() -> PathBuf {
    if let Ok(custom) = std::env::var("BROWSEY_UNDO_DIR") {
        return PathBuf::from(custom);
    }
    default_undo_dir()
}

fn default_undo_dir() -> PathBuf {
    dirs_next::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("browsey")
        .join("undo")
}

fn validate_undo_dir(path: &Path) -> UndoResult<()> {
    if cfg!(test) {
        return Ok(());
    }
    if !path.is_absolute() {
        return Err("Undo directory must be an absolute path".into());
    }
    if path.parent().is_none() {
        return Err("Undo directory cannot be the filesystem root".into());
    }
    let default_parent = default_undo_dir()
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("/"));
    if !path.starts_with(&default_parent) {
        return Err(format!(
            "Undo directory must reside under {}",
            default_parent.display()
        )
        .into());
    }
    Ok(())
}
