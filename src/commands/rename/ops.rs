use super::{
    error::{RenameError, RenameErrorCode, RenameResult},
    RenameEntryRequest,
};
use crate::{
    fs_utils::sanitize_path_nofollow,
    path_guard::{ensure_existing_dir_nonsymlink, ensure_existing_path_nonsymlink},
    undo::{
        assert_path_snapshot, is_destination_exists_error, move_with_fallback, run_actions,
        snapshot_existing_path, Action, Direction, UndoState,
    },
};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

fn build_rename_target(from: &Path, new_name: &str) -> RenameResult<PathBuf> {
    if new_name.trim().is_empty() {
        return Err(RenameError::invalid_input("New name cannot be empty"));
    }
    let parent = from
        .parent()
        .ok_or_else(|| RenameError::new(RenameErrorCode::RenameFailed, "Cannot rename root"))?;
    Ok(parent.join(new_name.trim()))
}

fn prepare_rename_pair(path: &str, new_name: &str) -> RenameResult<(PathBuf, PathBuf)> {
    let from = sanitize_path_nofollow(path, true).map_err(RenameError::from)?;
    let to = build_rename_target(&from, new_name)?;
    Ok((from, to))
}

fn apply_rename(from: &Path, to: &Path) -> RenameResult<()> {
    ensure_existing_path_nonsymlink(from).map_err(RenameError::from)?;
    let from_snapshot = snapshot_existing_path(from).map_err(RenameError::from)?;
    if let Some(parent) = to.parent() {
        ensure_existing_dir_nonsymlink(parent).map_err(RenameError::from)?;
        let parent_snapshot = snapshot_existing_path(parent).map_err(RenameError::from)?;
        assert_path_snapshot(parent, &parent_snapshot).map_err(RenameError::from)?;
    } else {
        return Err(RenameError::new(
            RenameErrorCode::InvalidPath,
            "Invalid destination path",
        ));
    }
    assert_path_snapshot(from, &from_snapshot).map_err(RenameError::from)?;
    match move_with_fallback(from, to) {
        Ok(_) => Ok(()),
        Err(e) if is_destination_exists_error(&e) => Err(RenameError::new(
            RenameErrorCode::TargetExists,
            "A file or directory with that name already exists",
        )),
        Err(e) => Err(RenameError::new(
            RenameErrorCode::RenameFailed,
            format!("Failed to rename: {e}"),
        )),
    }
}

pub(crate) fn rename_entry_impl(
    path: &str,
    new_name: &str,
    state: &UndoState,
) -> RenameResult<String> {
    let (from, to) = prepare_rename_pair(path, new_name)?;
    apply_rename(&from, &to)?;
    let _ = state.record_applied(Action::Rename {
        from: from.clone(),
        to: to.clone(),
    });
    Ok(to.to_string_lossy().to_string())
}

pub(crate) fn rename_entries_impl(
    entries: Vec<RenameEntryRequest>,
    undo: &UndoState,
) -> RenameResult<Vec<String>> {
    if entries.is_empty() {
        return Ok(Vec::new());
    }

    let mut pairs: Vec<(PathBuf, PathBuf)> = Vec::with_capacity(entries.len());
    let mut seen_sources: HashSet<PathBuf> = HashSet::with_capacity(entries.len());
    let mut seen_targets: HashSet<PathBuf> = HashSet::with_capacity(entries.len());

    for (idx, entry) in entries.into_iter().enumerate() {
        let (from, to) = prepare_rename_pair(entry.path.as_str(), entry.new_name.as_str())?;

        if !seen_sources.insert(from.clone()) {
            return Err(RenameError::new(
                RenameErrorCode::DuplicateSourcePath,
                format!("Duplicate source path in request (item {})", idx + 1),
            ));
        }
        if !seen_targets.insert(to.clone()) {
            return Err(RenameError::new(
                RenameErrorCode::DuplicateTargetName,
                format!("Duplicate target name in request (item {})", idx + 1),
            ));
        }

        pairs.push((from, to));
    }

    let mut performed: Vec<Action> = Vec::new();
    let mut renamed_paths: Vec<String> = Vec::with_capacity(pairs.len());

    for (from, to) in pairs {
        if from == to {
            continue;
        }
        if let Err(err) = apply_rename(&from, &to) {
            if !performed.is_empty() {
                let mut rollback = performed.clone();
                if let Err(rb_err) = run_actions(&mut rollback, Direction::Backward) {
                    return Err(RenameError::new(
                        RenameErrorCode::RollbackFailed,
                        format!("{}; rollback also failed: {}", err, rb_err),
                    ));
                }
            }
            return Err(err);
        }

        renamed_paths.push(to.to_string_lossy().to_string());
        performed.push(Action::Rename {
            from: from.clone(),
            to: to.clone(),
        });
    }

    if !performed.is_empty() {
        let recorded = if performed.len() == 1 {
            performed.pop().expect("single action should exist")
        } else {
            Action::Batch(performed)
        };
        let _ = undo.record_applied(recorded);
    }

    Ok(renamed_paths)
}
