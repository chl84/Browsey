//! Mount/eject handling for local and GVFS mounts.

use crate::{
    commands::fs::MountInfo, errors::api_error::ApiResult, fs_utils::debug_log, watcher::WatchState,
};
use serde_json::json;
use std::time::Instant;
use tauri::Emitter;

#[cfg(not(target_os = "windows"))]
use {
    super::{discovery, gio_mounts},
    dirs_next,
    std::fs,
    std::process::{Command, Stdio},
};

use super::error::{map_api_result, NetworkError, NetworkErrorCode, NetworkResult};

#[cfg(target_os = "windows")]
use crate::commands::fs::fs_windows;

#[cfg(not(target_os = "windows"))]
struct CmdError {
    message: String,
    busy: bool,
}

#[cfg(not(target_os = "windows"))]
fn command_output(cmd: &str, args: &[&str]) -> Result<(), CmdError> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| CmdError {
            message: e.to_string(),
            busy: false,
        })?;
    if output.status.success() {
        return Ok(());
    }
    let mut parts = Vec::new();
    if !output.stdout.is_empty() {
        parts.push(String::from_utf8_lossy(&output.stdout).trim().to_string());
    }
    if !output.stderr.is_empty() {
        parts.push(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let msg = if parts.is_empty() {
        format!("exit status {}", output.status)
    } else {
        format!("exit status {}: {}", output.status, parts.join(" | "))
    };
    let busy = msg.to_lowercase().contains("busy");
    Err(CmdError { message: msg, busy })
}

#[cfg(not(target_os = "windows"))]
fn block_device_for_mount(target: &str) -> Option<String> {
    if let Ok(output) = Command::new("findmnt")
        .args(["-n", "-o", "SOURCE", "--target", target])
        .output()
    {
        if output.status.success() {
            let src = String::from_utf8_lossy(&output.stdout);
            if let Some(first) = src.split_whitespace().next() {
                if !first.trim().is_empty() {
                    return Some(first.trim().to_string());
                }
            }
        }
    }
    None
}

#[cfg(not(target_os = "windows"))]
fn power_off_device(device: Option<String>) {
    // Only attempt power-off for real block devices; skip pseudo entries like gvfsd-fuse.
    if let Some(dev) = device {
        if !dev.starts_with("/dev/") {
            return;
        }
        if let Ok(status) = Command::new("udisksctl")
            .args(["power-off", "-b", &dev])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
        {
            if !status.success() {
                debug_log(&format!(
                    "udisksctl power-off failed for {}: status {:?}",
                    dev,
                    status.code()
                ));
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn same_mount_path(a: &str, b: &str) -> bool {
    a.trim_end_matches('/') == b.trim_end_matches('/')
}

#[cfg(not(target_os = "windows"))]
fn mount_path_is_under(path: &str, root: &str) -> bool {
    let path = path.trim_end_matches('/');
    let root = root.trim_end_matches('/');
    path == root
        || path
            .strip_prefix(root)
            .map(|rest| rest.starts_with('/'))
            .unwrap_or(false)
}

#[cfg(not(target_os = "windows"))]
fn invalidate_network_discovery_cache() {
    discovery::invalidate_network_devices_cache();
}

#[cfg(not(target_os = "windows"))]
fn linux_mounts() -> Vec<MountInfo> {
    let mut mounts = Vec::new();
    let gvfs_root = dirs_next::runtime_dir().map(|p| p.join("gvfs"));

    // Surface GVFS-backed MTP endpoints (e.g., Android phones).
    mounts.extend(gio_mounts::list_gvfs_mounts());

    if let Ok(contents) = fs::read_to_string("/proc/self/mounts") {
        for line in contents.lines() {
            let mut parts = line.split_whitespace();
            let src = match parts.next() {
                Some(s) => s.replace("\\040", " "),
                None => continue,
            };
            let target = match parts.next() {
                Some(t) => t.replace("\\040", " "),
                None => continue,
            };
            let fs = match parts.next() {
                Some(f) => f.to_string(),
                None => continue,
            };
            let fs_lc = fs.to_lowercase();

            // Skip pseudo/system mounts
            if matches!(
                fs_lc.as_str(),
                "proc"
                    | "sysfs"
                    | "devtmpfs"
                    | "devpts"
                    | "tmpfs"
                    | "pstore"
                    | "configfs"
                    | "debugfs"
                    | "tracefs"
                    | "overlay"
                    | "squashfs"
                    | "hugetlbfs"
                    | "mqueue"
                    | "cgroup"
                    | "cgroup2"
                    | "fuse.rofiles-fuse" // Flatpak Builder readonly rofiles mounts
            ) {
                continue;
            }
            let in_gvfs = gvfs_root
                .as_ref()
                .and_then(|p| p.to_str())
                .map(|p| mount_path_is_under(&target, p))
                .unwrap_or(false);
            let is_gvfs_root = gvfs_root
                .as_ref()
                .and_then(|p| p.to_str())
                .map(|p| same_mount_path(&target, p))
                .unwrap_or(false);

            // Keep GVFS endpoints (for example MTP), but hide the generic gvfs root mount from Partitions.
            if is_gvfs_root {
                continue;
            }

            if target.starts_with("/proc")
                || target.starts_with("/sys")
                || target.starts_with("/run/lock")
                || (target.starts_with("/run/user") && !in_gvfs)
            {
                continue;
            }

            let label = std::path::Path::new(&target)
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| target.clone());

            let is_user_mount = target.contains("/media/") || target.contains("/run/media/");
            let is_windows_fs = matches!(
                fs_lc.as_str(),
                "vfat" | "exfat" | "ntfs" | "fuseblk" | "fuse.exfat" | "fuse.ntfs-3g" | "fuse.ntfs"
            );
            let is_boot = target.starts_with("/boot");
            let removable_hint = (is_user_mount || is_windows_fs) && !is_boot;
            // device heuristic: only classic removable prefixes
            let dev_removable = src.starts_with("/dev/sd")
                || src.starts_with("/dev/mmc")
                || src.starts_with("/dev/sg")
                || src.contains("usb");

            mounts.push(MountInfo {
                label,
                path: target,
                fs,
                removable: removable_hint || dev_removable,
            });
        }
    }
    mounts
}

#[cfg(target_os = "windows")]
pub(super) fn list_mounts_sync() -> Vec<MountInfo> {
    fs_windows::list_windows_mounts()
}

#[cfg(target_os = "windows")]
#[tauri::command]
pub fn eject_drive(path: String, watcher: tauri::State<WatchState>) -> ApiResult<()> {
    map_api_result(eject_drive_impl(path, watcher))
}

#[cfg(target_os = "windows")]
fn eject_drive_impl(path: String, watcher: tauri::State<WatchState>) -> NetworkResult<()> {
    // Drop the active directory watcher before ejecting; open handles can block safe removal.
    watcher.replace(None);
    fs_windows::eject_drive(&path)
        .map_err(|error| NetworkError::new(NetworkErrorCode::EjectFailed, error))
}

#[tauri::command]
pub async fn list_mounts() -> ApiResult<Vec<MountInfo>> {
    map_api_result(list_mounts_impl().await)
}

async fn list_mounts_impl() -> NetworkResult<Vec<MountInfo>> {
    let task = tauri::async_runtime::spawn_blocking(list_mounts_sync);
    match task.await {
        Ok(result) => Ok(result),
        Err(error) => Err(NetworkError::new(
            NetworkErrorCode::TaskFailed,
            format!("mount scan failed: {error}"),
        )),
    }
}

#[cfg(not(target_os = "windows"))]
pub(super) fn list_mounts_sync() -> Vec<MountInfo> {
    linux_mounts()
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub fn eject_drive(path: String, watcher: tauri::State<WatchState>) -> ApiResult<()> {
    map_api_result(eject_drive_impl(path, watcher).map_err(NetworkError::from_external_message))
}

#[cfg(not(target_os = "windows"))]
fn eject_drive_impl(path: String, watcher: tauri::State<WatchState>) -> Result<(), String> {
    // Drop watcher to avoid open handles during unmount
    watcher.replace(None);

    let path = path;

    gio_mounts::ensure_gvfsd_fuse_running();

    let device = block_device_for_mount(&path);

    let mut errors: Vec<String> = Vec::new();
    let mut busy_detected = false;

    // Prefer gio (GVFS) if available; it handles user mounts.
    match command_output("gio", &["mount", "-u", &path]) {
        Ok(_) => {
            invalidate_network_discovery_cache();
            power_off_device(device);
            return Ok(());
        }
        Err(e) => {
            // Ignore noisy gvfsd-fuse lookup errors on unmount.
            if e.message.contains("gvfsd-fuse") {
                invalidate_network_discovery_cache();
                power_off_device(device);
                return Ok(());
            }
            busy_detected |= e.busy;
            errors.push(format!("gio mount -u: {}", e.message));
        }
    }

    // Fallback: plain umount.
    match command_output("umount", &[&path]) {
        Ok(_) => {
            invalidate_network_discovery_cache();
            power_off_device(device);
            return Ok(());
        }
        Err(e) => {
            busy_detected |= e.busy;
            errors.push(format!("umount: {}", e.message));
        }
    }

    // Last resort: udisksctl unmount by block device, if we have one.
    if let Some(dev) = device.clone() {
        match command_output("udisksctl", &["unmount", "-b", &dev]) {
            Ok(_) => {
                invalidate_network_discovery_cache();
                power_off_device(Some(dev));
                return Ok(());
            }
            Err(e) => {
                busy_detected |= e.busy;
                errors.push(format!("udisksctl unmount: {}", e.message));
            }
        }
    } else {
        errors.push("no block device found for this mount".into());
    }

    // Optional lazy unmount if we only saw busy errors
    if busy_detected && command_output("umount", &["-l", &path]).is_ok() {
        invalidate_network_discovery_cache();
        power_off_device(device);
        return Ok(());
    }

    let msg = if busy_detected {
        "Volume is in use. Close file managers or terminals using it and try again.".to_string()
    } else if let Some(first) = errors.first() {
        first.clone()
    } else {
        "Eject failed.".to_string()
    };
    debug_log(&format!(
        "eject errors for {}: {}",
        path,
        errors.join(" | ")
    ));
    Err(msg)
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub async fn mount_partition(path: String, app: tauri::AppHandle) -> ApiResult<()> {
    map_api_result(
        mount_partition_impl(path, app)
            .await
            .map_err(NetworkError::from_external_message),
    )
}

#[cfg(not(target_os = "windows"))]
async fn mount_partition_impl(path: String, app: tauri::AppHandle) -> Result<(), String> {
    let lower = path.to_ascii_lowercase();
    let scheme = lower
        .split_once("://")
        .map(|(prefix, _)| prefix.to_string());
    let fs_kind = scheme.unwrap_or_else(|| "gvfs".to_string());
    let _ = app.emit(
        "mounting-started",
        json!({ "path": &path, "fs": &fs_kind, "outcome": "connecting" }),
    );
    let started = Instant::now();

    if lower.contains("://") {
        let path_for_mount = path.clone();
        let status = tauri::async_runtime::spawn_blocking(move || {
            gio_mounts::mount_uri_status(&path_for_mount)
        })
        .await
        .unwrap_or(gio_mounts::MountUriStatus::Failed);
        let ok = !matches!(status, gio_mounts::MountUriStatus::Failed);
        let outcome = status.as_str();
        let duration_ms = started.elapsed().as_millis() as u64;
        let _ = app.emit(
            "mounting-done",
            json!({
                "path": &path,
                "fs": &fs_kind,
                "ok": ok,
                "outcome": outcome,
                "duration_ms": duration_ms
            }),
        );
        if ok {
            invalidate_network_discovery_cache();
            Ok(())
        } else {
            Err(format!("Failed to mount {fs_kind}"))
        }
    } else {
        let duration_ms = started.elapsed().as_millis() as u64;
        let _ = app.emit(
            "mounting-done",
            json!({
                "path": &path,
                "fs": &fs_kind,
                "ok": true,
                "outcome": "connected",
                "duration_ms": duration_ms
            }),
        );
        Ok(())
    }
}

#[cfg(target_os = "windows")]
#[tauri::command]
pub async fn mount_partition(_path: String) -> ApiResult<()> {
    map_api_result(Ok(()))
}
