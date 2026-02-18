#[cfg(unix)]
use std::ffi::{CStr, CString};
use std::fs;
use std::io::{self, ErrorKind};
#[cfg(all(unix, target_os = "linux"))]
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};
#[cfg(target_os = "windows")]
use std::{os::windows::ffi::OsStrExt, ptr};

use crate::fs_utils::check_no_symlink_components;

#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::{
    LocalFree, ERROR_ALREADY_EXISTS, ERROR_FILE_EXISTS, ERROR_SUCCESS,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::Security::Authorization::{
    GetNamedSecurityInfoW, SetNamedSecurityInfoW, SE_FILE_OBJECT,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::Security::{
    GetSecurityDescriptorDacl, ACL, DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::Storage::FileSystem::{
    GetFileAttributesW, MoveFileExW, SetFileAttributesW, FILE_ATTRIBUTE_HIDDEN,
    MOVEFILE_WRITE_THROUGH,
};

mod backup;
mod types;

pub use backup::{cleanup_stale_backups, temp_backup_path};
pub(crate) use types::PathSnapshot;
pub use types::{
    Action, Direction, OwnershipSnapshot, PermissionsSnapshot, UndoManager, UndoState,
};

#[cfg(test)]
mod tests;

#[tauri::command]
pub fn undo_action(state: tauri::State<'_, UndoState>) -> Result<(), String> {
    state.undo()
}

#[tauri::command]
pub fn redo_action(state: tauri::State<'_, UndoState>) -> Result<(), String> {
    state.redo()
}

pub(crate) fn run_actions(actions: &mut [Action], direction: Direction) -> Result<(), String> {
    execute_batch(actions, direction)
}

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
    let parent_fd = open_nofollow_path_fd(parent)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
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
fn rename_nofollow_io(src: &Path, dst: &Path) -> Result<(), std::io::Error> {
    if absolute_path(src)? == absolute_path(dst)? {
        return Ok(());
    }

    let (src_parent_fd, src_name) = parent_fd_and_name(src)?;
    let (dst_parent_fd, dst_name) = parent_fd_and_name(dst)?;

    let src_stat = fstatat_nofollow(src_parent_fd.as_raw_fd(), &src_name)?;
    if (src_stat.st_mode & libc::S_IFMT) == libc::S_IFLNK {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Symlinks are not allowed",
        ));
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
fn rename_nofollow_io(src: &Path, dst: &Path) -> Result<(), std::io::Error> {
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
fn rename_nofollow_io(src: &Path, dst: &Path) -> Result<(), std::io::Error> {
    let _ = (src, dst);
    Err(std::io::Error::new(
        ErrorKind::Unsupported,
        "RENAME_NOREPLACE is unavailable on this platform",
    ))
}

#[cfg(not(any(unix, target_os = "windows")))]
fn rename_nofollow_io(src: &Path, dst: &Path) -> Result<(), std::io::Error> {
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
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Symlinks are not allowed",
            ));
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
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Symlinks are not allowed",
        ));
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

pub(crate) fn snapshot_existing_path(path: &Path) -> Result<PathSnapshot, String> {
    let meta = ensure_existing_path_nonsymlink(path)?;
    Ok(types::path_snapshot_from_meta(&meta))
}

pub(crate) fn assert_path_snapshot(path: &Path, expected: &PathSnapshot) -> Result<(), String> {
    let meta = ensure_existing_path_nonsymlink(path)?;
    let current = types::path_snapshot_from_meta(&meta);
    if types::snapshots_match(expected, &current) {
        Ok(())
    } else {
        Err(format!("Path changed during operation: {}", path.display()))
    }
}

