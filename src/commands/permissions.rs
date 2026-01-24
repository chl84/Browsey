#![allow(dead_code, unused_variables)]
use std::fs::{self, Permissions};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use tracing::{debug, warn};

use crate::{
    fs_utils::{check_no_symlink_components, sanitize_path_follow, sanitize_path_nofollow},
    undo::{permissions_snapshot, run_actions, Action, Direction, UndoState},
};

#[derive(serde::Serialize, Clone, PartialEq, Eq)]
pub struct AccessBits {
    pub read: bool,
    pub write: bool,
    pub exec: bool,
}

#[derive(serde::Deserialize, Clone)]
pub struct AccessUpdate {
    pub read: Option<bool>,
    pub write: Option<bool>,
    pub exec: Option<bool>,
}

#[derive(serde::Serialize)]
pub struct PermissionInfo {
    pub read_only: bool,
    pub executable: Option<bool>,
    pub executable_supported: bool,
    pub access_supported: bool,
    pub owner: Option<AccessBits>,
    pub group: Option<AccessBits>,
    pub other: Option<AccessBits>,
}

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

#[tauri::command]
pub fn get_permissions(path: String) -> Result<PermissionInfo, String> {
    debug!(path = %path, "get_permissions start");
    let nofollow = sanitize_path_nofollow(&path, true)?;
    let meta =
        fs::symlink_metadata(&nofollow).map_err(|e| format!("Failed to read metadata: {e}"))?;
    if meta.file_type().is_symlink() {
        return Err("Permissions are not supported on symlinks".into());
    }
    let target = sanitize_path_follow(&path, true)?;
    check_no_symlink_components(&target)?;
    debug!(path = %target.display(), "get_permissions resolved target");

    #[cfg(target_os = "windows")]
    {
        let bits = windows_acl::read_bits(&target, meta.is_dir())?;
        return Ok(PermissionInfo {
            read_only: !bits.owner.write,
            executable: Some(bits.owner.exec),
            executable_supported: true,
            access_supported: true,
            owner: Some(bits.owner),
            group: bits.group,
            other: Some(bits.everyone),
        });
    }

    #[cfg(not(target_os = "windows"))]
    {
        let read_only = meta.permissions().readonly();
        let executable = is_executable(&meta);
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

        return Ok(PermissionInfo {
            read_only,
            executable,
            executable_supported: executable.is_some(),
            access_supported,
            owner,
            group,
            other,
        });
    }
}

