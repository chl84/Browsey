#![allow(dead_code, unused_variables)]

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use tracing::{debug, warn};

use crate::errors::api_error::{ApiError, ApiResult};
use crate::fs_utils::{check_no_symlink_components, sanitize_path_nofollow};

mod error;
mod ownership;
mod set_permissions;
#[cfg(all(test, unix))]
mod tests;
#[cfg(target_os = "windows")]
mod windows_acl;

use self::error::{
    is_expected_batch_error, map_api_result, PermissionsError, PermissionsErrorCode,
    PermissionsResult,
};
use ownership::{
    list_ownership_principals as list_ownership_principals_impl, set_ownership_batch,
    OwnershipPrincipalKind,
};
use set_permissions::set_permissions_batch;

pub use ownership::maybe_run_ownership_helper_from_args;

pub const OWNERSHIP_HELPER_FLAG: &str = "--browsey-ownership-helper";

pub(super) fn ensure_absolute_path(raw: &str) -> PermissionsResult<()> {
    if Path::new(raw).is_absolute() {
        Ok(())
    } else {
        Err(PermissionsError::path_not_absolute(raw))
    }
}

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
    pub ownership_supported: bool,
    pub owner_name: Option<String>,
    pub group_name: Option<String>,
    pub owner: Option<AccessBits>,
    pub group: Option<AccessBits>,
    pub other: Option<AccessBits>,
}

#[derive(Debug, serde::Serialize, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum AggregatedAccessBit {
    Bool(bool),
    Mixed(String),
}

#[derive(serde::Serialize)]
pub struct AggregatedAccess {
    pub read: AggregatedAccessBit,
    pub write: AggregatedAccessBit,
    pub exec: AggregatedAccessBit,
}

#[derive(serde::Serialize)]
pub struct PermissionsBatchAggregate {
    pub access_supported: bool,
    pub executable_supported: bool,
    pub ownership_supported: bool,
    pub read_only: Option<AggregatedAccessBit>,
    pub executable: Option<AggregatedAccessBit>,
    pub owner_name: Option<String>,
    pub group_name: Option<String>,
    pub owner: Option<AggregatedAccess>,
    pub group: Option<AggregatedAccess>,
    pub other: Option<AggregatedAccess>,
}

#[derive(serde::Serialize)]
pub struct PermissionsBatchItem {
    pub path: String,
    pub ok: bool,
    pub permissions: PermissionInfo,
    pub error: Option<ApiError>,
}

#[derive(serde::Serialize)]
pub struct PermissionsBatchResult {
    pub per_item: Vec<PermissionsBatchItem>,
    pub aggregate: PermissionsBatchAggregate,
    pub failures: usize,
    pub unexpected_failures: usize,
}

pub(super) fn permission_info_fallback() -> PermissionInfo {
    #[cfg(target_os = "windows")]
    {
        PermissionInfo {
            read_only: false,
            executable: None,
            executable_supported: true,
            access_supported: true,
            ownership_supported: false,
            owner_name: None,
            group_name: None,
            owner: None,
            group: None,
            other: None,
        }
    }
    #[cfg(unix)]
    {
        PermissionInfo {
            read_only: false,
            executable: None,
            executable_supported: true,
            access_supported: true,
            ownership_supported: true,
            owner_name: None,
            group_name: None,
            owner: None,
            group: None,
            other: None,
        }
    }
    #[cfg(not(any(unix, target_os = "windows")))]
    {
        PermissionInfo {
            read_only: false,
            executable: None,
            executable_supported: false,
            access_supported: false,
            ownership_supported: false,
            owner_name: None,
            group_name: None,
            owner: None,
            group: None,
            other: None,
        }
    }
}

pub(super) fn permission_info_unsupported() -> PermissionInfo {
    PermissionInfo {
        read_only: false,
        executable: None,
        executable_supported: false,
        access_supported: false,
        ownership_supported: false,
        owner_name: None,
        group_name: None,
        owner: None,
        group: None,
        other: None,
    }
}

fn is_virtual_uri_path(path: &str) -> bool {
    let trimmed = path.trim();
    let Some(idx) = trimmed.find("://") else {
        return false;
    };
    if idx == 0 {
        return false;
    }

    let scheme = &trimmed[..idx];
    let mut chars = scheme.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() => {}
        _ => return false,
    }

    chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
}

fn combine_bool(values: &[bool]) -> AggregatedAccessBit {
    if values.is_empty() {
        return AggregatedAccessBit::Bool(false);
    }
    if values.iter().all(|value| *value) {
        return AggregatedAccessBit::Bool(true);
    }
    if values.iter().all(|value| !*value) {
        return AggregatedAccessBit::Bool(false);
    }
    AggregatedAccessBit::Mixed("mixed".to_string())
}

