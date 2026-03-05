use super::{error::PermissionsResult, AccessBits, PermissionInfo};
use std::{fs, path::Path};

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, PermissionsExt};

fn is_executable(meta: &fs::Metadata) -> Option<bool> {
    #[cfg(unix)]
    {
        Some(meta.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        None
    }
}

#[cfg(unix)]
fn lookup_user_name(uid: u32) -> Option<String> {
    use std::ffi::CStr;
    use std::mem::MaybeUninit;
    use std::ptr;

    let mut buf_len = 1024usize;
    for _ in 0..4 {
        let mut pwd = MaybeUninit::<libc::passwd>::zeroed();
        let mut result: *mut libc::passwd = ptr::null_mut();
        let mut buf = vec![0u8; buf_len];
        let rc = unsafe {
            libc::getpwuid_r(
                uid,
                pwd.as_mut_ptr(),
                buf.as_mut_ptr() as *mut libc::c_char,
                buf.len(),
                &mut result,
            )
        };
        if rc == 0 {
            if result.is_null() {
                return None;
            }
            let pwd = unsafe { pwd.assume_init() };
            if pwd.pw_name.is_null() {
                return None;
            }
            let name = unsafe { CStr::from_ptr(pwd.pw_name) }
                .to_string_lossy()
                .into_owned();
            return if name.is_empty() { None } else { Some(name) };
        }
        if rc == libc::ERANGE {
            buf_len *= 2;
            continue;
        }
        return None;
    }
    None
}

#[cfg(unix)]
fn lookup_group_name(gid: u32) -> Option<String> {
    use std::ffi::CStr;
    use std::mem::MaybeUninit;
    use std::ptr;

    let mut buf_len = 1024usize;
    for _ in 0..4 {
        let mut grp = MaybeUninit::<libc::group>::zeroed();
        let mut result: *mut libc::group = ptr::null_mut();
        let mut buf = vec![0u8; buf_len];
        let rc = unsafe {
            libc::getgrgid_r(
                gid,
                grp.as_mut_ptr(),
                buf.as_mut_ptr() as *mut libc::c_char,
                buf.len(),
                &mut result,
            )
        };
        if rc == 0 {
            if result.is_null() {
                return None;
            }
            let grp = unsafe { grp.assume_init() };
            if grp.gr_name.is_null() {
                return None;
            }
            let name = unsafe { CStr::from_ptr(grp.gr_name) }
                .to_string_lossy()
                .into_owned();
            return if name.is_empty() { None } else { Some(name) };
        }
        if rc == libc::ERANGE {
            buf_len *= 2;
            continue;
        }
        return None;
    }
    None
}

#[cfg(unix)]
fn resolve_owner_group_names(meta: &fs::Metadata) -> (Option<String>, Option<String>) {
    let uid = meta.uid();
    let gid = meta.gid();
    let owner_name = lookup_user_name(uid).or_else(|| Some(uid.to_string()));
    let group_name = lookup_group_name(gid).or_else(|| Some(gid.to_string()));
    (owner_name, group_name)
}

pub(super) fn permission_info_from_metadata(
    target: &Path,
    meta: &fs::Metadata,
) -> PermissionsResult<PermissionInfo> {
    #[cfg(target_os = "windows")]
    {
        let bits = super::windows_acl::read_bits(target, meta.is_dir())?;
        return Ok(PermissionInfo {
            read_only: !bits.owner.write,
            executable: Some(bits.owner.exec),
            executable_supported: true,
            access_supported: true,
            ownership_supported: false,
            owner_name: None,
            group_name: None,
            owner: Some(bits.owner),
            group: bits.group,
            other: Some(bits.everyone),
        });
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = target;
        let read_only = meta.permissions().readonly();
        let executable = is_executable(meta);
        #[cfg(unix)]
        let (owner_name, group_name) = resolve_owner_group_names(meta);
        #[cfg(not(unix))]
        let (owner_name, group_name) = (None, None);
        #[cfg(unix)]
        let (access_supported, owner, group, other) = {
            let mode = meta.permissions().mode();
            let owner = AccessBits {
                read: mode & 0o400 != 0,
                write: mode & 0o200 != 0,
                exec: mode & 0o100 != 0,
            };
            let group = AccessBits {
                read: mode & 0o040 != 0,
                write: mode & 0o020 != 0,
                exec: mode & 0o010 != 0,
            };
            let other = AccessBits {
                read: mode & 0o004 != 0,
                write: mode & 0o002 != 0,
                exec: mode & 0o001 != 0,
            };
            (true, Some(owner), Some(group), Some(other))
        };
        #[cfg(not(unix))]
        let (access_supported, owner, group, other) = (false, None, None, None);

        Ok(PermissionInfo {
            read_only,
            executable,
            executable_supported: executable.is_some(),
            access_supported,
            ownership_supported: cfg!(unix),
            owner_name,
            group_name,
            owner,
            group,
            other,
        })
    }
}
