#![allow(dead_code, unused_variables)]
use std::fs::{self, Permissions};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use tracing::{debug, warn};

use crate::{
    fs_utils::{check_no_symlink_components, sanitize_path_follow, sanitize_path_nofollow},
    undo::{permissions_snapshot, Action, PermissionsSnapshot, UndoState},
};

#[derive(serde::Serialize)]
pub struct AccessBits {
    pub read: bool,
    pub write: bool,
    pub exec: bool,
}

#[derive(serde::Deserialize)]
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
    let meta = fs::symlink_metadata(&nofollow)
        .map_err(|e| format!("Failed to read metadata: {e}"))?;
    if meta.file_type().is_symlink() {
        return Err("Permissions are not supported on symlinks".into());
    }
    let target = sanitize_path_follow(&path, true)?;
    check_no_symlink_components(&target)?;
    debug!(path = %target.display(), "get_permissions resolved target");

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

    Ok(PermissionInfo {
        read_only,
        executable,
        executable_supported: executable.is_some(),
        access_supported,
        owner,
        group,
        other,
    })
}

fn set_permissions_impl(
    path: String,
    #[allow(non_snake_case)] readOnly: Option<bool>,
    read_only: Option<bool>,
    executable: Option<bool>,
    owner: Option<AccessUpdate>,
    group: Option<AccessUpdate>,
    other: Option<AccessUpdate>,
    undo: Option<&UndoState>,
) -> Result<PermissionInfo, String> {
    let read_only = read_only.or(readOnly);
    let has_access_updates = owner.is_some() || group.is_some() || other.is_some();
    if read_only.is_none() && executable.is_none() && !has_access_updates {
        return Err("No permission changes were provided".into());
    }
    debug!(
        path = %path,
        read_only = ?read_only,
        executable = ?executable,
        "set_permissions start"
    );
    let nofollow = sanitize_path_nofollow(&path, true)?;
    let meta = fs::symlink_metadata(&nofollow)
        .map_err(|e| format!("Failed to read metadata: {e}"))?;
    if meta.file_type().is_symlink() {
        return Err("Permissions are not supported on symlinks".into());
    }
    let target = sanitize_path_follow(&path, true)?;
    check_no_symlink_components(&target)?;
    debug!(path = %target.display(), "set_permissions resolved target");

    let before = permissions_snapshot(&target)?;

    let mut perms: Permissions = meta.permissions();
    #[cfg(unix)]
    {
        let mut mode = perms.mode();
        if let Some(ro) = read_only {
            if ro {
                // Strip all write bits when marking read-only.
                mode &= !0o222;
            } else {
                // Only re-enable owner write to avoid making the file world-writable.
                mode |= 0o200;
            }
        }
        if let Some(exec) = executable {
            if exec {
                // Set the owner execute bit; preserve any existing group/other bits.
                mode |= 0o100;
            } else {
                mode &= !0o111;
            }
        }
        if let Some(update) = owner {
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
        if let Some(update) = group {
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
        if let Some(update) = other {
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
        perms.set_mode(mode);
    }
    #[cfg(not(unix))]
    {
        if has_access_updates {
            return Err("Access control changes are only supported on Unix-like platforms".into());
        }
        if let Some(ro) = read_only {
            perms.set_readonly(ro);
        }
    }
    if let Err(e) = fs::set_permissions(&target, perms) {
        warn!(path = %target.display(), error = %e, "set_permissions failed");
        return Err(format!("Failed to update permissions: {e}"));
    }
    debug!(path = %target.display(), "set_permissions applied");
    let info = get_permissions(path.clone())?;
    if let Some(undo) = undo {
        let after = PermissionsSnapshot {
            readonly: info.read_only,
            #[cfg(unix)]
            mode: {
                use std::os::unix::fs::PermissionsExt;
                fs::metadata(&target)
                    .map(|m| m.permissions().mode())
                    .unwrap_or(before.mode)
            },
        };
        let _ = undo.record_applied(Action::SetPermissions {
            path: target,
            before,
            after,
        });
    }
    Ok(info)
}

#[tauri::command]
pub fn set_permissions(
    path: String,
    #[allow(non_snake_case)] readOnly: Option<bool>,
    read_only: Option<bool>,
    executable: Option<bool>,
    owner: Option<AccessUpdate>,
    group: Option<AccessUpdate>,
    other: Option<AccessUpdate>,
    undo: tauri::State<UndoState>,
) -> Result<PermissionInfo, String> {
    set_permissions_impl(
        path, readOnly, read_only, executable, owner, group, other, Some(undo.inner()),
    )
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
        fs::set_permissions(&path, PermissionsExt::from_mode(0o644)).unwrap();

        set_permissions_impl(
            path.to_string_lossy().to_string(),
            None,
            Some(true),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let after_ro = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(after_ro & 0o222, 0);

        set_permissions_impl(
            path.to_string_lossy().to_string(),
            None,
            Some(false),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let after_restore = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(after_restore & 0o222, 0o200);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn executable_toggle_sets_owner_only() {
        let path = temp_file("perm-exec");
        fs::write(&path, b"test").unwrap();
        fs::set_permissions(&path, PermissionsExt::from_mode(0o644)).unwrap();

        set_permissions_impl(
            path.to_string_lossy().to_string(),
            None,
            None,
            Some(true),
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let after_exec = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(after_exec & 0o111, 0o100);

        set_permissions_impl(
            path.to_string_lossy().to_string(),
            None,
            None,
            Some(false),
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let after_clear = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(after_clear & 0o111, 0);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn owner_group_other_bits_update() {
        let path = temp_file("perm-access");
        fs::write(&path, b"test").unwrap();
        fs::set_permissions(&path, PermissionsExt::from_mode(0o640)).unwrap();

        // Enable other read + owner exec without reintroducing world write.
        set_permissions_impl(
            path.to_string_lossy().to_string(),
            None,
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

        set_permissions_impl(
            path.to_string_lossy().to_string(),
            None,
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