#[cfg(unix)]
fn set_permissions_batch(
    paths: Vec<String>,
    read_only: Option<bool>,
    executable: Option<bool>,
    owner: Option<AccessUpdate>,
    group: Option<AccessUpdate>,
    other: Option<AccessUpdate>,
    undo: Option<&UndoState>,
) -> Result<PermissionInfo, String> {
    let has_access_updates = owner.is_some() || group.is_some() || other.is_some();
    if read_only.is_none() && executable.is_none() && !has_access_updates {
        return Err("No permission changes were provided".into());
    }

    let mut actions: Vec<Action> = Vec::with_capacity(paths.len());
    let mut first_info: Option<PermissionInfo> = None;

    for path in paths {
        let owner_update = owner.clone();
        let group_update = group.clone();
        let other_update = other.clone();
        debug!(
            path = %path,
            read_only = ?read_only,
            executable = ?executable,
            "set_permissions start"
        );
        let nofollow = sanitize_path_nofollow(&path, true)?;
        let meta =
            fs::symlink_metadata(&nofollow).map_err(|e| format!("Failed to read metadata: {e}"))?;
        if meta.file_type().is_symlink() {
            return Err("Permissions are not supported on symlinks".into());
        }
        let target = sanitize_path_follow(&path, true)?;
        check_no_symlink_components(&target)?;
        debug!(path = %target.display(), "set_permissions resolved target");

        let before = permissions_snapshot(&target)?;

        let mut perms: Permissions = meta.permissions();
        let mut changed = false;
        let mut mode = perms.mode();
        let original_mode = mode;
        if let Some(ro) = read_only {
            if ro {
                // Clear only owner write; leave group/other untouched.
                mode &= !0o200;
            } else {
                mode |= 0o200;
            }
        }
        if let Some(exec) = executable {
            if exec {
                // Set the owner execute bit; preserve any existing group/other bits.
                mode |= 0o100;
            } else {
                // Only clear owner execute; keep group/other as-is to avoid breaking collaborators.
                mode &= !0o100;
            }
        }
        if let Some(update) = owner_update {
            if let Some(r) = update.read {
                if r {
                    mode |= 0o400;
                } else {
                    mode &= !0o400;
                }
            }
            if let Some(w) = update.write {
                if w {
                    mode |= 0o200;
                } else {
                    mode &= !0o200;
                }
            }
            if let Some(x) = update.exec {
                if x {
                    mode |= 0o100;
                } else {
                    mode &= !0o100;
                }
            }
        }
        if let Some(update) = group_update {
            if let Some(r) = update.read {
                if r {
                    mode |= 0o040;
                } else {
                    mode &= !0o040;
                }
            }
            if let Some(w) = update.write {
                if w {
                    mode |= 0o020;
                } else {
                    mode &= !0o020;
                }
            }
            if let Some(x) = update.exec {
                if x {
                    mode |= 0o010;
                } else {
                    mode &= !0o010;
                }
            }
        }
        if let Some(update) = other_update {
            if let Some(r) = update.read {
                if r {
                    mode |= 0o004;
                } else {
                    mode &= !0o004;
                }
            }
            if let Some(w) = update.write {
                if w {
                    mode |= 0o002;
                } else {
                    mode &= !0o002;
                }
            }
            if let Some(x) = update.exec {
                if x {
                    mode |= 0o001;
                } else {
                    mode &= !0o001;
                }
            }
        }
        if mode != original_mode {
            changed = true;
            perms.set_mode(mode);
        }

        if changed {
            if let Err(e) = fs::set_permissions(&target, perms) {
                warn!(path = %target.display(), error = %e, "set_permissions failed");
                // rollback earlier ones
                if !actions.is_empty() {
                    let mut rev = actions.clone();
                    let _ = run_actions(&mut rev, Direction::Backward);
                }
                return Err(format!("Failed to update permissions: {e}"));
            }
            let after = permissions_snapshot(&target)?;
            actions.push(Action::SetPermissions {
                path: target.clone(),
                before,
                after,
            });
            if first_info.is_none() {
                // refresh info for first target
                let info = get_permissions(path.clone())?;
                first_info = Some(info);
            }
        }
    }

    if let Some(undo) = undo {
        if !actions.is_empty() {
            if actions.len() == 1 {
                let _ = undo.record_applied(actions.pop().unwrap());
            } else {
                let _ = undo.record_applied(Action::Batch(actions));
            }
        }
    }

    Ok(first_info.unwrap_or_else(|| PermissionInfo {
        read_only: false,
        executable: None,
        executable_supported: cfg!(unix),
        access_supported: cfg!(unix),
        owner: None,
        group: None,
        other: None,
    }))
}

