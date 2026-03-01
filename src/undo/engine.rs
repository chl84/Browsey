use super::nofollow::delete_entry_nofollow_io;
use super::path_ops::{copy_entry, delete_entry_path, move_with_fallback};
use super::{Action, Direction};
use crate::undo::error::UndoErrorCode;
use crate::undo::{UndoError, UndoResult};
use std::fs;

#[cfg(target_os = "windows")]
use crate::fs_utils::check_no_symlink_components;
#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStrExt;
#[cfg(target_os = "windows")]
use std::path::Path;
#[cfg(target_os = "windows")]
use windows_sys::Win32::Storage::FileSystem::{
    GetFileAttributesW, SetFileAttributesW, FILE_ATTRIBUTE_HIDDEN,
};

pub(crate) fn run_actions(actions: &mut [Action], direction: Direction) -> UndoResult<()> {
    execute_batch(actions, direction)
}

pub(super) fn execute_action(action: &mut Action, direction: Direction) -> UndoResult<()> {
    match action {
        Action::Batch(actions) => execute_batch(actions, direction),
        Action::Rename { from, to } | Action::Move { from, to } => {
            let (src, dst) = match direction {
                Direction::Forward => (from, to),
                Direction::Backward => (to, from),
            };
            move_with_fallback(src, dst)
        }
        Action::Copy { from, to } => match direction {
            Direction::Forward => copy_entry(from, to),
            Direction::Backward => delete_entry_path(to),
        },
        Action::Create { path, backup } => match direction {
            Direction::Forward => move_with_fallback(backup, path),
            Direction::Backward => {
                let parent = backup
                    .parent()
                    .ok_or_else(|| UndoError::invalid_input("Invalid backup path"))?;
                fs::create_dir_all(parent).map_err(|e| {
                    UndoError::from_io_error(
                        format!("Failed to create backup dir {}", parent.display()),
                        e,
                    )
                })?;
                move_with_fallback(path, backup)
            }
        },
        Action::Delete { path, backup } => match direction {
            Direction::Forward => {
                let parent = backup
                    .parent()
                    .ok_or_else(|| UndoError::invalid_input("Invalid backup path"))?;
                fs::create_dir_all(parent).map_err(|e| {
                    UndoError::from_io_error(
                        format!("Failed to create backup dir {}", parent.display()),
                        e,
                    )
                })?;
                move_with_fallback(path, backup)
            }
            Direction::Backward => move_with_fallback(backup, path),
        },
        #[cfg(target_os = "windows")]
        Action::SetHidden { path, hidden } => {
            let next = match direction {
                Direction::Forward => *hidden,
                Direction::Backward => !*hidden,
            };
            set_windows_hidden_attr(path, next)
        }
        Action::CreateFolder { path } => match direction {
            Direction::Forward => Ok(fs::create_dir(&*path).map_err(|e| {
                UndoError::from_io_error(
                    format!("Failed to create directory {}", path.display()),
                    e,
                )
            })?),
            Direction::Backward => match delete_entry_nofollow_io(path) {
                Ok(()) => Ok(()),
                Err(err) if err.code() == UndoErrorCode::NotFound => Ok(()),
                Err(err) => Err(err),
            },
        },
    }
}

#[cfg(target_os = "windows")]
fn set_windows_hidden_attr(path: &Path, hidden: bool) -> UndoResult<()> {
    check_no_symlink_components(path)?;
    let no_follow = fs::symlink_metadata(path).map_err(|e| {
        UndoError::from_io_error(format!("Failed to read metadata for {}", path.display()), e)
    })?;
    if no_follow.file_type().is_symlink() {
        return Err(UndoError::new(
            super::error::UndoErrorCode::SymlinkUnsupported,
            format!("Symlinks are not allowed: {}", path.display()),
        ));
    }

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let attrs = unsafe { GetFileAttributesW(wide.as_ptr()) };
    if attrs == u32::MAX {
        return Err(UndoError::new(
            super::error::UndoErrorCode::IoError,
            format!("GetFileAttributes failed for {}", path.display()),
        ));
    }

    let is_hidden = attrs & FILE_ATTRIBUTE_HIDDEN != 0;
    if is_hidden == hidden {
        return Ok(());
    }

    let mut next = attrs;
    if hidden {
        next |= FILE_ATTRIBUTE_HIDDEN;
    } else {
        next &= !FILE_ATTRIBUTE_HIDDEN;
    }
    let ok = unsafe { SetFileAttributesW(wide.as_ptr(), next) };
    if ok == 0 {
        return Err(UndoError::new(
            super::error::UndoErrorCode::IoError,
            format!("SetFileAttributes failed for {}", path.display()),
        ));
    }
    Ok(())
}

fn execute_batch(actions: &mut [Action], direction: Direction) -> UndoResult<()> {
    let order: Vec<usize> = match direction {
        Direction::Forward => (0..actions.len()).collect(),
        Direction::Backward => (0..actions.len()).rev().collect(),
    };

    let mut completed: Vec<usize> = Vec::with_capacity(order.len());
    for idx in order {
        if let Err(err) = execute_action(&mut actions[idx], direction) {
            let rollback_direction = reverse_direction(direction);
            let mut rollback_errors = Vec::new();
            for rollback_idx in completed.into_iter().rev() {
                if let Err(rollback_err) =
                    execute_action(&mut actions[rollback_idx], rollback_direction)
                {
                    rollback_errors.push(format!(
                        "rollback action {} failed: {}",
                        rollback_idx + 1,
                        rollback_err
                    ));
                }
            }
            if rollback_errors.is_empty() {
                return Err(err.with_context(format!("Batch action {} failed", idx + 1)));
            } else {
                return Err(UndoError::new(
                    err.code(),
                    format!(
                        "Batch action {} failed: {}; additional rollback issues: {}",
                        idx + 1,
                        err,
                        rollback_errors.join("; ")
                    ),
                ));
            }
        }
        completed.push(idx);
    }
    Ok(())
}

fn reverse_direction(direction: Direction) -> Direction {
    match direction {
        Direction::Forward => Direction::Backward,
        Direction::Backward => Direction::Forward,
    }
}
