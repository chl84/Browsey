use std::fs::{self, Permissions};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use tracing::{debug, warn};

use crate::{
    fs_utils::{check_no_symlink_components, sanitize_path_nofollow},
    undo::{permissions_snapshot, run_actions, Action, Direction, UndoState},
};

#[cfg(target_os = "windows")]
use super::windows_acl;
#[cfg(target_os = "windows")]
use super::AccessBits;
use super::{ensure_absolute_path, get_permissions, AccessUpdate, PermissionInfo};

fn rollback_permissions_actions(actions: &[Action]) -> Result<(), String> {
    if actions.is_empty() {
        return Ok(());
    }
    let mut rev = actions.to_vec();
    run_actions(&mut rev, Direction::Backward).map_err(|e| {
        warn!(error = %e, "permissions rollback failed");
        e
    })
}

#[cfg(unix)]
pub(super) fn set_permissions_batch(
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

    let first_path = paths.first().cloned();
    let mut actions: Vec<Action> = Vec::with_capacity(paths.len());

    for path in paths {
        let owner_update = owner.clone();
        let group_update = group.clone();
        let other_update = other.clone();
        let apply_result: Result<(), String> = (|| {
            ensure_absolute_path(&path)?;
            debug!(
                path = %path,
                read_only = ?read_only,
                executable = ?executable,
                "set_permissions start"
            );
            let target = sanitize_path_nofollow(&path, true)?;
            check_no_symlink_components(&target)?;
            let meta = fs::symlink_metadata(&target)
                .map_err(|e| format!("Failed to read metadata: {e}"))?;
            if meta.file_type().is_symlink() {
                return Err("Permissions are not supported on symlinks".into());
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
                fs::set_permissions(&target, perms)
                    .map_err(|e| format!("Failed to update permissions: {e}"))?;
                let after = permissions_snapshot(&target)?;
                actions.push(Action::SetPermissions {
                    path: target.clone(),
                    before,
                    after,
                });
            }
            Ok(())
        })();

        if let Err(err) = apply_result {
            warn!(path = %path, error = %err, "set_permissions failed");
            if let Err(rollback_err) = rollback_permissions_actions(&actions) {
                return Err(format!(
                    "{err}; rollback failed ({rollback_err}). System may be partially changed"
                ));
            }
            return Err(err);
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

    if let Some(path) = first_path {
        return get_permissions(path);
    }

    Ok(PermissionInfo {
        read_only: false,
        executable: None,
        executable_supported: cfg!(unix),
        access_supported: cfg!(unix),
        ownership_supported: cfg!(unix),
        owner_name: None,
        group_name: None,
        owner: None,
        group: None,
        other: None,
    })
}

#[cfg(target_os = "windows")]
pub(super) fn set_permissions_batch(
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

    let first_path = paths.first().cloned();
    let mut actions: Vec<Action> = Vec::with_capacity(paths.len());

    for path in paths {
        let owner_update = owner.clone();
        let group_update = group.clone();
        let other_update = other.clone();
        let apply_result: Result<(), String> = (|| {
            ensure_absolute_path(&path)?;
            debug!(
                path = %path,
                read_only = ?read_only,
                executable = ?executable,
                "set_permissions start"
            );
            let target = sanitize_path_nofollow(&path, true)?;
            check_no_symlink_components(&target)?;
            let meta = fs::symlink_metadata(&target)
                .map_err(|e| format!("Failed to read metadata: {e}"))?;
            if meta.file_type().is_symlink() {
                return Err("Permissions are not supported on symlinks".into());
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
                    fs::set_permissions(&target, perms.clone())
                        .map_err(|e| format!("Failed to update permissions: {e}"))?;
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
            }
            Ok(())
        })();

        if let Err(err) = apply_result {
            warn!(path = %path, error = %err, "set_permissions failed");
            if let Err(rollback_err) = rollback_permissions_actions(&actions) {
                return Err(format!(
                    "{err}; rollback failed ({rollback_err}). System may be partially changed"
                ));
            }
            return Err(err);
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

    if let Some(path) = first_path {
        return get_permissions(path);
    }

    Ok(PermissionInfo {
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
    })
}

#[cfg(not(any(unix, target_os = "windows")))]
pub(super) fn set_permissions_batch(
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