fn execute_action(action: &mut Action, direction: Direction) -> Result<(), String> {
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
                    .ok_or_else(|| "Invalid backup path".to_string())?;
                fs::create_dir_all(parent).map_err(|e| {
                    format!("Failed to create backup dir {}: {e}", parent.display())
                })?;
                move_with_fallback(path, backup)
            }
        },
        Action::Delete { path, backup } => match direction {
            Direction::Forward => {
                let parent = backup
                    .parent()
                    .ok_or_else(|| "Invalid backup path".to_string())?;
                fs::create_dir_all(parent).map_err(|e| {
                    format!("Failed to create backup dir {}: {e}", parent.display())
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
            Direction::Forward => fs::create_dir(&*path)
                .map_err(|e| format!("Failed to create directory {}: {e}", path.display())),
            Direction::Backward => match delete_entry_nofollow_io(path) {
                Ok(()) => Ok(()),
                Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
                Err(err) => Err(format!(
                    "Failed to remove directory {}: {err}",
                    path.display()
                )),
            },
        },
    }
}

#[cfg(target_os = "windows")]
fn set_windows_hidden_attr(path: &Path, hidden: bool) -> Result<(), String> {
    check_no_symlink_components(path)?;
    let no_follow = fs::symlink_metadata(path)
        .map_err(|e| format!("Failed to read metadata for {}: {e}", path.display()))?;
    if no_follow.file_type().is_symlink() {
        return Err(format!("Symlinks are not allowed: {}", path.display()));
    }

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let attrs = unsafe { GetFileAttributesW(wide.as_ptr()) };
    if attrs == u32::MAX {
        return Err(format!("GetFileAttributes failed for {}", path.display()));
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
        return Err(format!("SetFileAttributes failed for {}", path.display()));
    }
    Ok(())
}

fn execute_batch(actions: &mut [Action], direction: Direction) -> Result<(), String> {
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
                return Err(format!("Batch action {} failed: {}", idx + 1, err));
            } else {
                return Err(format!(
                    "Batch action {} failed: {}; additional rollback issues: {}",
                    idx + 1,
                    err,
                    rollback_errors.join("; ")
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

pub(crate) fn apply_permissions(path: &Path, snap: &PermissionsSnapshot) -> Result<(), String> {
    check_no_symlink_components(path)?;
    let meta_no_follow = fs::symlink_metadata(path)
        .map_err(|e| format!("Failed to read metadata for {}: {e}", path.display()))?;
    if meta_no_follow.file_type().is_symlink() {
        return Err(format!("Symlinks are not allowed: {}", path.display()));
    }
    let meta = fs::metadata(path)
        .map_err(|e| format!("Failed to read metadata for {}: {e}", path.display()))?;
    let mut perms = meta.permissions();
    perms.set_readonly(snap.readonly);
    #[cfg(target_os = "windows")]
    {
        let mut wide: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let dacl_ptr = snap
            .dacl
            .as_ref()
            .map(|v| v.as_ptr() as *mut ACL)
            .unwrap_or(ptr::null_mut());
        let status = unsafe {
            SetNamedSecurityInfoW(
                wide.as_mut_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                ptr::null_mut(),
                ptr::null_mut(),
                dacl_ptr,
                ptr::null_mut(),
            )
        };
        if status != ERROR_SUCCESS {
            return Err(format!(
                "Failed to update permissions for {}: Win32 error {}",
                path.display(),
                status
            ));
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(snap.mode);
    }
    fs::set_permissions(path, perms)
        .map_err(|e| format!("Failed to update permissions for {}: {e}", path.display()))
}

#[allow(dead_code)]
pub fn permissions_snapshot(path: &Path) -> Result<PermissionsSnapshot, String> {
    let meta = fs::metadata(path)
        .map_err(|e| format!("Failed to read metadata for {}: {e}", path.display()))?;
    let readonly = meta.permissions().readonly();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = meta.permissions().mode();
        return Ok(PermissionsSnapshot { readonly, mode });
    }
    #[cfg(target_os = "windows")]
    {
        let dacl = snapshot_dacl(path)?;
        return Ok(PermissionsSnapshot { readonly, dacl });
    }
    #[cfg(not(any(unix, target_os = "windows")))]
    Ok(PermissionsSnapshot { readonly })
}

#[allow(dead_code)]
pub fn ownership_snapshot(path: &Path) -> Result<OwnershipSnapshot, String> {
    #[cfg(unix)]
    {
        check_no_symlink_components(path)?;
        let meta = fs::symlink_metadata(path)
            .map_err(|e| format!("Failed to read metadata for {}: {e}", path.display()))?;
        if meta.file_type().is_symlink() {
            return Err(format!("Symlinks are not allowed: {}", path.display()));
        }
        return Ok(OwnershipSnapshot {
            uid: meta.uid(),
            gid: meta.gid(),
        });
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err("Ownership changes are not supported on this platform".into())
    }
}

#[cfg(all(unix, target_os = "linux"))]
fn open_nofollow_path_fd(path: &Path) -> Result<OwnedFd, String> {
    use std::io;

    if !path.is_absolute() {
        return Err(format!("Path must be absolute: {}", path.display()));
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
        ));
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
                ));
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
                    ));
                }
                current = unsafe { OwnedFd::from_raw_fd(fd) };
            }
            Component::Prefix(_) => {
                return Err(format!("Unsupported path prefix: {}", path.display()));
            }
        }
    }

    Ok(current)
}

