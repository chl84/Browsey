use std::fs;
use std::io::{self, ErrorKind};
use std::path::Path;

use super::nofollow::{delete_entry_nofollow_io, rename_nofollow_io};
use super::path_checks::{
    assert_path_snapshot, ensure_existing_dir_nonsymlink, ensure_existing_path_nonsymlink,
    snapshot_existing_path,
};
use super::types::{self, PathSnapshot};

pub(crate) fn copy_entry(src: &Path, dest: &Path) -> Result<(), String> {
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

fn copy_file_noreplace(src: &Path, dest: &Path) -> Result<(), String> {
    let mut src_file =
        fs::File::open(src).map_err(|e| format!("Failed to open source file {:?}: {e}", src))?;
    let mut dst_file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dest)
        .map_err(|e| {
            if e.kind() == ErrorKind::AlreadyExists {
                format!("Destination already exists: {}", dest.display())
            } else {
                format!("Failed to create destination file {:?}: {e}", dest)
            }
        })?;
    io::copy(&mut src_file, &mut dst_file)
        .map_err(|e| format!("Failed to copy file {:?} -> {:?}: {e}", src, dest))?;
    let perms = src_file
        .metadata()
        .map_err(|e| format!("Failed to read source permissions {:?}: {e}", src))?
        .permissions();
    fs::set_permissions(dest, perms)
        .map_err(|e| format!("Failed to set permissions on {:?}: {e}", dest))
}

fn copy_dir(src: &Path, dest: &Path) -> Result<(), String> {
    let src_snapshot = snapshot_existing_path(src)?;
    if let Some(parent) = dest.parent() {
        ensure_existing_dir_nonsymlink(parent)?;
    }
    assert_path_snapshot(src, &src_snapshot)?;
    fs::create_dir(dest).map_err(|e| {
        if e.kind() == ErrorKind::AlreadyExists {
            format!("Destination already exists: {}", dest.display())
        } else {
            format!("Failed to create dir {:?}: {e}", dest)
        }
    })?;
    for entry in fs::read_dir(src).map_err(|e| format!("Failed to read dir {:?}: {e}", src))? {
        let entry = entry.map_err(|e| format!("Failed to read dir entry: {e}"))?;
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

pub(crate) fn delete_entry_path(path: &Path) -> Result<(), String> {
    let snapshot = snapshot_existing_path(path)?;
    assert_path_snapshot(path, &snapshot)?;
    delete_entry_nofollow_io(path).map_err(|e| format!("Failed to delete {}: {e}", path.display()))
}

pub fn move_with_fallback(src: &Path, dst: &Path) -> Result<(), String> {
    let src_meta = ensure_existing_path_nonsymlink(src)?;
    let src_snapshot = types::path_snapshot_from_meta(&src_meta);
    if let Some(parent) = dst.parent() {
        ensure_existing_dir_nonsymlink(parent)?;
        let parent_snapshot = snapshot_existing_path(parent)?;
        assert_path_snapshot(parent, &parent_snapshot)?;
    } else {
        return Err("Invalid destination path".into());
    }
    assert_path_snapshot(src, &src_snapshot)?;
    match rename_nofollow_io(src, dst) {
        Ok(_) => Ok(()),
        Err(rename_err) => {
            if !is_cross_device(&rename_err) && !is_noreplace_unsupported(&rename_err) {
                return Err(format!(
                    "Failed to rename {} -> {}: {rename_err}",
                    src.display(),
                    dst.display()
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
) -> Result<(), String> {
    // Controlled fallback when atomic no-replace rename is unavailable
    // (or across filesystems): copy + delete without destination overwrite.
    copy_entry(src, dst).and_then(|_| {
        assert_path_snapshot(src, src_snapshot)?;
        delete_entry_path(src).map_err(|del_err| {
            // Best effort: clean up destination if delete failed to avoid duplicates.
            let _ = delete_entry_path(dst);
            format!(
                "Copied {} -> {} after fallback move, but failed to delete source: {del_err}",
                src.display(),
                dst.display()
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

pub(crate) fn is_destination_exists_error(err: &str) -> bool {
    let lower = err.to_lowercase();
    lower.contains("destination already exists")
        || lower.contains("already exists")
        || lower.contains("file exists")
        || lower.contains("os error 17")
        || lower.contains("os error 183")
}
