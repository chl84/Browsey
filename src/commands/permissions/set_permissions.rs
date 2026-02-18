use std::fs::{self, Permissions};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use tracing::{debug, warn};

#[cfg(target_os = "linux")]
use crate::undo::set_unix_mode_nofollow;
use crate::{
    fs_utils::{check_no_symlink_components, sanitize_path_nofollow},
    undo::{apply_permissions, permissions_snapshot},
};

#[cfg(target_os = "windows")]
use super::windows_acl;
#[cfg(target_os = "windows")]
use super::AccessBits;
use super::{
    ensure_absolute_path,
    error::{PermissionsError, PermissionsErrorCode, PermissionsResult},
    permission_info_fallback, refresh_permissions_after_apply, AccessUpdate, PermissionInfo,
};

#[derive(Clone)]
struct PermissionRollback {
    path: PathBuf,
    before: crate::undo::PermissionsSnapshot,
}

fn rollback_permissions_actions(actions: &[PermissionRollback]) -> PermissionsResult<()> {
    if actions.is_empty() {
        return Ok(());
    }
    let mut errors: Vec<String> = Vec::new();
    for action in actions.iter().rev() {
        if let Err(err) = apply_permissions(&action.path, &action.before) {
            errors.push(format!("{}: {err}", action.path.display()));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        let joined = errors.join("; ");
        warn!(error = %joined, "permissions rollback failed");
        Err(PermissionsError::new(
            PermissionsErrorCode::RollbackFailed,
            joined,
        ))
    }
}

#[cfg(unix)]
pub(super) fn set_permissions_batch(
    paths: Vec<String>,
    read_only: Option<bool>,
    executable: Option<bool>,
    owner: Option<AccessUpdate>,
    group: Option<AccessUpdate>,
    other: Option<AccessUpdate>,
) -> PermissionsResult<PermissionInfo> {
    let has_access_updates = owner.is_some() || group.is_some() || other.is_some();
    if read_only.is_none() && executable.is_none() && !has_access_updates {
        return Err(PermissionsError::invalid_input(
            "No permission changes were provided",
        ));
    }

    let first_path = paths.first().cloned();
    let mut rollbacks: Vec<PermissionRollback> = Vec::with_capacity(paths.len());

    for path in paths {
        let owner_update = owner.clone();
        let group_update = group.clone();
        let other_update = other.clone();
        let apply_result: PermissionsResult<()> = (|| {
            ensure_absolute_path(&path)?;
            debug!(
                path = %path,
                read_only = ?read_only,
                executable = ?executable,
                "set_permissions start"
            );
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
                #[cfg(target_os = "linux")]
                {
                    set_unix_mode_nofollow(&target, mode).map_err(|e| {
                        PermissionsError::new(
                            PermissionsErrorCode::PermissionsUpdateFailed,
                            format!("Failed to update permissions: {e}"),
                        )
                    })?;
                }
                #[cfg(not(target_os = "linux"))]
                {
                    fs::set_permissions(&target, perms).map_err(|e| {
                        PermissionsError::from_io_error(
                            PermissionsErrorCode::PermissionsUpdateFailed,
                            "Failed to update permissions",
                            e,
                        )
                    })?;
                }
                match permissions_snapshot(&target) {
                    Ok(_) => {}
                    Err(snapshot_err) => match apply_permissions(&target, &before) {
                        Ok(()) => {
                            return Err(PermissionsError::new(
                                PermissionsErrorCode::PostChangeSnapshotFailed,
                                format!(
                                    "Failed to capture post-change permissions for {}: {}; current target rolled back",
                                    target.display(),
                                    snapshot_err
                                ),
                            ));
                        }
                        Err(rollback_err) => {
                            return Err(PermissionsError::new(
                                PermissionsErrorCode::PostChangeSnapshotFailed,
                                format!(
                                    "Failed to capture post-change permissions for {}: {}; rollback failed ({rollback_err}). System may be partially changed",
                                    target.display(),
                                    snapshot_err
                                ),
                            ));
                        }
                    },
                }
                rollbacks.push(PermissionRollback {
                    path: target.clone(),
                    before,
                });
            }
            Ok(())
        })();

        if let Err(err) = apply_result {
            warn!(path = %path, error = %err, "set_permissions failed");
            if let Err(rollback_err) = rollback_permissions_actions(&rollbacks) {
                return Err(PermissionsError::new(
                    PermissionsErrorCode::RollbackFailed,
                    format!(
                        "{err}; rollback failed ({rollback_err}). System may be partially changed"
                    ),
                ));
            }
            return Err(err);
        }
    }

    let changed_any = !rollbacks.is_empty();

    if let Some(path) = first_path {
        return refresh_permissions_after_apply(path, changed_any);
    }

    Ok(permission_info_fallback())
}

