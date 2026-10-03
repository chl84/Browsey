use std::fs;
use std::io::{self, ErrorKind};
use std::path::Path;

use crate::fs_utils::{FileIdentity, FileState, TreeSnapshot};
use crate::undo::error::UndoErrorCode;
use crate::undo::{UndoError, UndoResult};

use super::nofollow::{delete_entry_nofollow_io, rename_nofollow_io};
use super::path_checks::{
    assert_path_snapshot, ensure_existing_dir_nonsymlink, ensure_existing_path_nonsymlink,
    snapshot_existing_path,
};
use super::types::{self, PathSnapshot};

pub(crate) fn copy_entry(src: &Path, dest: &Path) -> UndoResult<()> {
    copy_entry_recorded(src, dest).map(|_| ())
}

pub(super) fn copy_entry_recorded(src: &Path, dest: &Path) -> UndoResult<super::CopyReceipt> {
    let mut outputs = TreeSnapshot::default();
    copy_entry_tracked(src, dest, dest, &mut outputs)?;
    Ok(super::CopyReceipt::from_snapshot(outputs))
}

fn copy_entry_tracked(
    src: &Path,
    dest: &Path,
    root: &Path,
    outputs: &mut TreeSnapshot,
) -> UndoResult<()> {
    let meta = ensure_existing_path_nonsymlink(src)?;
    let src_snapshot = types::path_snapshot_from_meta(&meta);
    if let Some(parent) = dest.parent() {
        ensure_existing_dir_nonsymlink(parent)?;
    }
    if meta.is_dir() {
        assert_path_snapshot(src, &src_snapshot)?;
        copy_dir(src, dest, root, outputs)
    } else {
        if let Some(parent) = dest.parent() {
            ensure_existing_dir_nonsymlink(parent)?;
        }
        assert_path_snapshot(src, &src_snapshot)?;
        let state = copy_file_noreplace_with_sync(src, dest, fs::File::sync_all)?;
        outputs.record_file(dest.strip_prefix(root).unwrap().into(), state);
        Ok(())
    }
}

