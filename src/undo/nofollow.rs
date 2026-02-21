#[cfg(unix)]
use std::ffi::{CStr, CString};
#[cfg(any(target_os = "windows", not(all(unix, target_os = "linux"))))]
use std::fs;
use std::io::ErrorKind;
#[cfg(all(unix, target_os = "linux"))]
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};

use crate::undo::UndoResult;

#[cfg(not(all(unix, target_os = "linux")))]
use crate::fs_utils::check_no_symlink_components;

#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, ERROR_FILE_EXISTS};
#[cfg(target_os = "windows")]
use windows_sys::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_WRITE_THROUGH};

#[cfg(all(unix, target_os = "linux"))]
fn absolute_path(path: &Path) -> Result<PathBuf, std::io::Error> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

#[cfg(all(unix, target_os = "linux"))]
fn parent_fd_and_name(path: &Path) -> Result<(OwnedFd, CString), std::io::Error> {
    let abs = absolute_path(path)?;
    let parent = abs.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "Path is missing parent")
    })?;
    let name = abs.file_name().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Path is missing file name",
        )
    })?;
    let c_name = CString::new(name.as_bytes()).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Path contains invalid NUL byte",
        )
    })?;
    let parent_fd = open_nofollow_path_fd(parent).map_err(std::io::Error::other)?;
    Ok((parent_fd, c_name))
}

#[cfg(all(unix, target_os = "linux"))]
fn fstatat_nofollow(parent_fd: libc::c_int, name: &CString) -> Result<libc::stat, std::io::Error> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    let rc = unsafe {
        libc::fstatat(
            parent_fd,
            name.as_ptr(),
            stat.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if rc != 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(unsafe { stat.assume_init() })
    }
}

#[cfg(all(unix, target_os = "linux"))]
fn rename_noreplace_unsupported_error(code: Option<i32>) -> std::io::Error {
    let detail = code
        .map(|raw| format!(" (os error {raw})"))
        .unwrap_or_default();
    std::io::Error::new(
        ErrorKind::Unsupported,
        format!("RENAME_NOREPLACE is unavailable{detail}"),
    )
}

#[cfg(all(unix, target_os = "linux"))]
pub(super) fn rename_nofollow_io(src: &Path, dst: &Path) -> Result<(), std::io::Error> {
    if absolute_path(src)? == absolute_path(dst)? {
        return Ok(());
    }

    let (src_parent_fd, src_name) = parent_fd_and_name(src)?;
    let (dst_parent_fd, dst_name) = parent_fd_and_name(dst)?;

    let src_stat = fstatat_nofollow(src_parent_fd.as_raw_fd(), &src_name)?;
    if (src_stat.st_mode & libc::S_IFMT) == libc::S_IFLNK {
        return Err(std::io::Error::other("Symlinks are not allowed"));
    }

    let rc = unsafe {
        libc::syscall(
            libc::SYS_renameat2 as libc::c_long,
            src_parent_fd.as_raw_fd(),
            src_name.as_ptr(),
            dst_parent_fd.as_raw_fd(),
            dst_name.as_ptr(),
            libc::RENAME_NOREPLACE as libc::c_uint,
        )
    } as libc::c_int;
    if rc == 0 {
        Ok(())
    } else {
        let err = std::io::Error::last_os_error();
        match err.raw_os_error() {
            Some(code)
                if code == libc::ENOSYS
                    || code == libc::EINVAL
                    || code == libc::ENOTSUP
                    || code == libc::EOPNOTSUPP =>
            {
                Err(rename_noreplace_unsupported_error(Some(code)))
            }
            _ => Err(err),
        }
    }
}

#[cfg(target_os = "windows")]
pub(super) fn rename_nofollow_io(src: &Path, dst: &Path) -> Result<(), std::io::Error> {
    if src == dst {
        return Ok(());
    }

    // Validate source without following symlinks/reparse points via metadata.
    let src_meta = fs::symlink_metadata(src)?;
    if src_meta.file_type().is_symlink() {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "Symlinks are not allowed",
        ));
    }

    let src_wide: Vec<u16> = src
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let dst_wide: Vec<u16> = dst
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let ok = unsafe { MoveFileExW(src_wide.as_ptr(), dst_wide.as_ptr(), MOVEFILE_WRITE_THROUGH) };
    if ok != 0 {
        return Ok(());
    }

    let err = std::io::Error::last_os_error();
    match err.raw_os_error() {
        Some(code) if code == ERROR_ALREADY_EXISTS as i32 || code == ERROR_FILE_EXISTS as i32 => {
            Err(std::io::Error::new(
                ErrorKind::AlreadyExists,
                "Destination already exists",
            ))
        }
        _ => Err(err),
    }
}

