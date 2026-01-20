use std::fs::{self, Permissions};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use tracing::{info, warn};

use crate::fs_utils::{check_no_symlink_components, sanitize_path_follow, sanitize_path_nofollow};

#[derive(serde::Serialize)]
pub struct PermissionInfo {
    pub read_only: bool,
    pub executable: Option<bool>,
    pub executable_supported: bool,
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
    info!(path = %path, "get_permissions start");
    let nofollow = sanitize_path_nofollow(&path, true)?;
    let meta = fs::symlink_metadata(&nofollow)
        .map_err(|e| format!("Failed to read metadata: {e}"))?;
    if meta.file_type().is_symlink() {
        return Err("Permissions are not supported on symlinks".into());
    }
    let target = sanitize_path_follow(&path, true)?;
    check_no_symlink_components(&target)?;
    info!(path = %target.display(), "get_permissions resolved target");

    let read_only = meta.permissions().readonly();
    let executable = is_executable(&meta);
    Ok(PermissionInfo {
        read_only,
        executable,
        executable_supported: executable.is_some(),
    })
}

#[tauri::command]
pub fn set_permissions(
    path: String,
    #[allow(non_snake_case)] readOnly: Option<bool>,
    read_only: Option<bool>,
    executable: Option<bool>,
) -> Result<PermissionInfo, String> {
    let read_only = read_only.or(readOnly);
    if read_only.is_none() && executable.is_none() {
        return Err("No permission changes were provided".into());
    }
    info!(
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
    info!(path = %target.display(), "set_permissions resolved target");

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
        perms.set_mode(mode);
    }
    #[cfg(not(unix))]
    {
        if let Some(ro) = read_only {
            perms.set_readonly(ro);
        }
    }
    if let Err(e) = fs::set_permissions(&target, perms) {
        warn!(path = %target.display(), error = %e, "set_permissions failed");
        return Err(format!("Failed to update permissions: {e}"));
    }
    info!(path = %target.display(), "set_permissions applied");
    get_permissions(path)
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

        set_permissions(path.to_string_lossy().to_string(), None, Some(true), None).unwrap();
        let after_ro = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(after_ro & 0o222, 0);

        set_permissions(path.to_string_lossy().to_string(), None, Some(false), None).unwrap();
        let after_restore = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(after_restore & 0o222, 0o200);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn executable_toggle_sets_owner_only() {
        let path = temp_file("perm-exec");
        fs::write(&path, b"test").unwrap();
        fs::set_permissions(&path, PermissionsExt::from_mode(0o644)).unwrap();

        set_permissions(path.to_string_lossy().to_string(), None, None, Some(true)).unwrap();
        let after_exec = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(after_exec & 0o111, 0o100);

        set_permissions(path.to_string_lossy().to_string(), None, None, Some(false)).unwrap();
        let after_clear = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(after_clear & 0o111, 0);

        let _ = fs::remove_file(&path);
    }
}
