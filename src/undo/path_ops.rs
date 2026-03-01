use std::fs;
use std::io::{self, ErrorKind};
use std::path::Path;

use crate::undo::{UndoError, UndoResult};
use crate::undo::error::UndoErrorCode;

use super::nofollow::{delete_entry_nofollow_io, rename_nofollow_io};
use super::path_checks::{
    assert_path_snapshot, ensure_existing_dir_nonsymlink, ensure_existing_path_nonsymlink,
    snapshot_existing_path,
};
use super::types::{self, PathSnapshot};

pub(crate) fn copy_entry(src: &Path, dest: &Path) -> UndoResult<()> {
    let meta = ensure_existing_path_nonsymlink(src)?;
    let src_snapshot = types::path_snapshot_from_meta(&meta);
    if let Some(parent) = dest.parent() {
        ensure_existing_dir_nonsymlink(parent)?;
    }
    if meta.is_dir() {
        assert_path_snapshot(src, &src_snapshot)?;
        copy_dir(src, dest)
    } else {
        if let Some(parent) = dest.parent() {
            ensure_existing_dir_nonsymlink(parent)?;
        }
        assert_path_snapshot(src, &src_snapshot)?;
        copy_file_noreplace(src, dest)
    }
}

fn copy_file_noreplace(src: &Path, dest: &Path) -> UndoResult<()> {
    let mut src_file = fs::File::open(src).map_err(|e| {
        UndoError::from_io_error(format!("Failed to open source file {}", src.display()), e)
    })?;
    let mut dst_file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dest)
        .map_err(|e| {
            if e.kind() == ErrorKind::AlreadyExists {
                UndoError::target_exists(format!("Destination already exists: {}", dest.display()))
            } else {
                UndoError::from_io_error(
                    format!("Failed to create destination file {}", dest.display()),
                    e,
                )
            }
        })?;
    io::copy(&mut src_file, &mut dst_file).map_err(|e| {
        UndoError::from_io_error(
            format!(
                "Failed to copy file {} -> {}",
                src.display(),
                dest.display()
            ),
            e,
        )
    })?;
    let perms = src_file
        .metadata()
        .map_err(|e| {
            UndoError::from_io_error(
                format!("Failed to read source permissions {}", src.display()),
                e,
            )
        })?
        .permissions();
    fs::set_permissions(dest, perms).map_err(|e| {
        UndoError::from_io_error(
            format!("Failed to set permissions on {}", dest.display()),
            e,
        )
    })
}

fn copy_dir(src: &Path, dest: &Path) -> UndoResult<()> {
    let src_snapshot = snapshot_existing_path(src)?;
    if let Some(parent) = dest.parent() {
        ensure_existing_dir_nonsymlink(parent)?;
    }
    assert_path_snapshot(src, &src_snapshot)?;
    fs::create_dir(dest).map_err(|e| {
        if e.kind() == ErrorKind::AlreadyExists {
            UndoError::target_exists(format!("Destination already exists: {}", dest.display()))
        } else {
            UndoError::from_io_error(format!("Failed to create dir {}", dest.display()), e)
        }
    })?;
    for entry in fs::read_dir(src)
        .map_err(|e| UndoError::from_io_error(format!("Failed to read dir {}", src.display()), e))?
    {
        let entry = entry.map_err(|e| UndoError::from_io_error("Failed to read dir entry", e))?;
        let path = entry.path();
        let meta = ensure_existing_path_nonsymlink(&path)?;
        let child_snapshot = types::path_snapshot_from_meta(&meta);
        let target = dest.join(entry.file_name());
        if meta.is_dir() {
            assert_path_snapshot(&path, &child_snapshot)?;
            copy_dir(&path, &target)?;
        } else {
            assert_path_snapshot(&path, &child_snapshot)?;
            copy_file_noreplace(&path, &target)?;
        }
    }
    Ok(())
}

pub(crate) fn delete_entry_path(path: &Path) -> UndoResult<()> {
    let snapshot = snapshot_existing_path(path)?;
    assert_path_snapshot(path, &snapshot)?;
    delete_entry_nofollow_io(path)
}

pub fn move_with_fallback(src: &Path, dst: &Path) -> UndoResult<()> {
    let src_meta = ensure_existing_path_nonsymlink(src)?;
    let src_snapshot = types::path_snapshot_from_meta(&src_meta);
    if let Some(parent) = dst.parent() {
        ensure_existing_dir_nonsymlink(parent)?;
        let parent_snapshot = snapshot_existing_path(parent)?;
        assert_path_snapshot(parent, &parent_snapshot)?;
    } else {
        return Err(UndoError::invalid_input("Invalid destination path"));
    }
    assert_path_snapshot(src, &src_snapshot)?;
    match rename_nofollow_io(src, dst) {
        Ok(_) => Ok(()),
        Err(rename_err) => {
            if !is_cross_device(&rename_err) && !is_noreplace_unsupported(&rename_err) {
                return Err(UndoError::from_io_error(
                    format!("Failed to rename {} -> {}", src.display(), dst.display()),
                    rename_err,
                ));
            }
            move_by_copy_delete_noreplace(src, dst, &src_snapshot)
        }
    }
}

pub(crate) fn move_by_copy_delete_noreplace(
    src: &Path,
    dst: &Path,
    src_snapshot: &PathSnapshot,
) -> UndoResult<()> {
    // Controlled fallback when atomic no-replace rename is unavailable
    // (or across filesystems): copy + delete without destination overwrite.
    copy_entry(src, dst).and_then(|_| {
        assert_path_snapshot(src, src_snapshot)?;
        delete_entry_path(src).map_err(|del_err| {
            // Best effort: clean up destination if delete failed to avoid duplicates.
            let _ = delete_entry_path(dst);
            UndoError::new(
                del_err.code(),
                format!(
                    "Copied {} -> {} after fallback move, but failed to delete source: {del_err}",
                    src.display(),
                    dst.display()
                ),
            )
        })
    })
}

fn is_cross_device(err: &std::io::Error) -> bool {
    matches!(err.raw_os_error(), Some(17) | Some(18))
}

fn is_noreplace_unsupported(err: &std::io::Error) -> bool {
    err.kind() == ErrorKind::Unsupported
}

pub(crate) fn is_destination_exists_error(err: &UndoError) -> bool {
    err.code() == UndoErrorCode::TargetExists
}
