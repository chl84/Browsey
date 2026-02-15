use std::fs;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(unix)]
use std::process::{Command, Stdio};
use tracing::{debug, warn};

use crate::{
    fs_utils::{check_no_symlink_components, sanitize_path_nofollow},
    undo::{apply_ownership, ownership_snapshot, set_ownership_nofollow},
};

use super::{
    ensure_absolute_path, permission_info_fallback, refresh_permissions_after_apply,
    PermissionInfo, OWNERSHIP_HELPER_FLAG,
};

#[derive(Clone)]
struct OwnershipRollback {
    path: std::path::PathBuf,
    before: crate::undo::OwnershipSnapshot,
}

fn rollback_ownership_actions(actions: &[OwnershipRollback]) -> Result<(), String> {
    if actions.is_empty() {
        return Ok(());
    }
    let mut errors: Vec<String> = Vec::new();
    for action in actions.iter().rev() {
        if let Err(err) = apply_ownership(&action.path, &action.before) {
            errors.push(format!("{}: {err}", action.path.display()));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        let joined = errors.join("; ");
        warn!(error = %joined, "ownership rollback failed");
        Err(joined)
    }
}

#[cfg(unix)]
#[derive(serde::Serialize, serde::Deserialize)]
struct OwnershipHelperRequest {
    paths: Vec<String>,
    owner: Option<String>,
    group: Option<String>,
}

fn normalize_principal_spec(raw: Option<String>) -> Option<String> {
    raw.and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

#[cfg(unix)]
fn lookup_user_id(name: &str) -> Option<u32> {
    use std::ffi::CString;
    use std::mem::MaybeUninit;
    use std::ptr;

    let c_name = CString::new(name).ok()?;
    let mut buf_len = 1024usize;
    for _ in 0..4 {
        let mut pwd = MaybeUninit::<libc::passwd>::zeroed();
        let mut result: *mut libc::passwd = ptr::null_mut();
        let mut buf = vec![0u8; buf_len];
        let rc = unsafe {
            libc::getpwnam_r(
                c_name.as_ptr(),
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
            return Some(pwd.pw_uid);
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
fn lookup_group_id(name: &str) -> Option<u32> {
    use std::ffi::CString;
    use std::mem::MaybeUninit;
    use std::ptr;

    let c_name = CString::new(name).ok()?;
    let mut buf_len = 1024usize;
    for _ in 0..4 {
        let mut grp = MaybeUninit::<libc::group>::zeroed();
        let mut result: *mut libc::group = ptr::null_mut();
        let mut buf = vec![0u8; buf_len];
        let rc = unsafe {
            libc::getgrnam_r(
                c_name.as_ptr(),
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
            return Some(grp.gr_gid);
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
fn resolve_uid_spec(spec: &str) -> Result<u32, String> {
    if let Ok(uid) = spec.parse::<u32>() {
        return Ok(uid);
    }
    lookup_user_id(spec).ok_or_else(|| format!("User not found: {spec}"))
}

#[cfg(unix)]
fn resolve_gid_spec(spec: &str) -> Result<u32, String> {
    if let Ok(gid) = spec.parse::<u32>() {
        return Ok(gid);
    }
    lookup_group_id(spec).ok_or_else(|| format!("Group not found: {spec}"))
}

#[cfg(unix)]
fn is_elevated_privileges_error(msg: &str) -> bool {
    msg.contains("requires elevated privileges")
}

#[cfg(unix)]
fn run_ownership_with_pkexec(
    paths: Vec<String>,
    owner: Option<String>,
    group: Option<String>,
) -> Result<(), String> {
    let exe =
        std::env::current_exe().map_err(|e| format!("Failed to locate Browsey executable: {e}"))?;
    let request = OwnershipHelperRequest {
        paths,
        owner,
        group,
    };
    let payload = serde_json::to_vec(&request)
        .map_err(|e| format!("Failed to serialize helper request: {e}"))?;

    let mut child = Command::new("pkexec")
        .arg(&exe)
        .arg(OWNERSHIP_HELPER_FLAG)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "pkexec is not installed; cannot request elevated ownership change".to_string()
            } else {
                format!("Failed to start pkexec: {e}")
            }
        })?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        if let Err(e) = stdin.write_all(&payload) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("Failed to send helper request: {e}"));
        }
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("Failed waiting for pkexec helper: {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !stderr.is_empty() {
        return Err(stderr);
    }
    if !stdout.is_empty() {
        return Err(stdout);
    }
    Err("Authentication was cancelled or denied".into())
}

#[cfg(unix)]
#[derive(Clone)]
struct OwnershipTarget {
    target: std::path::PathBuf,
    before: crate::undo::OwnershipSnapshot,
    uid_update: Option<u32>,
    gid_update: Option<u32>,
}

#[cfg(unix)]
fn set_ownership_batch_impl(
    paths: Vec<String>,
    owner: Option<String>,
    group: Option<String>,
    allow_pkexec_retry: bool,
) -> Result<PermissionInfo, String> {
    let owner_spec = normalize_principal_spec(owner);
    let group_spec = normalize_principal_spec(group);
    if owner_spec.is_none() && group_spec.is_none() {
        return Err("No ownership changes were provided".into());
    }
    let desired_uid = owner_spec.as_deref().map(resolve_uid_spec).transpose()?;
    let desired_gid = group_spec.as_deref().map(resolve_gid_spec).transpose()?;

    let first_path = paths.first().cloned();
    let mut targets: Vec<OwnershipTarget> = Vec::with_capacity(paths.len());
    for path in paths {
        ensure_absolute_path(&path)?;
        debug!(
            path = %path,
            owner = ?owner_spec,
            group = ?group_spec,
            "set_ownership start"
        );
        let target = sanitize_path_nofollow(&path, true)?;
        check_no_symlink_components(&target)?;
        let meta =
            fs::symlink_metadata(&target).map_err(|e| format!("Failed to read metadata: {e}"))?;
        if meta.file_type().is_symlink() {
            return Err("Ownership changes are not supported on symlinks".into());
        }
        let current_uid = meta.uid();
        let current_gid = meta.gid();
        let uid_update = desired_uid.filter(|uid| *uid != current_uid);
        let gid_update = desired_gid.filter(|gid| *gid != current_gid);
        let before = ownership_snapshot(&target)?;
        targets.push(OwnershipTarget {
            target,
            before,
            uid_update,
            gid_update,
        });
    }

    let mut rollbacks: Vec<OwnershipRollback> = Vec::with_capacity(targets.len());
    let mut escalated = false;

    for target in &targets {
        if target.uid_update.is_none() && target.gid_update.is_none() {
            continue;
        }
        let apply_result: Result<(), String> = (|| {
            if let Err(e) =
                set_ownership_nofollow(&target.target, target.uid_update, target.gid_update)
            {
                if allow_pkexec_retry && is_elevated_privileges_error(&e) {
                    let helper_paths: Vec<String> = targets
                        .iter()
                        .map(|t| t.target.to_string_lossy().into_owned())
                        .collect();
                    run_ownership_with_pkexec(
                        helper_paths,
                        owner_spec.clone(),
                        group_spec.clone(),
                    )?;
                    escalated = true;
                    return Ok(());
                }
                return Err(e);
            }

            let after = match ownership_snapshot(&target.target) {
                Ok(after) => after,
                Err(snapshot_err) => {
                    match apply_ownership(&target.target, &target.before) {
                        Ok(()) => {
                            return Err(format!(
                                "Failed to capture post-change ownership for {}: {}; current target rolled back",
                                target.target.display(),
                                snapshot_err
                            ));
                        }
                        Err(rollback_err) => {
                            return Err(format!(
                                "Failed to capture post-change ownership for {}: {}; rollback failed ({rollback_err}). System may be partially changed",
                                target.target.display(),
                                snapshot_err
                            ));
                        }
                    }
                }
            };
            if after.uid != target.before.uid || after.gid != target.before.gid {
                rollbacks.push(OwnershipRollback {
                    path: target.target.clone(),
                    before: target.before.clone(),
                });
            }
            Ok(())
        })();

        if let Err(err) = apply_result {
            if let Err(rollback_err) = rollback_ownership_actions(&rollbacks) {
                return Err(format!(
                    "{err}; rollback failed ({rollback_err}). System may be partially changed"
                ));
            }
            return Err(err);
        }
        if escalated {
            break;
        }
    }

    let changed_any = if escalated {
        let mut changed = false;
        for target in &targets {
            if target.uid_update.is_none() && target.gid_update.is_none() {
                continue;
            }
            let after = ownership_snapshot(&target.target)?;
            if after.uid != target.before.uid || after.gid != target.before.gid {
                changed = true;
            }
        }
        changed
    } else {
        !rollbacks.is_empty()
    };

    if let Some(path) = first_path {
        return refresh_permissions_after_apply(path, changed_any);
    }

    Ok(permission_info_fallback())
}

#[cfg(unix)]
pub(super) fn set_ownership_batch(
    paths: Vec<String>,
    owner: Option<String>,
    group: Option<String>,
) -> Result<PermissionInfo, String> {
    set_ownership_batch_impl(paths, owner, group, true)
}

#[cfg(not(unix))]
pub(super) fn set_ownership_batch(
    paths: Vec<String>,
    owner: Option<String>,
    group: Option<String>,
) -> Result<PermissionInfo, String> {
    let _ = (paths, owner, group);
    Err("Ownership changes are not supported on this platform".into())
}

#[cfg(unix)]
fn run_ownership_helper_from_stdin() -> Result<(), String> {
    use std::io::Read;

    let mut input = Vec::new();
    std::io::stdin()
        .read_to_end(&mut input)
        .map_err(|e| format!("Failed reading helper input: {e}"))?;
    let request: OwnershipHelperRequest =
        serde_json::from_slice(&input).map_err(|e| format!("Invalid helper input: {e}"))?;
    set_ownership_batch_impl(request.paths, request.owner, request.group, false).map(|_| ())
}

pub fn maybe_run_ownership_helper_from_args() -> Option<i32> {
    let mut args = std::env::args();
    let _ = args.next();
    if args.next().as_deref() != Some(OWNERSHIP_HELPER_FLAG) {
        return None;
    }

    #[cfg(unix)]
    {
        return match run_ownership_helper_from_stdin() {
            Ok(()) => Some(0),
            Err(err) => {
                eprintln!("{err}");
                Some(1)
            }
        };
    }

    #[cfg(not(unix))]
    {
        eprintln!("Ownership helper mode is only supported on Unix");
        Some(1)
    }
}