#[cfg(target_os = "windows")]
fn set_permissions_batch(
    paths: Vec<String>,
    read_only: Option<bool>,
    executable: Option<bool>,
    owner: Option<AccessUpdate>,
    group: Option<AccessUpdate>,
    other: Option<AccessUpdate>,
    undo: Option<&UndoState>,
) -> Result<PermissionInfo, String> {
    let has_access_updates = owner.is_some() || group.is_some() || other.is_some();
    if read_only.is_none() && executable.is_none() && !has_access_updates {
        return Err("No permission changes were provided".into());
    }

    let mut actions: Vec<Action> = Vec::with_capacity(paths.len());
    let mut first_info: Option<PermissionInfo> = None;

    for path in paths {
        let owner_update = owner.clone();
        let group_update = group.clone();
        let other_update = other.clone();
        debug!(
            path = %path,
            read_only = ?read_only,
            executable = ?executable,
            "set_permissions start"
        );
        let nofollow = sanitize_path_nofollow(&path, true)?;
        let meta =
            fs::symlink_metadata(&nofollow).map_err(|e| format!("Failed to read metadata: {e}"))?;
        if meta.file_type().is_symlink() {
            return Err("Permissions are not supported on symlinks".into());
        }
        let target = sanitize_path_follow(&path, true)?;
        check_no_symlink_components(&target)?;
        debug!(path = %target.display(), "set_permissions resolved target");

        let before = permissions_snapshot(&target)?;
        let current = windows_acl::read_bits(&target, meta.is_dir())?;
        let mut desired = current.clone();

        if let Some(ro) = read_only {
            desired.owner.write = !ro;
        }
        if let Some(exec) = executable {
            desired.owner.exec = exec;
        }
        let apply_update = |bits: &mut AccessBits, upd: AccessUpdate| {
            if let Some(r) = upd.read {
                bits.read = r;
            }
            if let Some(w) = upd.write {
                bits.write = w;
            }
            if let Some(x) = upd.exec {
                bits.exec = x;
            }
        };
        if let Some(update) = owner_update {
            apply_update(&mut desired.owner, update);
        }
        if let Some(update) = group_update {
            if let Some(ref mut bits) = desired.group {
                apply_update(bits, update);
            } else {
                return Err("Group information is unavailable for this entry".into());
            }
        }
        if let Some(update) = other_update {
            apply_update(&mut desired.everyone, update);
        }

        let mut changed = false;
        let mut perms: Permissions = meta.permissions();
        let orig_ro = perms.readonly();
        if let Some(ro) = read_only {
            if ro != orig_ro {
                perms.set_readonly(ro);
                if let Err(e) = fs::set_permissions(&target, perms.clone()) {
                    warn!(path = %target.display(), error = %e, "set_permissions attr failed");
                    if !actions.is_empty() {
                        let mut rev = actions.clone();
                        let _ = run_actions(&mut rev, Direction::Backward);
                    }
                    return Err(format!("Failed to update permissions: {e}"));
                }
                changed = true;
            }
        }

        let dacl_changed = desired.owner != current.owner
            || desired.group != current.group
            || desired.everyone != current.everyone;
        if dacl_changed {
            if let Err(e) = windows_acl::apply_bits(&target, meta.is_dir(), &desired) {
                if let Some(ro) = read_only {
                    if ro != orig_ro {
                        let mut revert = perms.clone();
                        revert.set_readonly(orig_ro);
                        let _ = fs::set_permissions(&target, revert);
                    }
                }
                if !actions.is_empty() {
                    let mut rev = actions.clone();
                    let _ = run_actions(&mut rev, Direction::Backward);
                }
                return Err(format!("Failed to update permissions: {e}"));
            }
            changed = true;
        }

        if changed {
            let after = permissions_snapshot(&target)?;
            actions.push(Action::SetPermissions {
                path: target.clone(),
                before,
                after,
            });
            if first_info.is_none() {
                let info = get_permissions(path.clone())?;
                first_info = Some(info);
            }
        }
    }

    if let Some(undo) = undo {
        if !actions.is_empty() {
            if actions.len() == 1 {
                let _ = undo.record_applied(actions.pop().unwrap());
            } else {
                let _ = undo.record_applied(Action::Batch(actions));
            }
        }
    }

    Ok(first_info.unwrap_or_else(|| PermissionInfo {
        read_only: false,
        executable: None,
        executable_supported: true,
        access_supported: true,
        owner: None,
        group: None,
        other: None,
    }))
}

#[cfg(not(any(unix, target_os = "windows")))]
fn set_permissions_batch(
    paths: Vec<String>,
    read_only: Option<bool>,
    executable: Option<bool>,
    owner: Option<AccessUpdate>,
    group: Option<AccessUpdate>,
    other: Option<AccessUpdate>,
    undo: Option<&UndoState>,
) -> Result<PermissionInfo, String> {
    let _ = (paths, executable, owner, group, other, undo);
    if let Some(ro) = read_only {
        return Err(format!(
            "Permission changes are not supported on this platform (requested readOnly={ro})"
        ));
    }
    Err("Permission changes are not supported on this platform".into())
}

