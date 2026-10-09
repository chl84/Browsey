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
    let _backup_use = super::write_backups(&[dest])?;
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
    options.read(true).write(true).create_new(true);
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
    let mut writer = crate::fs_utils::DigestWriter::new(&mut dst_file);
    let copied = io::copy(&mut src_file, &mut writer).map_err(|e| {
        UndoError::from_io_error(
            format!(
                "Failed to copy file {} -> {}; source retained, partial destination may remain",
                src.display(),
                dest.display()
            ),
            e,
        )
    })?;
    let digest = writer.digest();
    drop(writer);
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
    crate::fs_utils::verify_copy_content(&dst_file, dest, &completed_state, copied, &digest, || Ok(()))
        .map_err(|error| UndoError::from_io_error(
            "Copied content verification failed; source retained, destination retained for inspection", error
        ))?;
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
    let _backup_use = super::write_backups(&[path])?;
    let snapshot = snapshot_existing_path(path)?;
    assert_path_snapshot(path, &snapshot)?;
    delete_entry_nofollow_io(path)
}

pub fn move_with_fallback(src: &Path, dst: &Path) -> UndoResult<()> {
    let _backup_use = super::write_backups(&[src, dst])?;
    if cfg!(target_os = "linux") && is_mtp_case_only_rename(src, dst) {
        return move_case_only_via_temporary(src, dst, move_single_with_fallback);
    }
    move_single_with_fallback(src, dst)
}

fn is_mtp_case_only_rename(src: &Path, dst: &Path) -> bool {
    let mut previous_is_gvfs = false;
    let mtp = src.components().any(|part| {
        let name = part.as_os_str().to_string_lossy();
        let found = previous_is_gvfs && name.starts_with("mtp:host=");
        previous_is_gvfs = name == "gvfs";
        found
    });
    mtp && src.is_absolute()
        && src != dst
        && src.parent() == dst.parent()
        && src
            .file_name()
            .map(|name| name.to_string_lossy().to_lowercase())
            == dst
                .file_name()
                .map(|name| name.to_string_lossy().to_lowercase())
}

fn move_case_only_via_temporary(
    src: &Path,
    dst: &Path,
    mut move_one: impl FnMut(&Path, &Path) -> UndoResult<()>,
) -> UndoResult<()> {
    // GVFS/MTP can return the renamed object twice after a direct case-only
    // rename. Two distinct names avoid that alias cache without relaxing any
    // existing no-follow/no-replace checks, including during Undo/Redo.
    ensure_existing_dir_nonsymlink(
        dst.parent()
            .ok_or_else(|| UndoError::invalid_input("Invalid rename destination"))?,
    )?;
    match fs::symlink_metadata(dst) {
        Ok(_) => {
            return Err(UndoError::target_exists(
                "Rename destination already exists",
            ))
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(UndoError::from_io_error(
                "Inspect rename destination",
                error,
            ))
        }
    }
    static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let temporary = src
        .parent()
        .ok_or_else(|| UndoError::invalid_input("Cannot rename root"))?
        .join(format!(
            ".browsey-rename-{}-{nanos}-{sequence}",
            std::process::id()
        ));
    for (from, to) in [(src, temporary.as_path()), (temporary.as_path(), dst)] {
        move_one(from, to).map_err(|error| UndoError::new(UndoErrorCode::IoError, format!(
            "Case-only MTP rename may be incomplete. Inspect {}, {} and {} before retrying. No automatic retry or rollback was attempted: {error}",
            src.display(), temporary.display(), dst.display(),
        )))?;
    }
    Ok(())
}