fn combine_principal<'a>(values: impl Iterator<Item = Option<&'a str>>) -> Option<String> {
    let normalized: Vec<String> = values
        .map(|value: Option<&str>| value.map(str::trim).unwrap_or("").to_string())
        .collect();
    if normalized.is_empty() {
        return None;
    }

    let mut unique: Vec<String> = normalized
        .iter()
        .filter_map(|value: &String| {
            if value.is_empty() {
                None
            } else {
                Some(value.clone())
            }
        })
        .collect();
    unique.sort();
    unique.dedup();

    if unique.is_empty() {
        return None;
    }
    if unique.len() == 1 && normalized.iter().all(|value| value == &unique[0]) {
        Some(unique[0].clone())
    } else {
        Some("mixed".to_string())
    }
}

fn aggregate_permissions(items: &[PermissionsBatchItem]) -> PermissionsBatchAggregate {
    let access_supported = items.iter().all(|item| item.permissions.access_supported);
    let executable_supported = items
        .iter()
        .all(|item| item.permissions.executable_supported);
    let ownership_supported = items
        .iter()
        .all(|item| item.permissions.ownership_supported);

    let owner_reads: Vec<bool> = items
        .iter()
        .filter_map(|item| item.permissions.owner.as_ref().map(|value| value.read))
        .collect();
    let owner_writes: Vec<bool> = items
        .iter()
        .filter_map(|item| item.permissions.owner.as_ref().map(|value| value.write))
        .collect();
    let owner_execs: Vec<bool> = items
        .iter()
        .filter_map(|item| item.permissions.owner.as_ref().map(|value| value.exec))
        .collect();

    let group_reads: Vec<bool> = items
        .iter()
        .filter_map(|item| item.permissions.group.as_ref().map(|value| value.read))
        .collect();
    let group_writes: Vec<bool> = items
        .iter()
        .filter_map(|item| item.permissions.group.as_ref().map(|value| value.write))
        .collect();
    let group_execs: Vec<bool> = items
        .iter()
        .filter_map(|item| item.permissions.group.as_ref().map(|value| value.exec))
        .collect();

    let other_reads: Vec<bool> = items
        .iter()
        .filter_map(|item| item.permissions.other.as_ref().map(|value| value.read))
        .collect();
    let other_writes: Vec<bool> = items
        .iter()
        .filter_map(|item| item.permissions.other.as_ref().map(|value| value.write))
        .collect();
    let other_execs: Vec<bool> = items
        .iter()
        .filter_map(|item| item.permissions.other.as_ref().map(|value| value.exec))
        .collect();

    let read_only_values: Vec<bool> = items
        .iter()
        .map(|item| item.permissions.read_only)
        .collect();
    let executable_values: Vec<bool> = items
        .iter()
        .filter_map(|item| item.permissions.executable)
        .collect();

    PermissionsBatchAggregate {
        access_supported,
        executable_supported,
        ownership_supported,
        read_only: if access_supported {
            Some(combine_bool(&read_only_values))
        } else {
            None
        },
        executable: if executable_supported {
            Some(combine_bool(&executable_values))
        } else {
            None
        },
        owner_name: combine_principal(
            items
                .iter()
                .map(|item| item.permissions.owner_name.as_deref()),
        ),
        group_name: combine_principal(
            items
                .iter()
                .map(|item| item.permissions.group_name.as_deref()),
        ),
        owner: if access_supported {
            Some(AggregatedAccess {
                read: combine_bool(&owner_reads),
                write: combine_bool(&owner_writes),
                exec: combine_bool(&owner_execs),
            })
        } else {
            None
        },
        group: if access_supported {
            Some(AggregatedAccess {
                read: combine_bool(&group_reads),
                write: combine_bool(&group_writes),
                exec: combine_bool(&group_execs),
            })
        } else {
            None
        },
        other: if access_supported {
            Some(AggregatedAccess {
                read: combine_bool(&other_reads),
                write: combine_bool(&other_writes),
                exec: combine_bool(&other_execs),
            })
        } else {
            None
        },
    }
}