#[tauri::command]
pub fn set_permissions(
    path: Option<String>,
    paths: Option<Vec<String>>,
    #[allow(non_snake_case)] readOnly: Option<bool>,
    read_only: Option<bool>,
    executable: Option<bool>,
    owner: Option<AccessUpdate>,
    group: Option<AccessUpdate>,
    other: Option<AccessUpdate>,
    undo: tauri::State<UndoState>,
) -> Result<PermissionInfo, String> {
    let targets: Vec<String> = match (paths, path) {
        (Some(list), _) if !list.is_empty() => list,
        (_, Some(single)) => vec![single],
        _ => return Err("No paths provided".into()),
    };
    set_permissions_batch(
        targets,
        readOnly.or(read_only),
        executable,
        owner,
        group,
        other,
        Some(undo.inner()),
    )
}

#[cfg(target_os = "windows")]
mod windows_acl {
    use super::AccessBits;
    use std::{os::windows::ffi::OsStrExt, path::Path, ptr};
    use windows_sys::Win32::Foundation::{LocalFree, ERROR_SUCCESS};
    use windows_sys::Win32::Security::Authorization::{
        GetNamedSecurityInfoW, SetEntriesInAclW, SetNamedSecurityInfoW, EXPLICIT_ACCESS_W,
        NO_MULTIPLE_TRUSTEE, REVOKE_ACCESS, SET_ACCESS, SE_FILE_OBJECT, TRUSTEE_IS_GROUP,
        TRUSTEE_IS_SID, TRUSTEE_IS_USER, TRUSTEE_IS_WELL_KNOWN_GROUP, TRUSTEE_TYPE, TRUSTEE_W,
    };
    use windows_sys::Win32::Security::{
        CreateWellKnownSid, EqualSid, GetAce, GetSecurityDescriptorDacl, MapGenericMask,
        WinWorldSid, ACCESS_ALLOWED_ACE, ACCESS_DENIED_ACE, ACE_HEADER, ACL,
        DACL_SECURITY_INFORMATION, GENERIC_MAPPING, GROUP_SECURITY_INFORMATION, NO_INHERITANCE,
        OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, SECURITY_MAX_SID_SIZE,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ALL_ACCESS, FILE_APPEND_DATA, FILE_GENERIC_EXECUTE, FILE_GENERIC_READ,
        FILE_GENERIC_WRITE, FILE_LIST_DIRECTORY, FILE_READ_DATA, FILE_TRAVERSE, FILE_WRITE_DATA,
    };

    const ACE_TYPE_ALLOWED: u8 = 0;
    const ACE_TYPE_DENIED: u8 = 1;

    #[derive(Clone)]
    pub struct PrincipalBits {
        pub owner: AccessBits,
        pub group: Option<AccessBits>,
        pub everyone: AccessBits,
    }

    struct SecurityDescriptor {
        raw: PSECURITY_DESCRIPTOR,
    }

    impl Drop for SecurityDescriptor {
        fn drop(&mut self) {
            if !self.raw.is_null() {
                unsafe {
                    LocalFree(self.raw);
                }
            }
        }
    }

    struct LocalAcl {
        raw: *mut ACL,
    }

    impl Drop for LocalAcl {
        fn drop(&mut self) {
            if !self.raw.is_null() {
                unsafe {
                    LocalFree(self.raw as *mut _);
                }
            }
        }
    }

