use super::{
    error::{PermissionsError, PermissionsErrorCode, PermissionsResult},
    AccessBits,
};
use std::{os::windows::ffi::OsStrExt, path::Path, ptr};
use windows_sys::Win32::Foundation::{LocalFree, ERROR_SUCCESS};
use windows_sys::Win32::Security::Authorization::{
    GetNamedSecurityInfoW, SetEntriesInAclW, SetNamedSecurityInfoW, EXPLICIT_ACCESS_W,
    NO_MULTIPLE_TRUSTEE, REVOKE_ACCESS, SET_ACCESS, SE_FILE_OBJECT, TRUSTEE_IS_GROUP,
    TRUSTEE_IS_SID, TRUSTEE_IS_USER, TRUSTEE_IS_WELL_KNOWN_GROUP, TRUSTEE_TYPE, TRUSTEE_W,
};
use windows_sys::Win32::Security::{
    CreateWellKnownSid, EqualSid, GetAce, GetSecurityDescriptorDacl, MapGenericMask, WinWorldSid,
    ACCESS_ALLOWED_ACE, ACCESS_DENIED_ACE, ACE_HEADER, ACL, DACL_SECURITY_INFORMATION,
    GENERIC_MAPPING, GROUP_SECURITY_INFORMATION, NO_INHERITANCE, OWNER_SECURITY_INFORMATION,
    PSECURITY_DESCRIPTOR, SECURITY_MAX_SID_SIZE,
};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_ALL_ACCESS, FILE_APPEND_DATA, FILE_GENERIC_EXECUTE, FILE_GENERIC_READ, FILE_GENERIC_WRITE,
    FILE_LIST_DIRECTORY, FILE_READ_DATA, FILE_TRAVERSE, FILE_WRITE_DATA,
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
) -> PermissionsResult<
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
        return Err(PermissionsError::new(
            PermissionsErrorCode::MetadataReadFailed,
            format!("GetNamedSecurityInfoW failed: Win32 error {status}"),
        ));
    }

    Ok((SecurityDescriptor { raw: sd }, dacl, owner, group))
}

fn everyone_sid() -> PermissionsResult<Vec<u8>> {
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
        return Err(PermissionsError::new(
            PermissionsErrorCode::MetadataReadFailed,
            "CreateWellKnownSid failed",
        ));
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
    _is_dir: bool,
) -> PermissionsResult<u32> {
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
        return Err(PermissionsError::new(
            PermissionsErrorCode::MetadataReadFailed,
            "GetSecurityDescriptorDacl failed",
        ));
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
            return Err(PermissionsError::new(
                PermissionsErrorCode::MetadataReadFailed,
                "GetAce failed",
            ));
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

pub fn read_bits(path: &Path, is_dir: bool) -> PermissionsResult<PrincipalBits> {
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

pub fn apply_bits(path: &Path, _is_dir: bool, bits: &PrincipalBits) -> PermissionsResult<()> {
    let (_sd, dacl, owner_sid, group_sid) = fetch_security(path)?;
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
        return Err(PermissionsError::new(
            PermissionsErrorCode::PermissionsUpdateFailed,
            format!("SetEntriesInAclW failed: Win32 error {set_status}"),
        ));
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
        return Err(PermissionsError::new(
            PermissionsErrorCode::PermissionsUpdateFailed,
            format!("SetNamedSecurityInfoW failed: Win32 error {set_status}"),
        ));
    }

    drop(new_acl_guard);
    Ok(())
}