pub(crate) fn set_ownership_nofollow(
    path: &Path,
    uid: Option<u32>,
    gid: Option<u32>,
) -> Result<(), String> {
    #[cfg(all(unix, target_os = "linux"))]
    {
        use std::io;

        if uid.is_none() && gid.is_none() {
            return Ok(());
        }
        let fd = open_nofollow_path_fd(path)?;
        let uid_arg = uid.map(|v| v as libc::uid_t).unwrap_or(!0 as libc::uid_t);
        let gid_arg = gid.map(|v| v as libc::gid_t).unwrap_or(!0 as libc::gid_t);
        let empty: [libc::c_char; 1] = [0];
        let rc = unsafe {
            libc::fchownat(
                fd.as_raw_fd(),
                empty.as_ptr(),
                uid_arg,
                gid_arg,
                libc::AT_EMPTY_PATH,
            )
        };
        if rc == 0 {
            return Ok(());
        }
        let err = io::Error::last_os_error();
        let suffix = match err.raw_os_error() {
            Some(code) if code == libc::EPERM || code == libc::EACCES => {
                " (requires elevated privileges: root or CAP_CHOWN)"
            }
            _ => "",
        };
        return Err(format!(
            "Failed to change owner/group for {}: {}{}",
            path.display(),
            err,
            suffix
        ));
    }
    #[cfg(all(unix, not(target_os = "linux")))]
    {
        use std::io;

        if uid.is_none() && gid.is_none() {
            return Ok(());
        }
        check_no_symlink_components(path)?;
        let bytes = path.as_os_str().as_bytes();
        let c_path = CString::new(bytes)
            .map_err(|_| format!("Path contains NUL byte: {}", path.display()))?;
        let uid_arg = uid.map(|v| v as libc::uid_t).unwrap_or(!0 as libc::uid_t);
        let gid_arg = gid.map(|v| v as libc::gid_t).unwrap_or(!0 as libc::gid_t);
        let rc = unsafe {
            libc::fchownat(
                libc::AT_FDCWD,
                c_path.as_ptr(),
                uid_arg,
                gid_arg,
                libc::AT_SYMLINK_NOFOLLOW,
            )
        };
        if rc == 0 {
            return Ok(());
        }
        let err = io::Error::last_os_error();
        let suffix = match err.raw_os_error() {
            Some(code) if code == libc::EPERM || code == libc::EACCES => {
                " (requires elevated privileges: root or CAP_CHOWN)"
            }
            _ => "",
        };
        return Err(format!(
            "Failed to change owner/group for {}: {}{}",
            path.display(),
            err,
            suffix
        ));
    }
    #[cfg(not(unix))]
    {
        let _ = (path, uid, gid);
        Err("Ownership changes are not supported on this platform".into())
    }
}