pub(super) fn copy_file_noreplace_with_sync(
    src: &Path,
    dest: &Path,
    sync: impl FnOnce(&fs::File) -> io::Result<()>,
) -> UndoResult<FileState> {
    #[cfg_attr(test, allow(unused_mut))]
    let mut src_file = crate::fs_utils::open_regular_file_nofollow(src).map_err(|e| {
        UndoError::from_io_error(format!("Failed to open source file {}", src.display()), e)
    })?;
    let permissions = src_file
        .metadata()
        .map_err(|e| UndoError::from_io_error("Failed to read source permissions", e))?
        .permissions();
    let source_state = FileState::from_file(&src_file)
        .map_err(|error| UndoError::from_io_error("Snapshot open copy source", error))?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        options.mode(permissions.mode() & 0o777);
    }
    #[cfg_attr(test, allow(unused_mut))]
    let mut dst_file = options.open(dest).map_err(|e| {
        if e.kind() == ErrorKind::AlreadyExists {
            UndoError::target_exists(format!("Destination already exists: {}", dest.display()))
        } else {
            UndoError::from_io_error(
                format!("Failed to create destination file {}", dest.display()),
                e,
            )
        }
    })?;
    let target_identity = crate::fs_utils::FileIdentity::from_file(&dst_file);
    #[cfg(test)]
    let mut src_file = crate::fs_utils::copy_test_hooks::TestFile::new(src_file, src, dest);
    #[cfg(test)]
    let mut dst_file = crate::fs_utils::copy_test_hooks::TestFile::new(dst_file, src, dest);
    let copied = io::copy(&mut src_file, &mut dst_file).map_err(|e| {
        UndoError::from_io_error(
            format!(
                "Failed to copy file {} -> {}; source retained, partial destination may remain",
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
    dst_file.set_permissions(perms).map_err(|e| {
        UndoError::from_io_error(
            format!("Failed to set permissions on {}", dest.display()),
            e,
        )
    })?;
    let completed_state = FileState::from_file(&dst_file).map_err(|error| {
        UndoError::from_io_error("Snapshot written copy target; source retained", error)
    })?;
    // File::drop ignores close/writeback errors. Do not delete a move's source
    // until destination finalization has actually succeeded.
    let sync_result = (|| {
        #[cfg(test)]
        crate::fs_utils::copy_test_hooks::hit(
            src,
            dest,
            crate::fs_utils::copy_test_hooks::Phase::Sync,
            copied,
        )?;
        sync(&dst_file)?;
        #[cfg(test)]
        crate::fs_utils::copy_test_hooks::hit(
            src,
            dest,
            crate::fs_utils::copy_test_hooks::Phase::Synced,
            copied,
        )?;
        Ok::<_, io::Error>(())
    })();
    sync_result.map_err(|e| {
        UndoError::from_io_error(
            format!(
                "Failed to sync copied file {}; source retained, destination may remain",
                dest.display()
            ),
            e,
        )
    })?;
    if !target_identity
        .as_ref()
        .is_some_and(|identity| identity.matches(dest))
    {
        return Err(UndoError::new(
            UndoErrorCode::IoError,
            format!(
                "Cannot verify copied target {}; source retained",
                dest.display()
            ),
        ));
    }
    completed_state.verify_copied_file(&dst_file, dest, copied).map_err(|error| {
        UndoError::from_io_error("Copied target changed during finalization; source retained, destination retained for inspection", error)
    })?;
    source_state.verify_copied_file(&src_file, src, copied).map_err(|error| {
        UndoError::from_io_error("Source changed during copy; source not removed, destination retained for inspection", error)
    })?;
    Ok(completed_state)
}

fn copy_dir(src: &Path, dest: &Path, root: &Path, outputs: &mut TreeSnapshot) -> UndoResult<()> {
    let src_snapshot = snapshot_existing_path(src)?;
    if let Some(parent) = dest.parent() {
        ensure_existing_dir_nonsymlink(parent)?;
    }
    assert_path_snapshot(src, &src_snapshot)?;
    let permissions = fs::metadata(src)
        .map_err(|e| UndoError::from_io_error("Failed to read directory permissions", e))?
        .permissions();
    let mut builder = fs::DirBuilder::new();
    builder.recursive(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(dest).map_err(|e| {
        if e.kind() == ErrorKind::AlreadyExists {
            UndoError::target_exists(format!("Destination already exists: {}", dest.display()))
        } else {
            UndoError::from_io_error(format!("Failed to create dir {}", dest.display()), e)
        }
    })?;
    let identity = FileIdentity::capture(dest)
        .ok_or_else(|| UndoError::invalid_input("Cannot verify created directory identity"))?;
    outputs.record_directory(dest.strip_prefix(root).unwrap().into(), identity.clone());
    for entry in fs::read_dir(src)
        .map_err(|e| UndoError::from_io_error(format!("Failed to read dir {}", src.display()), e))?
    {
        let entry = entry.map_err(|e| UndoError::from_io_error("Failed to read dir entry", e))?;
        let path = entry.path();
        let meta = ensure_existing_path_nonsymlink(&path)?;
        let child_snapshot = types::path_snapshot_from_meta(&meta);
        let target = dest.join(entry.file_name());
        assert_path_snapshot(&path, &child_snapshot)?;
        copy_entry_tracked(&path, &target, root, outputs)?;
    }
    if !identity.matches(dest) {
        return Err(UndoError::invalid_input(
            "Copy destination directory changed; retained outputs",
        ));
    }
    fs::set_permissions(dest, permissions)
        .map_err(|e| UndoError::from_io_error("Failed to set directory permissions", e))
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
                return Err(rename_err);
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
    let source_tree = crate::fs_utils::TreeSnapshot::capture(src)
        .map_err(|error| UndoError::from_io_error("Snapshot source before fallback copy", error))?;
    copy_entry(src, dst).and_then(|_| {
        assert_path_snapshot(src, src_snapshot)?;
        source_tree.verify(src).map_err(|error| UndoError::from_io_error(
            format!("Source changed or could not be verified; completed copy retained at {}", dst.display()), error))?;
        delete_entry_path(src).map_err(|del_err| {
            // Recursive deletion can fail after some source children are gone.
            // The destination may now be the only complete copy: never remove it.
            UndoError::new(
                del_err.code(),
                format!(
                    "Copied {} -> {} after fallback move, but failed to delete source; destination retained: {del_err}",
                    src.display(),
                    dst.display()
                ),
            )
        })
    })
}

fn is_cross_device(err: &UndoError) -> bool {
    err.code() == UndoErrorCode::CrossDeviceMove
}

fn is_noreplace_unsupported(err: &UndoError) -> bool {
    err.code() == UndoErrorCode::AtomicRenameUnsupported
}

pub(crate) fn is_destination_exists_error(err: &UndoError) -> bool {
    err.code() == UndoErrorCode::TargetExists
}
