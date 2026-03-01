use std::fs;
use std::path::Path;

use crate::fs_utils::check_no_symlink_components;
use crate::undo::{UndoError, UndoResult};

use super::nofollow;
use super::{OwnershipSnapshot, PermissionsSnapshot};

#[cfg(all(unix, target_os = "linux"))]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(all(unix, not(target_os = "linux")))]
use std::{ffi::CString, os::unix::ffi::OsStrExt};

#[cfg(target_os = "windows")]
use std::{os::windows::ffi::OsStrExt, ptr};
#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::{LocalFree, ERROR_SUCCESS};
#[cfg(target_os = "windows")]
use windows_sys::Win32::Security::Authorization::{
    GetNamedSecurityInfoW, SetNamedSecurityInfoW, SE_FILE_OBJECT,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::Security::{
    GetSecurityDescriptorDacl, ACL, DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR,
};

pub(crate) fn apply_permissions(path: &Path, snap: &PermissionsSnapshot) -> UndoResult<()> {
    check_no_symlink_components(path)?;
    let meta_no_follow = fs::symlink_metadata(path).map_err(|e| {
        UndoError::from_io_error(format!("Failed to read metadata for {}", path.display()), e)
    })?;
    if meta_no_follow.file_type().is_symlink() {
        return Err(UndoError::symlink_unsupported(path));
    }
    let meta = fs::metadata(path).map_err(|e| {
        UndoError::from_io_error(format!("Failed to read metadata for {}", path.display()), e)
    })?;
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
            return Err(UndoError::win32_failure(
                format!("Failed to update permissions for {}", path.display()),
                status,
            ));
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(snap.mode);
    }
    fs::set_permissions(path, perms).map_err(|e| {
        UndoError::from_io_error(
            format!("Failed to update permissions for {}", path.display()),
            e,
        )
    })
}

#[allow(dead_code)]
pub fn permissions_snapshot(path: &Path) -> UndoResult<PermissionsSnapshot> {
    let meta = fs::metadata(path).map_err(|e| {
        UndoError::from_io_error(format!("Failed to read metadata for {}", path.display()), e)
    })?;
    let readonly = meta.permissions().readonly();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = meta.permissions().mode();
        Ok(PermissionsSnapshot { readonly, mode })
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
pub fn ownership_snapshot(path: &Path) -> UndoResult<OwnershipSnapshot> {
    #[cfg(unix)]
    {
        check_no_symlink_components(path)?;
        let meta = fs::symlink_metadata(path).map_err(|e| {
            UndoError::from_io_error(format!("Failed to read metadata for {}", path.display()), e)
        })?;
        if meta.file_type().is_symlink() {
            return Err(UndoError::symlink_unsupported(path));
        }
        Ok(OwnershipSnapshot {
            uid: meta.uid(),
            gid: meta.gid(),
        })
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err(UndoError::unsupported_operation(
            "Ownership changes are not supported on this platform",
        ))
    }
}

pub(crate) fn set_ownership_nofollow(
    path: &Path,
    uid: Option<u32>,
    gid: Option<u32>,
) -> UndoResult<()> {
    #[cfg(all(unix, target_os = "linux"))]
    {
        use std::io;

        if uid.is_none() && gid.is_none() {
            return Ok(());
        }
        let fd = nofollow::open_nofollow_path_fd(path)?;
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
        Err(UndoError::from_io_error(
            format!(
                "Failed to change owner/group for {}{}",
                path.display(),
                suffix
            ),
            err,
        ))
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
            .map_err(|_| UndoError::invalid_path(path, "Path contains NUL byte"))?;
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
        return Err(UndoError::from_io_error(
            format!(
                "Failed to change owner/group for {}{}",
                path.display(),
                suffix
            ),
            err,
        ));
    }
    #[cfg(not(unix))]
    {
        let _ = (path, uid, gid);
        Err(UndoError::unsupported_operation(
            "Ownership changes are not supported on this platform",
        ))
    }
}

#[cfg(all(unix, target_os = "linux"))]
pub(crate) fn set_unix_mode_nofollow(path: &Path, mode: u32) -> UndoResult<()> {
    use std::io;

    let fd = nofollow::open_nofollow_path_fd(path)?;
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
    Err(UndoError::from_io_error(
        format!("Failed to update permissions for {}", path.display()),
        err,
    ))
}

pub(crate) fn apply_ownership(path: &Path, snap: &OwnershipSnapshot) -> UndoResult<()> {
    #[cfg(unix)]
    {
        check_no_symlink_components(path)?;
        let meta = fs::symlink_metadata(path).map_err(|e| {
            UndoError::from_io_error(format!("Failed to read metadata for {}", path.display()), e)
        })?;
        if meta.file_type().is_symlink() {
            return Err(UndoError::symlink_unsupported(path));
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
        set_ownership_nofollow(path, uid, gid)
    }
    #[cfg(not(unix))]
    {
        let _ = (path, snap);
        Err(UndoError::unsupported_operation(
            "Ownership changes are not supported on this platform",
        ))
    }
}

#[cfg(target_os = "windows")]
fn snapshot_dacl(path: &Path) -> UndoResult<Option<Vec<u8>>> {
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
        return Err(UndoError::win32_failure(
            format!("GetNamedSecurityInfoW failed for {}", path.display()),
            status,
        ));
    }
    let result = unsafe {
        let mut present = 0i32;
        let mut defaulted = 0i32;
        let mut acl_ptr = dacl;
        let ok = GetSecurityDescriptorDacl(sd, &mut present, &mut acl_ptr, &mut defaulted);
        if ok == 0 {
            Err(UndoError::new(
                super::error::UndoErrorCode::IoError,
                "GetSecurityDescriptorDacl failed",
            ))
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