#[cfg(all(unix, target_os = "linux"))]
pub(crate) fn set_unix_mode_nofollow(path: &Path, mode: u32) -> Result<(), String> {
    use std::io;

    let fd = open_nofollow_path_fd(path)?;
    let empty: [libc::c_char; 1] = [0];
    let rc = unsafe {
        libc::fchmodat(
            fd.as_raw_fd(),
            empty.as_ptr(),
            mode as libc::mode_t,
            libc::AT_EMPTY_PATH,
        )
    };
    if rc == 0 {
        return Ok(());
    }
    let err = io::Error::last_os_error();
    Err(format!(
        "Failed to update permissions for {}: {}",
        path.display(),
        err
    ))
}

pub(crate) fn apply_ownership(path: &Path, snap: &OwnershipSnapshot) -> Result<(), String> {
    #[cfg(unix)]
    {
        check_no_symlink_components(path)?;
        let meta = fs::symlink_metadata(path)
            .map_err(|e| format!("Failed to read metadata for {}: {e}", path.display()))?;
        if meta.file_type().is_symlink() {
            return Err(format!("Symlinks are not allowed: {}", path.display()));
        }
        let current_uid = meta.uid();
        let current_gid = meta.gid();
        let uid = if current_uid != snap.uid {
            Some(snap.uid)
        } else {
            None
        };
        let gid = if current_gid != snap.gid {
            Some(snap.gid)
        } else {
            None
        };
        return set_ownership_nofollow(path, uid, gid);
    }
    #[cfg(not(unix))]
    {
        let _ = (path, snap);
        Err("Ownership changes are not supported on this platform".into())
    }
}

#[cfg(target_os = "windows")]
fn snapshot_dacl(path: &Path) -> Result<Option<Vec<u8>>, String> {
    let mut sd: PSECURITY_DESCRIPTOR = ptr::null_mut();
    let mut dacl: *mut ACL = ptr::null_mut();
    let mut wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let status = unsafe {
        GetNamedSecurityInfoW(
            wide.as_mut_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            &mut dacl,
            ptr::null_mut(),
            &mut sd,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(format!(
            "GetNamedSecurityInfoW failed for {}: Win32 error {}",
            path.display(),
            status
        ));
    }
    let result = unsafe {
        let mut present = 0i32;
        let mut defaulted = 0i32;
        let mut acl_ptr = dacl;
        let ok = GetSecurityDescriptorDacl(sd, &mut present, &mut acl_ptr, &mut defaulted);
        if ok == 0 {
            Err("GetSecurityDescriptorDacl failed".into())
        } else if present == 0 || acl_ptr.is_null() {
            Ok(None)
        } else {
            let size = (*acl_ptr).AclSize as usize;
            let bytes = std::slice::from_raw_parts(acl_ptr as *const u8, size).to_vec();
            Ok(Some(bytes))
        }
    };
    unsafe {
        LocalFree(sd);
    }
    result
}

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

fn move_by_copy_delete_noreplace(
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

fn ensure_existing_path_nonsymlink(path: &Path) -> Result<fs::Metadata, String> {
    check_no_symlink_components(path)?;
    let meta = fs::symlink_metadata(path)
        .map_err(|e| format!("Failed to read metadata for {}: {e}", path.display()))?;
    if meta.file_type().is_symlink() {
        return Err(format!(
            "Refusing path with symlink target: {}",
            path.display()
        ));
    }
    Ok(meta)
}

fn ensure_existing_dir_nonsymlink(path: &Path) -> Result<(), String> {
    let meta = ensure_existing_path_nonsymlink(path)?;
    if !meta.is_dir() {
        return Err(format!("Expected directory path: {}", path.display()));
    }
    Ok(())
}
