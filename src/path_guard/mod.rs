use crate::fs_utils::check_no_symlink_components;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

mod error;

pub(crate) use error::{PathGuardError, PathGuardErrorCode, PathGuardResult};

pub(crate) fn ensure_existing_path_nonsymlink(path: &Path) -> PathGuardResult<fs::Metadata> {
    check_no_symlink_components(path)
        .map_err(|error| PathGuardError::new(PathGuardErrorCode::SymlinkUnsupported, error))?;
    let meta = fs::symlink_metadata(path).map_err(|error| {
        PathGuardError::from_io_error(
            &format!("Failed to read metadata for {}", path.display()),
            error,
        )
    })?;
    if meta.file_type().is_symlink() {
        return Err(PathGuardError::new(
            PathGuardErrorCode::SymlinkUnsupported,
            format!("Symlinks are not allowed: {}", path.display()),
        ));
    }
    Ok(meta)
}

pub(crate) fn ensure_existing_dir_nonsymlink(path: &Path) -> PathGuardResult<()> {
    let meta = ensure_existing_path_nonsymlink(path)?;
    if !meta.is_dir() {
        return Err(PathGuardError::new(
            PathGuardErrorCode::NotDirectory,
            "Destination is not a directory",
        ));
    }
    Ok(())
}

pub(crate) fn ensure_no_symlink_components_existing_prefix(path: &Path) -> PathGuardResult<()> {
    let mut acc = PathBuf::new();
    for comp in path.components() {
        match comp {
            std::path::Component::Prefix(p) => {
                acc.push(p.as_os_str());
                continue;
            }
            std::path::Component::RootDir => {
                acc.push(std::path::Component::RootDir.as_os_str());
                continue;
            }
            std::path::Component::CurDir => continue,
            std::path::Component::ParentDir => {
                acc.pop();
                continue;
            }
            std::path::Component::Normal(seg) => acc.push(seg),
        }
        if acc.as_os_str().is_empty() {
            continue;
        }
        match fs::symlink_metadata(&acc) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    return Err(PathGuardError::new(
                        PathGuardErrorCode::SymlinkUnsupported,
                        format!("Symlinks are not allowed in path: {}", acc.display()),
                    ));
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => break,
            Err(e) => {
                return Err(PathGuardError::from_io_error(
                    &format!("Failed to read metadata for {}", acc.display()),
                    e,
                ));
            }
        }
    }
    Ok(())
}
