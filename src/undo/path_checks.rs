use std::fs;
use std::path::Path;

use crate::fs_utils::check_no_symlink_components;
use crate::undo::UndoResult;

use super::types::{self, PathSnapshot};

pub(crate) fn snapshot_existing_path(path: &Path) -> UndoResult<PathSnapshot> {
    let meta = ensure_existing_path_nonsymlink(path)?;
    Ok(types::path_snapshot_from_meta(&meta))
}

pub(crate) fn assert_path_snapshot(path: &Path, expected: &PathSnapshot) -> UndoResult<()> {
    let meta = ensure_existing_path_nonsymlink(path)?;
    let current = types::path_snapshot_from_meta(&meta);
    if types::snapshots_match(expected, &current) {
        Ok(())
    } else {
        Err(format!("Path changed during operation: {}", path.display()).into())
    }
}

pub(super) fn ensure_existing_path_nonsymlink(path: &Path) -> UndoResult<fs::Metadata> {
    check_no_symlink_components(path)?;
    let meta = fs::symlink_metadata(path)
        .map_err(|e| format!("Failed to read metadata for {}: {e}", path.display()))?;
    if meta.file_type().is_symlink() {
        return Err(format!(
            "Refusing path with symlink target: {}",
            path.display()
        )
        .into());
    }
    Ok(meta)
}

pub(super) fn ensure_existing_dir_nonsymlink(path: &Path) -> UndoResult<()> {
    let meta = ensure_existing_path_nonsymlink(path)?;
    if !meta.is_dir() {
        return Err(format!("Expected directory path: {}", path.display()).into());
    }
    Ok(())
}