fn move_single_with_fallback(src: &Path, dst: &Path) -> UndoResult<()> {
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
    copy_entry_recorded(src, dst).and_then(|receipt| {
        assert_path_snapshot(src, src_snapshot)?;
        #[cfg(test)]
        crate::fs_utils::copy_test_hooks::hit(src, dst, crate::fs_utils::copy_test_hooks::Phase::BeforeSourceDelete, 0)
            .map_err(|error| UndoError::from_io_error("Prepare fallback source deletion", error))?;
        receipt.verify(dst).map_err(|error| error.with_context(format!(
            "Fallback target changed or unverifiable; source not removed, copied output retained at {}", dst.display()
        )))?;
        source_tree.remove_recorded(src).map_err(|del_err| {
            // Per-entry removal can fail after some source children are gone.
            // The destination may now be the only complete copy: never remove it.
            UndoError::from_io_error(
                format!(
                    "Copied {} -> {} after fallback move, but failed to remove all source entries; destination retained. Inspect remaining source entries before retrying",
                    src.display(),
                    dst.display()
                ),
                del_err,
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

#[cfg(all(test, target_os = "linux"))]
mod case_rename_tests {
    use super::*;

    #[test]
    fn mtp_case_route_requires_same_parent_and_only_a_casing_change() {
        let src = Path::new("/runtime/gvfs/mtp:host=fixture/owned/report.txt");
        assert!(is_mtp_case_only_rename(
            src,
            Path::new("/runtime/gvfs/mtp:host=fixture/owned/Report.txt")
        ));
        for destination in [
            src,
            Path::new("/runtime/gvfs/mtp:host=fixture/other/Report.txt"),
            Path::new("/runtime/gvfs/mtp:host=fixture/owned/other.txt"),
        ] {
            assert!(!is_mtp_case_only_rename(src, destination));
        }
        for root in [
            "/owned",
            "/runtime/gvfs/sftp:host=fixture",
            "/runtime/gvfs/mtp-fake:host=fixture",
        ] {
            assert!(!is_mtp_case_only_rename(
                &Path::new(root).join("report.txt"),
                &Path::new(root).join("Report.txt")
            ));
        }
    }

    #[test]
    fn case_rename_preserves_nested_bytes_and_failed_stage_recovery_without_retry() {
        for failed_stage in [0, 1, 2] {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "browsey-mtp-case-{}-{nanos}-{failed_stage}",
                std::process::id()
            ));
            fs::create_dir(&root).unwrap();
            let (src, dst) = (root.join("tree"), root.join("TREE"));
            fs::create_dir(&src).unwrap();
            fs::create_dir(src.join("empty")).unwrap();
            fs::write(src.join("nested.txt"), b"preserved").unwrap();
            fs::write(root.join("unrelated.txt"), b"unrelated").unwrap();
            let mut calls = 0;
            let result = move_case_only_via_temporary(&src, &dst, |from, to| {
                calls += 1;
                assert_ne!(
                    from.file_name().unwrap().to_string_lossy().to_lowercase(),
                    to.file_name().unwrap().to_string_lossy().to_lowercase()
                );
                if calls == failed_stage {
                    return Err(UndoError::target_exists("injected occupied destination"));
                }
                move_single_with_fallback(from, to)
            });
            let entries = fs::read_dir(&root)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .collect::<Vec<_>>();
            let tree = entries.iter().find(|path| path.is_dir()).unwrap();
            let bytes = fs::read(tree.join("nested.txt")).unwrap();
            let empty = tree.join("empty").is_dir();
            let unrelated = fs::read(root.join("unrelated.txt")).unwrap();
            let recovery_name = tree.file_name().unwrap().to_string_lossy().to_string();
            fs::remove_dir_all(&root).unwrap();
            assert_eq!(bytes, b"preserved");
            assert!(empty);
            assert_eq!(unrelated, b"unrelated");
            assert_eq!(entries.len(), 2);
            if failed_stage == 0 {
                result.unwrap();
                assert_eq!(calls, 2);
                assert_eq!(tree, &dst);
            } else {
                let error = result.unwrap_err();
                assert_eq!(calls, failed_stage);
                assert_eq!(error.code(), UndoErrorCode::IoError);
                assert!(error.to_string().contains("may be incomplete"));
                assert!(error.to_string().contains(&recovery_name));
                assert!(error.to_string().contains("No automatic retry or rollback"));
                if failed_stage == 1 {
                    assert_eq!(tree, &src);
                } else {
                    assert!(recovery_name.starts_with(".browsey-rename-"));
                }
            }
        }
    }

    #[test]
    fn case_rename_refuses_existing_destination_before_the_first_write() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "browsey-mtp-collision-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        let (src, dst) = (root.join("file"), root.join("FILE"));
        fs::write(&src, b"source").unwrap();
        fs::write(&dst, b"occupied").unwrap();
        let result = move_case_only_via_temporary(&src, &dst, |_, _| {
            panic!("A collision must stop before any move")
        });
        let source = fs::read(&src).unwrap();
        let occupied = fs::read(&dst).unwrap();
        fs::remove_dir_all(root).unwrap();
        assert_eq!(result.unwrap_err().code(), UndoErrorCode::TargetExists);
        assert_eq!(source, b"source");
        assert_eq!(occupied, b"occupied");
    }
}