#[cfg(all(unix, not(target_os = "linux")))]
pub(super) fn rename_nofollow_io(src: &Path, dst: &Path) -> Result<(), std::io::Error> {
    let _ = (src, dst);
    Err(std::io::Error::new(
        ErrorKind::Unsupported,
        "RENAME_NOREPLACE is unavailable on this platform",
    ))
}

#[cfg(not(any(unix, target_os = "windows")))]
pub(super) fn rename_nofollow_io(src: &Path, dst: &Path) -> Result<(), std::io::Error> {
    let _ = (src, dst);
    Err(std::io::Error::new(
        ErrorKind::Unsupported,
        "RENAME_NOREPLACE is unavailable on this platform",
    ))
}

#[cfg(all(unix, target_os = "linux"))]
fn read_dir_entry_names(dir_fd: &OwnedFd) -> Result<Vec<CString>, std::io::Error> {
    let dup_fd = unsafe { libc::fcntl(dir_fd.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 0) };
    if dup_fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let dir = unsafe { libc::fdopendir(dup_fd) };
    if dir.is_null() {
        let err = std::io::Error::last_os_error();
        let _ = unsafe { libc::close(dup_fd) };
        return Err(err);
    }

    let mut names = Vec::new();
    let mut read_err: Option<std::io::Error> = None;
    loop {
        unsafe {
            *libc::__errno_location() = 0;
        }
        let entry = unsafe { libc::readdir(dir) };
        if entry.is_null() {
            let errno = unsafe { *libc::__errno_location() };
            if errno != 0 {
                read_err = Some(std::io::Error::from_raw_os_error(errno));
            }
            break;
        }
        let raw = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) };
        let bytes = raw.to_bytes();
        if bytes == b"." || bytes == b".." {
            continue;
        }
        let name = CString::new(bytes).map_err(|_| {
            std::io::Error::new(
                ErrorKind::InvalidData,
                "Directory entry contains an unexpected NUL byte",
            )
        })?;
        names.push(name);
    }

    if unsafe { libc::closedir(dir) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    if let Some(err) = read_err {
        return Err(err);
    }
    Ok(names)
}

#[cfg(all(unix, target_os = "linux"))]
fn unlinkat_nofollow(
    parent_fd: libc::c_int,
    name: &CString,
    flags: libc::c_int,
) -> Result<(), std::io::Error> {
    let rc = unsafe { libc::unlinkat(parent_fd, name.as_ptr(), flags) };
    if rc == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(all(unix, target_os = "linux"))]
fn delete_dir_recursive_at(parent_fd: libc::c_int, name: &CString) -> Result<(), std::io::Error> {
    let child_raw = unsafe {
        libc::openat(
            parent_fd,
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if child_raw < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let child_fd = unsafe { OwnedFd::from_raw_fd(child_raw) };

    for child_name in read_dir_entry_names(&child_fd)? {
        let child_stat = match fstatat_nofollow(child_fd.as_raw_fd(), &child_name) {
            Ok(stat) => stat,
            Err(err) if err.kind() == ErrorKind::NotFound => continue,
            Err(err) => return Err(err),
        };
        let mode = child_stat.st_mode & libc::S_IFMT;
        if mode == libc::S_IFLNK {
            return Err(std::io::Error::other("Symlinks are not allowed"));
        }
        if mode == libc::S_IFDIR {
            delete_dir_recursive_at(child_fd.as_raw_fd(), &child_name)?;
        } else {
            match unlinkat_nofollow(child_fd.as_raw_fd(), &child_name, 0) {
                Ok(()) => {}
                Err(err) if err.kind() == ErrorKind::NotFound => {}
                Err(err) => return Err(err),
            }
        }
    }

    unlinkat_nofollow(parent_fd, name, libc::AT_REMOVEDIR)
}

#[cfg(all(unix, target_os = "linux"))]
fn delete_nofollow_io(path: &Path) -> Result<(), std::io::Error> {
    let (parent_fd, name) = parent_fd_and_name(path)?;
    let stat = fstatat_nofollow(parent_fd.as_raw_fd(), &name)?;
    let mode = stat.st_mode & libc::S_IFMT;
    if mode == libc::S_IFLNK {
        return Err(std::io::Error::other("Symlinks are not allowed"));
    }
    if mode == libc::S_IFDIR {
        delete_dir_recursive_at(parent_fd.as_raw_fd(), &name)
    } else {
        unlinkat_nofollow(parent_fd.as_raw_fd(), &name, 0)
    }
}

#[cfg(not(all(unix, target_os = "linux")))]
fn metadata_nofollow_path(path: &Path) -> Result<fs::Metadata, std::io::Error> {
    check_no_symlink_components(path).map_err(|e| std::io::Error::new(ErrorKind::Other, e))?;
    let meta = fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Symlinks are not allowed",
        ));
    }
    Ok(meta)
}