pub(super) fn refresh_permissions_after_apply(
    path: String,
    changed_any: bool,
) -> PermissionsResult<PermissionInfo> {
    match get_permissions_impl(path.clone()) {
        Ok(info) => Ok(info),
        Err(err) if changed_any => {
            warn!(
                path = %path,
                error = %err,
                "permission refresh failed after successful apply; returning fallback"
            );
            Ok(permission_info_fallback())
        }
        Err(err) => Err(err),
    }
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

#[tauri::command]
pub fn get_permissions(path: String) -> ApiResult<PermissionInfo> {
    map_api_result(get_permissions_impl(path))
}

pub(super) fn get_permissions_impl(path: String) -> PermissionsResult<PermissionInfo> {
    debug!(path = %path, "get_permissions start");
    ensure_absolute_path(&path)?;
    let target = sanitize_path_nofollow(&path, true).map_err(PermissionsError::from)?;
    check_no_symlink_components(&target).map_err(PermissionsError::from)?;
    let meta = fs::symlink_metadata(&target).map_err(|e| {
        PermissionsError::from_io_error(
            PermissionsErrorCode::MetadataReadFailed,
            "Failed to read metadata",
            e,
        )
    })?;
    if meta.file_type().is_symlink() {
        return Err(PermissionsError::new(
            PermissionsErrorCode::SymlinkUnsupported,
            "Permissions are not supported on symlinks",
        ));
    }
    debug!(path = %target.display(), "get_permissions resolved target");

    #[cfg(target_os = "windows")]
    {
        let bits = windows_acl::read_bits(&target, meta.is_dir())?;
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
        let read_only = meta.permissions().readonly();
        let executable = is_executable(&meta);
        #[cfg(unix)]
        let (owner_name, group_name) = resolve_owner_group_names(&meta);
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

pub(super) fn get_permissions_batch_impl(
    paths: Vec<String>,
) -> PermissionsResult<PermissionsBatchResult> {
    if paths.is_empty() {
        return Err(PermissionsError::invalid_input("No paths provided"));
    }

    let mut per_item: Vec<PermissionsBatchItem> = Vec::with_capacity(paths.len());
    let mut failures = 0usize;
    let mut unexpected_failures = 0usize;

    for path in paths {
        if is_virtual_uri_path(&path) {
            per_item.push(PermissionsBatchItem {
                path,
                ok: true,
                permissions: permission_info_unsupported(),
                error: None,
            });
            continue;
        }

        match get_permissions_impl(path.clone()) {
            Ok(permissions) => per_item.push(PermissionsBatchItem {
                path,
                ok: true,
                permissions,
                error: None,
            }),
            Err(error) => {
                failures += 1;
                if !is_expected_batch_error(&error) {
                    unexpected_failures += 1;
                }
                per_item.push(PermissionsBatchItem {
                    path,
                    ok: false,
                    permissions: permission_info_unsupported(),
                    error: Some(error.to_api_error()),
                });
            }
        }
    }

    let aggregate = aggregate_permissions(&per_item);
    Ok(PermissionsBatchResult {
        per_item,
        aggregate,
        failures,
        unexpected_failures,
    })
}

#[tauri::command]
pub fn get_permissions_batch(paths: Vec<String>) -> ApiResult<PermissionsBatchResult> {
    map_api_result(get_permissions_batch_impl(paths))
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn set_permissions(
    path: Option<String>,
    paths: Option<Vec<String>>,
    #[allow(non_snake_case)] readOnly: Option<bool>,
    read_only: Option<bool>,
    executable: Option<bool>,
    owner: Option<AccessUpdate>,
    group: Option<AccessUpdate>,
    other: Option<AccessUpdate>,
) -> ApiResult<PermissionInfo> {
    let targets: Vec<String> = match (paths, path) {
        (Some(list), _) if !list.is_empty() => list,
        (_, Some(single)) => vec![single],
        _ => return map_api_result(Err(PermissionsError::invalid_input("No paths provided"))),
    };
    map_api_result(set_permissions_batch(
        targets,
        readOnly.or(read_only),
        executable,
        owner,
        group,
        other,
    ))
}

#[tauri::command]
pub fn set_ownership(
    path: Option<String>,
    paths: Option<Vec<String>>,
    owner: Option<String>,
    group: Option<String>,
) -> ApiResult<PermissionInfo> {
    let targets: Vec<String> = match (paths, path) {
        (Some(list), _) if !list.is_empty() => list,
        (_, Some(single)) => vec![single],
        _ => return map_api_result(Err(PermissionsError::invalid_input("No paths provided"))),
    };
    map_api_result(set_ownership_batch(targets, owner, group))
}

#[tauri::command]
pub fn list_ownership_principals(
    kind: OwnershipPrincipalKind,
    query: Option<String>,
    limit: Option<usize>,
) -> ApiResult<Vec<String>> {
    map_api_result(list_ownership_principals_impl(kind, query, limit))
}