#[cfg(target_os = "windows")]
pub(super) fn set_permissions_batch(
    paths: Vec<String>,
    read_only: Option<bool>,
    executable: Option<bool>,
    owner: Option<AccessUpdate>,
    group: Option<AccessUpdate>,
    other: Option<AccessUpdate>,
) -> PermissionsResult<PermissionInfo> {
    let has_access_updates = owner.is_some() || group.is_some() || other.is_some();
    if read_only.is_none() && executable.is_none() && !has_access_updates {
        return Err(PermissionsError::invalid_input(
            "No permission changes were provided",
        ));
    }

    let first_path = paths.first().cloned();
    let mut rollbacks: Vec<PermissionRollback> = Vec::with_capacity(paths.len());

    for path in paths {
        let owner_update = owner.clone();
        let group_update = group.clone();
        let other_update = other.clone();
        let apply_result: PermissionsResult<()> = (|| {
            ensure_absolute_path(&path)?;
            debug!(
                path = %path,
                read_only = ?read_only,
                executable = ?executable,
                "set_permissions start"
            );
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
                    return Err(PermissionsError::new(
                        PermissionsErrorCode::GroupUnavailable,
                        "Group information is unavailable for this entry",
                    ));
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
                    fs::set_permissions(&target, perms.clone()).map_err(|e| {
                        PermissionsError::from_io_error(
                            PermissionsErrorCode::PermissionsUpdateFailed,
                            "Failed to update permissions",
                            e,
                        )
                    })?;
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
                    return Err(PermissionsError::from_io_error(
                        PermissionsErrorCode::PermissionsUpdateFailed,
                        "Failed to update permissions",
                        e,
                    ));
                }
                changed = true;
            }

            if changed {
                match permissions_snapshot(&target) {
                    Ok(_) => {}
                    Err(snapshot_err) => match apply_permissions(&target, &before) {
                        Ok(()) => {
                            return Err(PermissionsError::new(
                                PermissionsErrorCode::PostChangeSnapshotFailed,
                                format!(
                                    "Failed to capture post-change permissions for {}: {}; current target rolled back",
                                    target.display(),
                                    snapshot_err
                                ),
                            ));
                        }
                        Err(rollback_err) => {
                            return Err(PermissionsError::new(
                                PermissionsErrorCode::PostChangeSnapshotFailed,
                                format!(
                                    "Failed to capture post-change permissions for {}: {}; rollback failed ({rollback_err}). System may be partially changed",
                                    target.display(),
                                    snapshot_err
                                ),
                            ));
                        }
                    },
                }
                rollbacks.push(PermissionRollback {
                    path: target.clone(),
                    before,
                });
            }
            Ok(())
        })();

        if let Err(err) = apply_result {
            warn!(path = %path, error = %err, "set_permissions failed");
            if let Err(rollback_err) = rollback_permissions_actions(&rollbacks) {
                return Err(PermissionsError::new(
                    PermissionsErrorCode::RollbackFailed,
                    format!(
                        "{err}; rollback failed ({rollback_err}). System may be partially changed"
                    ),
                ));
            }
            return Err(err);
        }
    }

    let changed_any = !rollbacks.is_empty();

    if let Some(path) = first_path {
        return refresh_permissions_after_apply(path, changed_any);
    }

    Ok(permission_info_fallback())
}

#[cfg(not(any(unix, target_os = "windows")))]
pub(super) fn set_permissions_batch(
    paths: Vec<String>,
    read_only: Option<bool>,
    executable: Option<bool>,
    owner: Option<AccessUpdate>,
    group: Option<AccessUpdate>,
    other: Option<AccessUpdate>,
) -> PermissionsResult<PermissionInfo> {
    let _ = (paths, executable, owner, group, other);
    if let Some(ro) = read_only {
        return Err(PermissionsError::new(
            PermissionsErrorCode::UnsupportedPlatform,
            format!(
                "Permission changes are not supported on this platform (requested readOnly={ro})"
            ),
        ));
    }
    Err(PermissionsError::new(
        PermissionsErrorCode::UnsupportedPlatform,
        "Permission changes are not supported on this platform",
    ))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    fn temp_path(prefix: &str) -> PathBuf {
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
    fn rollback_permissions_actions_reports_partial_failure() {
        let path = temp_path("perm-rollback-partial-ok");
        let missing = temp_path("perm-rollback-partial-missing");
        fs::write(&path, b"test").unwrap();
        fs::set_permissions(&path, PermissionsExt::from_mode(0o600)).unwrap();
        let before = permissions_snapshot(&path).unwrap();
        fs::set_permissions(&path, PermissionsExt::from_mode(0o640)).unwrap();

        let actions = vec![
            PermissionRollback {
                path: path.clone(),
                before: before.clone(),
            },
            PermissionRollback {
                path: missing.clone(),
                before,
            },
        ];

        let err = rollback_permissions_actions(&actions).unwrap_err();
        assert!(err.to_string().contains(missing.to_string_lossy().as_ref()));

        let restored_mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(restored_mode, 0o600);

        let _ = fs::remove_file(&path);
    }
}