#[cfg(not(all(unix, target_os = "linux")))]
fn delete_dir_recursive_nofollow_path(path: &Path) -> Result<(), std::io::Error> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let child = entry.path();
        let child_meta = metadata_nofollow_path(&child)?;
        if child_meta.is_dir() {
            delete_dir_recursive_nofollow_path(&child)?;
        } else {
            fs::remove_file(&child)?;
        }
    }
    fs::remove_dir(path)
}

#[cfg(not(all(unix, target_os = "linux")))]
fn delete_nofollow_io(path: &Path) -> Result<(), std::io::Error> {
    let meta = metadata_nofollow_path(path)?;
    if meta.is_dir() {
        delete_dir_recursive_nofollow_path(path)
    } else {
        fs::remove_file(path)
    }
}

pub(crate) fn delete_entry_nofollow_io(path: &Path) -> Result<(), std::io::Error> {
    delete_nofollow_io(path)
}

#[cfg(all(unix, target_os = "linux"))]
pub(super) fn open_nofollow_path_fd(path: &Path) -> UndoResult<OwnedFd> {
    use std::io;

    if !path.is_absolute() {
        return Err(format!("Path must be absolute: {}", path.display()).into());
    }

    let root = CString::new("/").map_err(|_| "Failed to build root path".to_string())?;
    let root_fd = unsafe {
        libc::open(
            root.as_ptr(),
            libc::O_PATH | libc::O_DIRECTORY | libc::O_CLOEXEC,
        )
    };
    if root_fd < 0 {
        return Err(format!(
            "Failed to open root while resolving {}: {}",
            path.display(),
            io::Error::last_os_error()
        )
        .into());
    }

    let mut current = unsafe { OwnedFd::from_raw_fd(root_fd) };
    let mut components = path.components().peekable();
    while let Some(component) = components.next() {
        match component {
            Component::RootDir | Component::CurDir => continue,
            Component::ParentDir => {
                return Err(format!(
                    "Parent directory components are not allowed: {}",
                    path.display()
                )
                .into());
            }
            Component::Normal(seg) => {
                let seg_name = seg.to_string_lossy().into_owned();
                let c_seg = CString::new(seg.as_bytes()).map_err(|_| {
                    format!(
                        "Invalid path component (NUL byte) while resolving {}",
                        path.display()
                    )
                })?;
                let is_last = components.peek().is_none();
                let mut flags = libc::O_PATH | libc::O_NOFOLLOW | libc::O_CLOEXEC;
                if !is_last {
                    flags |= libc::O_DIRECTORY;
                }
                let fd = unsafe { libc::openat(current.as_raw_fd(), c_seg.as_ptr(), flags) };
                if fd < 0 {
                    return Err(format!(
                        "Failed to open path component '{}' for {}: {}",
                        seg_name,
                        path.display(),
                        io::Error::last_os_error()
                    )
                    .into());
                }
                current = unsafe { OwnedFd::from_raw_fd(fd) };
            }
            Component::Prefix(_) => {
                return Err(format!("Unsupported path prefix: {}", path.display()).into());
            }
        }
    }

    Ok(current)
}