    fn to_wide(path: &Path) -> Vec<u16> {
        path.as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    fn fetch_security(
        path: &Path,
    ) -> Result<
        (
            SecurityDescriptor,
            *mut ACL,
            *mut core::ffi::c_void,
            *mut core::ffi::c_void,
        ),
        String,
    > {
        let mut sd: PSECURITY_DESCRIPTOR = ptr::null_mut();
        let mut dacl: *mut ACL = ptr::null_mut();
        let mut owner: *mut core::ffi::c_void = ptr::null_mut();
        let mut group: *mut core::ffi::c_void = ptr::null_mut();

        let mut wide = to_wide(path);
        let status = unsafe {
            GetNamedSecurityInfoW(
                wide.as_mut_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | OWNER_SECURITY_INFORMATION | GROUP_SECURITY_INFORMATION,
                &mut owner,
                &mut group,
                &mut dacl,
                ptr::null_mut(),
                &mut sd,
            )
        };
        if status != ERROR_SUCCESS {
            return Err(format!(
                "GetNamedSecurityInfoW failed: Win32 error {status}"
            ));
        }

        Ok((SecurityDescriptor { raw: sd }, dacl, owner, group))
    }

    fn everyone_sid() -> Result<Vec<u8>, String> {
        let mut sid = vec![0u8; SECURITY_MAX_SID_SIZE as usize];
        let mut size = sid.len() as u32;
        let ok = unsafe {
            CreateWellKnownSid(
                WinWorldSid,
                ptr::null_mut(),
                sid.as_mut_ptr() as *mut _,
                &mut size,
            )
        };
        if ok == 0 {
            return Err("CreateWellKnownSid failed".into());
        }
        sid.truncate(size as usize);
        Ok(sid)
    }

    fn mask_from_bits(bits: &AccessBits) -> u32 {
        let mut mask = 0u32;
        if bits.read {
            mask |= FILE_GENERIC_READ;
        }
        if bits.write {
            mask |= FILE_GENERIC_WRITE;
        }
        if bits.exec {
            mask |= FILE_GENERIC_EXECUTE;
        }
        mask
    }

    fn bits_from_mask(mask: u32, is_dir: bool) -> AccessBits {
        let read_mask = FILE_GENERIC_READ | FILE_READ_DATA | FILE_LIST_DIRECTORY;
        let write_mask = FILE_GENERIC_WRITE | FILE_WRITE_DATA | FILE_APPEND_DATA;
        let exec_mask = FILE_GENERIC_EXECUTE | FILE_TRAVERSE;
        AccessBits {
            read: mask & read_mask != 0,
            write: mask & write_mask != 0,
            exec: mask & exec_mask != 0 || (!is_dir && (mask & FILE_GENERIC_EXECUTE != 0)),
        }
    }

    fn mask_for_sid(
        dacl: *mut ACL,
        sd: &SecurityDescriptor,
        sid: *mut core::ffi::c_void,
        is_dir: bool,
    ) -> Result<u32, String> {
        if sid.is_null() {
            return Ok(0);
        }
        let mut present = 0i32;
        let mut defaulted = 0i32;
        let mut actual_dacl = dacl;
        let ok = unsafe {
            GetSecurityDescriptorDacl(sd.raw, &mut present, &mut actual_dacl, &mut defaulted)
        };
        if ok == 0 {
            return Err("GetSecurityDescriptorDacl failed".into());
        }
        if present == 0 || actual_dacl.is_null() {
            return Ok(FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_GENERIC_EXECUTE);
        }

        let mut allow = 0u32;
        let mut mapping = GENERIC_MAPPING {
            GenericRead: FILE_GENERIC_READ,
            GenericWrite: FILE_GENERIC_WRITE,
            GenericExecute: FILE_GENERIC_EXECUTE,
            GenericAll: FILE_ALL_ACCESS,
        };

        let count = unsafe { (*actual_dacl).AceCount };
        for idx in 0..count {
            let mut ace_ptr: *mut core::ffi::c_void = ptr::null_mut();
            let ok = unsafe { GetAce(actual_dacl, idx as u32, &mut ace_ptr) };
            if ok == 0 || ace_ptr.is_null() {
                return Err("GetAce failed".into());
            }
            let header = unsafe { *(ace_ptr as *const ACE_HEADER) };
            match header.AceType {
                t if t == ACE_TYPE_ALLOWED => {
                    let ace = unsafe { &*(ace_ptr as *const ACCESS_ALLOWED_ACE) };
                    let ace_sid = &ace.SidStart as *const u32 as *mut core::ffi::c_void;
                    if unsafe { EqualSid(sid, ace_sid) } != 0 {
                        let mut mask = ace.Mask;
                        unsafe {
                            MapGenericMask(&mut mask, &mut mapping);
                        }
                        allow |= mask;
                    }
                }
                t if t == ACE_TYPE_DENIED => {
                    let ace = unsafe { &*(ace_ptr as *const ACCESS_DENIED_ACE) };
                    let ace_sid = &ace.SidStart as *const u32 as *mut core::ffi::c_void;
                    if unsafe { EqualSid(sid, ace_sid) } != 0 {
                        let mut mask = ace.Mask;
                        unsafe {
                            MapGenericMask(&mut mask, &mut mapping);
                        }
                        allow &= !mask;
                    }
                }
                _ => {}
            }
        }
        Ok(allow)
    }

    pub fn read_bits(path: &Path, is_dir: bool) -> Result<PrincipalBits, String> {
        let (sd, dacl, owner_sid, group_sid) = fetch_security(path)?;
        let owner_mask = mask_for_sid(dacl, &sd, owner_sid, is_dir)?;
        let group_mask: Option<u32> = if group_sid.is_null() {
            None
        } else {
            Some(mask_for_sid(dacl, &sd, group_sid, is_dir)?)
        };
        let everyone_sid = everyone_sid()?;
        let everyone_mask = mask_for_sid(dacl, &sd, everyone_sid.as_ptr() as *mut _, is_dir)?;

        Ok(PrincipalBits {
            owner: bits_from_mask(owner_mask, is_dir),
            group: group_mask.map(|m| bits_from_mask(m, is_dir)),
            everyone: bits_from_mask(everyone_mask, is_dir),
        })
    }

    fn trustee_for_sid(sid: *mut core::ffi::c_void, ttype: TRUSTEE_TYPE) -> TRUSTEE_W {
        TRUSTEE_W {
            pMultipleTrustee: ptr::null_mut(),
            MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: ttype,
            ptstrName: sid as *mut u16,
        }
    }

    pub fn apply_bits(path: &Path, is_dir: bool, bits: &PrincipalBits) -> Result<(), String> {
        let (sd, dacl, owner_sid, group_sid) = fetch_security(path)?;
        let everyone_sid = everyone_sid()?;
        let everyone_sid_ptr = everyone_sid.as_ptr() as *mut core::ffi::c_void;

        let owner_mask = mask_from_bits(&bits.owner);
        let group_mask = bits.group.as_ref().map(mask_from_bits);
        let everyone_mask = mask_from_bits(&bits.everyone);

        let mut entries: Vec<EXPLICIT_ACCESS_W> = Vec::with_capacity(6);
        entries.push(EXPLICIT_ACCESS_W {
            grfAccessPermissions: 0,
            grfAccessMode: REVOKE_ACCESS,
            grfInheritance: NO_INHERITANCE,
            Trustee: trustee_for_sid(owner_sid, TRUSTEE_IS_USER),
        });
        entries.push(EXPLICIT_ACCESS_W {
            grfAccessPermissions: owner_mask,
            grfAccessMode: SET_ACCESS,
            grfInheritance: NO_INHERITANCE,
            Trustee: trustee_for_sid(owner_sid, TRUSTEE_IS_USER),
        });

        if let Some(mask) = group_mask {
            if !group_sid.is_null() {
                entries.push(EXPLICIT_ACCESS_W {
                    grfAccessPermissions: 0,
                    grfAccessMode: REVOKE_ACCESS,
                    grfInheritance: NO_INHERITANCE,
                    Trustee: trustee_for_sid(group_sid, TRUSTEE_IS_GROUP),
                });
                entries.push(EXPLICIT_ACCESS_W {
                    grfAccessPermissions: mask,
                    grfAccessMode: SET_ACCESS,
                    grfInheritance: NO_INHERITANCE,
                    Trustee: trustee_for_sid(group_sid, TRUSTEE_IS_GROUP),
                });
            }
        }

        entries.push(EXPLICIT_ACCESS_W {
            grfAccessPermissions: 0,
            grfAccessMode: REVOKE_ACCESS,
            grfInheritance: NO_INHERITANCE,
            Trustee: trustee_for_sid(everyone_sid_ptr, TRUSTEE_IS_WELL_KNOWN_GROUP),
        });
        entries.push(EXPLICIT_ACCESS_W {
            grfAccessPermissions: everyone_mask,
            grfAccessMode: SET_ACCESS,
            grfInheritance: NO_INHERITANCE,
            Trustee: trustee_for_sid(everyone_sid_ptr, TRUSTEE_IS_WELL_KNOWN_GROUP),
        });

        let mut new_acl: *mut ACL = ptr::null_mut();
        let set_status =
            unsafe { SetEntriesInAclW(entries.len() as u32, entries.as_ptr(), dacl, &mut new_acl) };
        if set_status != ERROR_SUCCESS {
            return Err(format!("SetEntriesInAclW failed: Win32 error {set_status}"));
        }
        let new_acl_guard = LocalAcl { raw: new_acl };

        let mut wide = to_wide(path);
        let set_status = unsafe {
            SetNamedSecurityInfoW(
                wide.as_mut_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                ptr::null_mut(),
                ptr::null_mut(),
                new_acl,
                ptr::null_mut(),
            )
        };
        if set_status != ERROR_SUCCESS {
            return Err(format!(
                "SetNamedSecurityInfoW failed: Win32 error {set_status}"
            ));
        }

        drop(new_acl_guard);
        Ok(())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn temp_file(prefix: &str) -> std::path::PathBuf {
        let unique = format!(
            "{}-{}-{}",
            prefix,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        std::env::temp_dir().join(unique)
    }

    #[test]
    fn read_only_toggle_does_not_grant_world_write() {
        let path = temp_file("perm-ro");
        fs::write(&path, b"test").unwrap();
        fs::set_permissions(&path, PermissionsExt::from_mode(0o664)).unwrap();

        set_permissions_batch(
            vec![path.to_string_lossy().to_string()],
            Some(true),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let after_ro = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(after_ro & 0o222, 0o020); // only owner write cleared

        set_permissions_batch(
            vec![path.to_string_lossy().to_string()],
            Some(false),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let after_restore = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(after_restore & 0o222, 0o220); // original writes restored

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn executable_toggle_sets_owner_only() {
        let path = temp_file("perm-exec");
        fs::write(&path, b"test").unwrap();
        fs::set_permissions(&path, PermissionsExt::from_mode(0o654)).unwrap(); // owner no exec, group exec

        set_permissions_batch(
            vec![path.to_string_lossy().to_string()],
            None,
            Some(true),
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let after_exec = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(after_exec & 0o111, 0o110); // owner + existing group preserved

        set_permissions_batch(
            vec![path.to_string_lossy().to_string()],
            None,
            Some(false),
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let after_clear = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(after_clear & 0o111, 0o010); // only owner exec cleared; group exec stays

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn owner_group_other_bits_update() {
        let path = temp_file("perm-access");
        fs::write(&path, b"test").unwrap();
        fs::set_permissions(&path, PermissionsExt::from_mode(0o750)).unwrap();

        // Enable other read + owner exec without reintroducing world write.
        set_permissions_batch(
            vec![path.to_string_lossy().to_string()],
            None,
            None,
            None,
            None,
            Some(AccessUpdate {
                read: Some(true),
                write: Some(false),
                exec: Some(false),
            }),
            None,
        )
        .unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o004, 0o004);
        assert_eq!(mode & 0o002, 0);
        assert_eq!(mode & 0o001, 0);

        set_permissions_batch(
            vec![path.to_string_lossy().to_string()],
            None,
            None,
            Some(AccessUpdate {
                read: None,
                write: None,
                exec: Some(true),
            }),
            None,
            None,
            None,
        )
        .unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o100, 0o100);
    }
}
