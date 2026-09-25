//! Mount/eject handling for local and GVFS mounts.

use crate::{
    commands::fs::MountInfo, errors::api_error::ApiResult, fs_utils::debug_log, runtime_lifecycle,
    watcher::WatchState,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Instant;

#[cfg(not(target_os = "windows"))]
use std::fmt;

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
    command_output_text(cmd, args).map(|_| ())
}

#[cfg(not(target_os = "windows"))]
fn command_output_text(cmd: &str, args: &[&str]) -> Result<String, CmdError> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| CmdError {
            message: e.to_string(),
            busy: false,
        })?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
    }
    let mut parts = Vec::new();
    if !output.stdout.is_empty() {
        parts.push(String::from_utf8_lossy(&output.stdout).trim().to_string());
    }
    if !output.stderr.is_empty() {
        parts.push(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let msg = if parts.is_empty() {
        output.status.to_string()
    } else {
        format!("{}: {}", output.status, parts.join(" | "))
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
#[derive(Deserialize)]
struct LsblkOutput {
    blockdevices: Vec<LsblkDevice>,
}

#[cfg(not(target_os = "windows"))]
#[derive(Deserialize)]
struct LsblkDevice {
    path: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    fstype: Option<String>,
    #[serde(rename = "type")]
    kind: String,
    rm: bool,
    tran: Option<String>,
    pkname: Option<String>,
    #[serde(default)]
    mountpoints: Vec<Option<String>>,
}

#[cfg(not(target_os = "windows"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UsbFilesystem {
    Exfat,
    Fat32,
    Ext4,
    Btrfs,
}

#[cfg(not(target_os = "windows"))]
impl UsbFilesystem {
    fn parse(value: &str) -> NetworkResult<Self> {
        match value {
            "exfat" => Ok(Self::Exfat),
            "fat32" => Ok(Self::Fat32),
            "ext4" => Ok(Self::Ext4),
            "btrfs" => Ok(Self::Btrfs),
            _ => Err(NetworkError::new(
                NetworkErrorCode::FormatNotAllowed,
                "Unsupported USB filesystem.",
            )),
        }
    }

    fn udisks_type(self) -> &'static str {
        match self {
            Self::Exfat => "exfat",
            Self::Fat32 => "vfat",
            Self::Ext4 => "ext4",
            Self::Btrfs => "btrfs",
        }
    }

    fn id(self) -> &'static str {
        match self {
            Self::Exfat => "exfat",
            Self::Fat32 => "fat32",
            Self::Ext4 => "ext4",
            Self::Btrfs => "btrfs",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::Exfat => "Compatible with Linux, macOS, and Windows",
            Self::Fat32 => "Widest compatibility; files are limited to 4 GB",
            Self::Ext4 => "Linux filesystem",
            Self::Btrfs => "Linux filesystem with checksums and snapshots",
        }
    }

    fn mkfs_program(self) -> &'static str {
        match self {
            Self::Exfat => "mkfs.exfat",
            Self::Fat32 => "mkfs.fat",
            Self::Ext4 => "mkfs.ext4",
            Self::Btrfs => "mkfs.btrfs",
        }
    }

    fn is_available(self) -> bool {
        which::which(self.mkfs_program()).is_ok()
    }
}

#[cfg(not(target_os = "windows"))]
impl fmt::Display for UsbFilesystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Exfat => "exFAT",
            Self::Fat32 => "FAT32",
            Self::Ext4 => "ext4",
            Self::Btrfs => "Btrfs",
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsbFilesystemOption {
    id: String,
    label: String,
    description: String,
    available: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsbFormatInfo {
    device: String,
    model: String,
    size_bytes: u64,
    filesystems: Vec<UsbFilesystemOption>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsbFormatResult {
    device: String,
    mount_path: Option<String>,
    size_bytes: u64,
    filesystem: String,
    label: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct UsbFormatProgress {
    pub phase: String,
    pub percent: Option<f64>,
}

#[cfg(not(target_os = "windows"))]
impl UsbFormatProgress {
    pub(super) fn new(phase: &str, percent: Option<f64>) -> Self {
        Self {
            phase: phase.to_owned(),
            percent,
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn supported_usb_filesystems() -> Vec<UsbFilesystemOption> {
    [
        UsbFilesystem::Exfat,
        UsbFilesystem::Fat32,
        UsbFilesystem::Ext4,
        UsbFilesystem::Btrfs,
    ]
    .into_iter()
    .map(|filesystem| UsbFilesystemOption {
        id: filesystem.id().to_string(),
        label: filesystem.to_string(),
        description: filesystem.description().to_string(),
        available: filesystem.is_available(),
    })
    .collect()
}

#[cfg(not(target_os = "windows"))]
fn lsblk_listing() -> NetworkResult<LsblkOutput> {
    let output = Command::new("lsblk")
        .args([
            "--json",
            "--bytes",
            "--paths",
            "--list",
            "--output",
            "PATH,SIZE,MODEL,LABEL,FSTYPE,TYPE,RM,TRAN,PKNAME,MOUNTPOINTS",
        ])
        .output()
        .map_err(|error| NetworkError::new(NetworkErrorCode::FormatFailed, error.to_string()))?;
    if !output.status.success() {
        return Err(NetworkError::new(
            NetworkErrorCode::FormatFailed,
            "Could not inspect the selected device.",
        ));
    }
    serde_json::from_slice(&output.stdout).map_err(|error| {
        NetworkError::new(
            NetworkErrorCode::FormatFailed,
            format!("Could not read device information: {error}"),
        )
    })
}

#[cfg(not(target_os = "windows"))]
fn removable_usb_disk_for_partition(listing: &LsblkOutput, device: &str) -> Option<String> {
    let target = listing
        .blockdevices
        .iter()
        .find(|entry| entry.path == device)?;
    if target.kind != "part" {
        return None;
    }
    let parent_path = target.pkname.as_deref()?;
    let parent = listing
        .blockdevices
        .iter()
        .find(|entry| entry.path == parent_path)?;
    if parent.kind != "disk" || !parent.path.starts_with("/dev/") {
        return None;
    }
    let is_usb = |entry: &LsblkDevice| {
        entry.rm
            || entry
                .tran
                .as_deref()
                .is_some_and(|transport| transport == "usb")
    };
    if !is_usb(target) && !is_usb(parent) {
        return None;
    }
    // Never offer a system disk or a disk with active layered devices for
    // whole-drive formatting, even when the machine booted from USB.
    let partitions: Vec<_> = listing
        .blockdevices
        .iter()
        .filter(|entry| entry.pkname.as_deref() == Some(&parent.path))
        .collect();
    if partitions.iter().any(|entry| {
        entry.mountpoints.iter().flatten().any(|mount| {
            matches!(
                mount.as_str(),
                "/" | "/boot" | "/boot/efi" | "/home" | "/usr" | "/var" | "[SWAP]"
            )
        })
    }) || listing.blockdevices.iter().any(|entry| {
        partitions
            .iter()
            .any(|partition| entry.pkname.as_deref() == Some(&partition.path))
    }) {
        return None;
    }
    Some(parent.path.clone())
}

#[cfg(not(target_os = "windows"))]
fn removable_usb_disk_for_mount(path: &str) -> NetworkResult<(LsblkOutput, String)> {
    let partition = path
        .strip_prefix("usb-volume://")
        .map(str::to_owned)
        .or_else(|| block_device_for_mount(path))
        .ok_or_else(|| {
            NetworkError::new(
                NetworkErrorCode::FormatNotAllowed,
                "Selected volume is no longer mounted.",
            )
        })?;
    let listing = lsblk_listing()?;
    let disk = removable_usb_disk_for_partition(&listing, &partition).ok_or_else(|| {
        NetworkError::new(
            NetworkErrorCode::FormatNotAllowed,
            "Only removable USB partitions can be formatted.",
        )
    })?;
    Ok((listing, disk))
}

#[cfg(not(target_os = "windows"))]
fn volume_label(label: &str) -> NetworkResult<Option<String>> {
    let label = label.trim();
    if label.is_empty() {
        return Ok(None);
    }
    if label.len() > 11
        || !label.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || byte == b' ' || byte == b'_' || byte == b'-'
        })
    {
        return Err(NetworkError::new(
            NetworkErrorCode::FormatNotAllowed,
            "Volume names may contain up to 11 letters, numbers, spaces, hyphens, or underscores.",
        ));
    }
    Ok(Some(label.to_string()))
}

#[cfg(not(target_os = "windows"))]
fn unmount_mounted_partitions(listing: &LsblkOutput, disk: &str) -> NetworkResult<()> {
    for partition in listing.blockdevices.iter().filter(|entry| {
        entry.kind == "part"
            && entry.pkname.as_deref() == Some(disk)
            && entry
                .mountpoints
                .iter()
                .flatten()
                .any(|mountpoint| !mountpoint.is_empty())
    }) {
        command_output("udisksctl", &["unmount", "-b", &partition.path]).map_err(|error| {
            NetworkError::new(
                NetworkErrorCode::FormatFailed,
                format!("Could not unmount the USB volume: {}", error.message),
            )
        })?;
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn mount_new_partition(device: &str, filesystem: UsbFilesystem) -> NetworkResult<()> {
    match command_output("udisksctl", &["mount", "-b", device]) {
        Ok(()) => Ok(()),
        Err(error)
            if error
                .message
                .to_ascii_lowercase()
                .contains("already mounted") =>
        {
            Ok(())
        }
        Err(error) => Err(NetworkError::new(
            NetworkErrorCode::FormatFailed,
            format!(
                "The USB volume was formatted as {filesystem}, but could not be mounted again: {}",
                error.message
            ),
        )),
    }
}

#[cfg(not(target_os = "windows"))]
fn mount_path_for_device(device: &str) -> Option<String> {
    let output = Command::new("findmnt")
        .args(["--json", "-o", "TARGET", "--source", device])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    value["filesystems"][0]["target"]
        .as_str()
        .map(str::to_owned)
}

/// Synthetic sidebar locations: resolve against a fresh device listing before use.
#[cfg(not(target_os = "windows"))]
fn unmounted_usb_volumes(listing: &LsblkOutput) -> Vec<MountInfo> {
    listing.blockdevices.iter().filter(|entry| {
        removable_usb_disk_for_partition(listing, &entry.path).is_some()
            && !entry.mountpoints.iter().flatten().any(|path| !path.is_empty())
            // Layered/crypt volumes need an unlock workflow, not a mount request.
            && !listing.blockdevices.iter().any(|child| child.pkname.as_deref() == Some(&entry.path))
            && !matches!(entry.fstype.as_deref(), Some("swap" | "crypto_LUKS" | "LVM2_member" | "linux_raid_member"))
    }).map(|entry| MountInfo {
        label: entry.label.as_deref().filter(|label| !label.is_empty()).unwrap_or(&entry.path).to_owned(),
        path: format!("usb-volume://{}", entry.path),
        fs: entry.fstype.clone().unwrap_or_default(),
        removable: true,
    }).collect()
}

#[tauri::command]
pub async fn mount_usb_volume(path: String) -> ApiResult<String> {
    #[cfg(not(target_os = "windows"))]
    let result = tauri::async_runtime::spawn_blocking(move || {
        let device = path.strip_prefix("usb-volume://").ok_or_else(|| {
            NetworkError::new(
                NetworkErrorCode::MountFailed,
                "Invalid USB volume location.",
            )
        })?;
        let listing = lsblk_listing()?;
        if removable_usb_disk_for_partition(&listing, device).is_none() {
            return Err(NetworkError::new(
                NetworkErrorCode::MountFailed,
                "USB volume is no longer available.",
            ));
        }
        if let Some(path) = mount_path_for_device(device) {
            return Ok(path);
        }
        command_output("udisksctl", &["mount", "--block-device", device])
            .map_err(|error| NetworkError::new(NetworkErrorCode::MountFailed, error.message))?;
        mount_path_for_device(device).ok_or_else(|| {
            NetworkError::new(
                NetworkErrorCode::MountFailed,
                "Mounted, but no mount path was found.",
            )
        })
    })
    .await
    .map_err(|error| NetworkError::new(NetworkErrorCode::TaskFailed, error.to_string()))
    .and_then(|result| result);
    #[cfg(target_os = "windows")]
    let result = {
        let _ = path;
        Err(NetworkError::new(
            NetworkErrorCode::MountFailed,
            "USB mounting is only available on Linux.",
        ))
    };
    map_api_result(result)
}

#[cfg(not(target_os = "windows"))]
fn udisks_object_path(device: &str) -> NetworkResult<String> {
    let name = device.strip_prefix("/dev/").ok_or_else(|| {
        NetworkError::new(NetworkErrorCode::FormatNotAllowed, "Invalid block device.")
    })?;
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(NetworkError::new(
            NetworkErrorCode::FormatNotAllowed,
            "Unsupported block device name.",
        ));
    }
    Ok(format!("/org/freedesktop/UDisks2/block_devices/{name}"))
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
fn parse_linux_mounts(contents: &str, gvfs_root: Option<&str>) -> Vec<MountInfo> {
    let mut mounts = Vec::new();

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
            .map(|root| mount_path_is_under(&target, root))
            .unwrap_or(false);
        let is_gvfs_root = gvfs_root
            .map(|root| same_mount_path(&target, root))
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

    mounts
}

#[cfg(not(target_os = "windows"))]
fn linux_mounts() -> NetworkResult<Vec<MountInfo>> {
    let mut mounts = Vec::new();
    let gvfs_root = dirs_next::runtime_dir().map(|p| p.join("gvfs"));

    // Surface GVFS-backed MTP endpoints (e.g., Android phones).
    mounts.extend(gio_mounts::list_gvfs_mounts());

    match fs::read_to_string("/proc/self/mounts") {
        Ok(contents) => {
            mounts.extend(parse_linux_mounts(
                &contents,
                gvfs_root.as_ref().and_then(|p| p.to_str()),
            ));
        }
        Err(error) if mounts.is_empty() => {
            return Err(NetworkError::new(
                NetworkErrorCode::DiscoveryFailed,
                format!("Failed to read /proc/self/mounts: {error}"),
            ));
        }
        Err(error) => {
            debug_log(&format!(
                "mount listing skipped /proc/self/mounts after error: {}",
                error
            ));
        }
    }
    if let Ok(listing) = lsblk_listing() {
        mounts.extend(unmounted_usb_volumes(&listing));
    }
    Ok(mounts)
}

#[cfg(target_os = "windows")]
pub(super) fn list_mounts_sync() -> NetworkResult<Vec<MountInfo>> {
    fs_windows::list_windows_mounts().map_err(|error| {
        NetworkError::new(
            NetworkErrorCode::DiscoveryFailed,
            format!("Failed to list Windows mounts: {error}"),
        )
    })
}

#[cfg(target_os = "windows")]
#[tauri::command]
pub fn eject_drive(path: String, watcher: tauri::State<WatchState>) -> ApiResult<()> {
    map_api_result(eject_drive_impl(path, watcher))
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub async fn format_removable_partition(
    path: String,
    filesystem: String,
    label: String,
    on_progress: tauri::ipc::Channel<UsbFormatProgress>,
) -> ApiResult<UsbFormatResult> {
    // Linux inotify watches do not hold mounts busy. Keep the current directory
    // watch alive, especially when validation or authorization fails.
    let result = tauri::async_runtime::spawn_blocking(move || {
        format_removable_partition_impl(&path, &filesystem, &label, &|progress| {
            let _ = on_progress.send(progress);
        })
    })
    .await
    .map_err(|error| {
        NetworkError::new(
            NetworkErrorCode::TaskFailed,
            format!("format task failed: {error}"),
        )
    })
    .and_then(|result| result);
    map_api_result(result)
}

#[cfg(target_os = "windows")]
#[tauri::command]
pub async fn format_removable_partition(
    _path: String,
    _filesystem: String,
    _label: String,
    _on_progress: tauri::ipc::Channel<UsbFormatProgress>,
) -> ApiResult<UsbFormatResult> {
    map_api_result(Err(NetworkError::new(
        NetworkErrorCode::FormatNotAllowed,
        "Formatting removable volumes is not available on Windows yet.",
    )))
}

#[cfg(target_os = "windows")]
#[tauri::command]
pub async fn get_removable_usb_format_info(_path: String) -> ApiResult<UsbFormatInfo> {
    map_api_result(Err(NetworkError::new(
        NetworkErrorCode::FormatNotAllowed,
        "Formatting removable volumes is not available on Windows yet.",
    )))
}

#[cfg(not(target_os = "windows"))]
fn format_removable_partition_impl(
    path: &str,
    filesystem: &str,
    label: &str,
    report: &(dyn Fn(UsbFormatProgress) + Sync),
) -> NetworkResult<UsbFormatResult> {
    report(UsbFormatProgress::new("Checking USB drive", None));
    let filesystem = UsbFilesystem::parse(filesystem)?;
    if !filesystem.is_available() {
        return Err(NetworkError::new(
            NetworkErrorCode::FormatNotAllowed,
            format!("{} formatting support is not installed.", filesystem),
        ));
    }
    let label = volume_label(label)?;
    let _guard = super::usb_format::acquire_format_lock()?;
    let (listing, disk) = removable_usb_disk_for_mount(path)?;
    let client = super::usb_format::Client::connect(&udisks_object_path(&disk)?)?;
    client.ensure_idle()?;
    let size_bytes = listing
        .blockdevices
        .iter()
        .find(|entry| entry.path == disk)
        .map(|entry| entry.size)
        .unwrap_or_default();
    report(UsbFormatProgress::new("Unmounting USB volumes", None));
    unmount_mounted_partitions(&listing, &disk)?;
    let new_partition = client.format_disk(filesystem.udisks_type(), label.as_deref(), report)?;
    // Formatting replaces the partition table and necessarily unmounts the old filesystem.
    // Mount the newly created filesystem so it is immediately visible in Partitions.
    report(UsbFormatProgress::new("Mounting USB drive", None));
    mount_new_partition(&new_partition, filesystem)?;
    invalidate_network_discovery_cache();
    Ok(UsbFormatResult {
        device: disk,
        mount_path: mount_path_for_device(&new_partition),
        size_bytes,
        filesystem: filesystem.to_string(),
        label,
    })
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub async fn get_removable_usb_format_info(path: String) -> ApiResult<UsbFormatInfo> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _guard = super::usb_format::acquire_format_lock()?;
        let (listing, disk) = removable_usb_disk_for_mount(&path)?;
        super::usb_format::Client::connect(&udisks_object_path(&disk)?)?.ensure_idle()?;
        let device = listing
            .blockdevices
            .iter()
            .find(|entry| entry.path == disk)
            .ok_or_else(|| {
                NetworkError::new(NetworkErrorCode::FormatFailed, "USB drive disappeared.")
            })?;
        Ok(UsbFormatInfo {
            device: disk,
            model: device
                .model
                .as_deref()
                .map(str::trim)
                .filter(|model| !model.is_empty())
                .unwrap_or("USB drive")
                .to_string(),
            size_bytes: device.size,
            filesystems: supported_usb_filesystems(),
        })
    })
    .await
    .map_err(|error| {
        NetworkError::new(
            NetworkErrorCode::TaskFailed,
            format!("USB inspection task failed: {error}"),
        )
    })
    .and_then(|result: NetworkResult<UsbFormatInfo>| result);
    map_api_result(result)
}

#[cfg(target_os = "windows")]
fn eject_drive_impl(path: String, watcher: tauri::State<WatchState>) -> NetworkResult<()> {
    // Drop the active directory watcher before ejecting; open handles can block safe removal.
    watcher.replace(None).map_err(NetworkError::from)?;
    fs_windows::eject_drive(&path).map_err(NetworkError::from)
}

#[tauri::command]
pub async fn list_mounts() -> ApiResult<Vec<MountInfo>> {
    map_api_result(list_mounts_impl().await)
}

async fn list_mounts_impl() -> NetworkResult<Vec<MountInfo>> {
    let task = tauri::async_runtime::spawn_blocking(list_mounts_sync);
    match task.await {
        Ok(result) => result,
        Err(error) => Err(NetworkError::new(
            NetworkErrorCode::TaskFailed,
            format!("mount scan failed: {error}"),
        )),
    }
}

#[cfg(not(target_os = "windows"))]
pub(super) fn list_mounts_sync() -> NetworkResult<Vec<MountInfo>> {
    linux_mounts()
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub fn eject_drive(path: String, watcher: tauri::State<WatchState>) -> ApiResult<()> {
    map_api_result(eject_drive_impl(path, watcher))
}

#[cfg(not(target_os = "windows"))]
fn eject_drive_impl(path: String, watcher: tauri::State<WatchState>) -> NetworkResult<()> {
    // Drop watcher to avoid open handles during unmount
    watcher.replace(None).map_err(NetworkError::from)?;

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
    Err(NetworkError::new(NetworkErrorCode::EjectFailed, msg))
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub async fn mount_partition(path: String, app: tauri::AppHandle) -> ApiResult<()> {
    map_api_result(mount_partition_impl(path, app).await)
}

#[cfg(not(target_os = "windows"))]
pub(super) async fn mount_partition_impl(path: String, app: tauri::AppHandle) -> NetworkResult<()> {
    #[cfg(target_os = "linux")]
    if path.starts_with("mtp://") {
        return mount_mtp_uri_impl(path, app).await.map(|_| ());
    }
    let lower = path.to_ascii_lowercase();
    let scheme = lower
        .split_once("://")
        .map(|(prefix, _)| prefix.to_string());
    let fs_kind = scheme.unwrap_or_else(|| "gvfs".to_string());
    runtime_lifecycle::emit_if_running(
        &app,
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
        .map_err(|error| {
            NetworkError::new(
                NetworkErrorCode::TaskFailed,
                format!("mount task failed: {error}"),
            )
        })?;
        let ok = !matches!(status, gio_mounts::MountUriStatus::Failed);
        let outcome = status.as_str();
        let duration_ms = started.elapsed().as_millis() as u64;
        runtime_lifecycle::emit_if_running(
            &app,
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
            Err(NetworkError::new(
                NetworkErrorCode::MountFailed,
                format!("Failed to mount {fs_kind}"),
            ))
        }
    } else {
        let duration_ms = started.elapsed().as_millis() as u64;
        runtime_lifecycle::emit_if_running(
            &app,
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

#[cfg(target_os = "linux")]
pub(super) async fn mount_mtp_uri_impl(
    path: String,
    app: tauri::AppHandle,
) -> NetworkResult<String> {
    runtime_lifecycle::emit_if_running(
        &app,
        "mounting-started",
        json!({ "path": &path, "fs": "mtp", "outcome": "connecting" }),
    );
    let result = crate::mtp::mount(path.clone(), app.clone()).await;
    runtime_lifecycle::emit_if_running(
        &app,
        "mounting-done",
        json!({ "path": &path, "fs": "mtp", "ok": result.is_ok(), "outcome": if result.is_ok() { "connected" } else { "failed" } }),
    );
    result.map_err(|error| NetworkError::new(NetworkErrorCode::MountFailed, error))
}

#[cfg(target_os = "windows")]
#[tauri::command]
pub async fn mount_partition(_path: String) -> ApiResult<()> {
    map_api_result(Ok(()))
}

#[cfg(all(test, not(target_os = "windows")))]
mod tests {
    use super::*;

    fn lsblk_device(
        path: &str,
        kind: &str,
        removable: bool,
        transport: Option<&str>,
        parent: Option<&str>,
    ) -> LsblkDevice {
        LsblkDevice {
            path: path.into(),
            size: 0,
            model: None,
            label: None,
            fstype: None,
            kind: kind.into(),
            rm: removable,
            tran: transport.map(str::to_owned),
            pkname: parent.map(str::to_owned),
            mountpoints: Vec::new(),
        }
    }

    #[test]
    fn accepts_only_supported_usb_filesystems() {
        assert_eq!(UsbFilesystem::parse("exfat").unwrap(), UsbFilesystem::Exfat);
        assert_eq!(UsbFilesystem::parse("fat32").unwrap(), UsbFilesystem::Fat32);
        assert_eq!(UsbFilesystem::parse("ext4").unwrap(), UsbFilesystem::Ext4);
        assert_eq!(UsbFilesystem::parse("btrfs").unwrap(), UsbFilesystem::Btrfs);
        assert!(UsbFilesystem::parse("ntfs").is_err());
    }

    #[test]
    fn validates_cross_platform_volume_labels() {
        assert_eq!(volume_label(" SHARE_01 ").unwrap(), Some("SHARE_01".into()));
        assert_eq!(volume_label(" ").unwrap(), None);
        assert!(volume_label("label-with-too-many-characters").is_err());
        assert!(volume_label("not/allowed").is_err());
    }

    #[test]
    fn lists_unmounted_usb_partitions_without_mounted_or_internal_duplicates() {
        let listing: LsblkOutput = serde_json::from_str(r#"{"blockdevices":[
            {"path":"/dev/sdz","type":"disk","rm":true,"tran":"usb","pkname":null},
            {"path":"/dev/sdz1","type":"part","rm":true,"tran":null,"pkname":"/dev/sdz","label":"MY USB","fstype":"exfat","mountpoints":[null]},
            {"path":"/dev/sdz2","type":"part","rm":true,"tran":null,"pkname":"/dev/sdz","mountpoints":["/run/media/user/SECOND"]},
            {"path":"/dev/nvme0n1","type":"disk","rm":false,"tran":"nvme","pkname":null},
            {"path":"/dev/nvme0n1p1","type":"part","rm":false,"tran":null,"pkname":"/dev/nvme0n1"}
        ]}"#).unwrap();
        let volumes = unmounted_usb_volumes(&listing);
        assert_eq!(volumes.len(), 1);
        assert_eq!(volumes[0].path, "usb-volume:///dev/sdz1");
        assert_eq!(volumes[0].label, "MY USB");
        assert_eq!(volumes[0].fs, "exfat");
    }

    #[test]
    fn rejects_usb_system_disks_and_active_layered_devices() {
        let mut root = lsblk_device("/dev/sdz1", "part", true, None, Some("/dev/sdz"));
        root.mountpoints = vec![Some("/".into())];
        let mut listing = LsblkOutput {
            blockdevices: vec![
                lsblk_device("/dev/sdz", "disk", true, Some("usb"), None),
                root,
                lsblk_device("/dev/sdz2", "part", true, None, Some("/dev/sdz")),
            ],
        };
        assert!(removable_usb_disk_for_partition(&listing, "/dev/sdz2").is_none());
        assert!(unmounted_usb_volumes(&listing).is_empty());
        listing.blockdevices[1].mountpoints.clear();
        listing.blockdevices.push(lsblk_device(
            "/dev/mapper/secret",
            "crypt",
            false,
            None,
            Some("/dev/sdz1"),
        ));
        assert!(removable_usb_disk_for_partition(&listing, "/dev/sdz2").is_none());
    }

    #[test]
    fn invalid_format_requests_fail_before_resolving_or_touching_devices() {
        assert!(format_removable_partition_impl(
            "/definitely/not/a/device",
            "invalid",
            "OK",
            &|_| {}
        )
        .is_err());
        assert!(volume_label("bad/name").is_err());
    }

    #[test]
    fn identifies_only_removable_usb_parent_disks() {
        let listing = LsblkOutput {
            blockdevices: vec![
                lsblk_device("/dev/sda", "disk", true, Some("usb"), None),
                lsblk_device("/dev/sda1", "part", true, None, Some("/dev/sda")),
                lsblk_device("/dev/nvme0n1", "disk", false, Some("nvme"), None),
                lsblk_device("/dev/nvme0n1p1", "part", false, None, Some("/dev/nvme0n1")),
            ],
        };

        assert_eq!(
            removable_usb_disk_for_partition(&listing, "/dev/sda1"),
            Some("/dev/sda".into())
        );
        assert_eq!(
            removable_usb_disk_for_partition(&listing, "/dev/nvme0n1p1"),
            None
        );
        assert_eq!(removable_usb_disk_for_partition(&listing, "/dev/sda"), None);
    }

    #[test]
    fn parse_linux_mounts_filters_pseudo_mounts_and_generic_gvfs_root() {
        let mounts = parse_linux_mounts(
            "\
proc /proc proc rw 0 0\n\
tmpfs /run/user/1000 tmpfs rw 0 0\n\
gvfsd-fuse /run/user/1000/gvfs fuse.gvfsd-fuse rw 0 0\n\
/dev/sda2 / ext4 rw 0 0\n\
gvfsd-fuse /run/user/1000/gvfs/smb-share:server=nas.local,share=docs fuse.gvfsd-fuse rw 0 0\n",
            Some("/run/user/1000/gvfs"),
        );

        assert_eq!(
            mounts.len(),
            2,
            "expected only root and concrete gvfs mount"
        );
        assert!(mounts.iter().any(|mount| mount.path == "/"));
        assert!(mounts.iter().any(|mount| {
            mount.path == "/run/user/1000/gvfs/smb-share:server=nas.local,share=docs"
        }));
        assert!(
            mounts
                .iter()
                .all(|mount| mount.path != "/run/user/1000/gvfs"),
            "generic gvfs root should stay hidden"
        );
    }

    #[test]
    fn parse_linux_mounts_marks_removable_user_and_windows_style_mounts() {
        let mounts = parse_linux_mounts(
            "\
/dev/sdb1 /run/media/chris/USB_DISK vfat rw 0 0\n\
/dev/nvme0n1p2 /home ext4 rw 0 0\n\
server:/export /mnt/nfs nfs4 rw 0 0\n",
            Some("/run/user/1000/gvfs"),
        );

        let usb_mount = mounts
            .iter()
            .find(|mount| mount.path == "/run/media/chris/USB_DISK")
            .expect("usb mount should be kept");
        assert_eq!(usb_mount.label, "USB_DISK");
        assert!(
            usb_mount.removable,
            "user-visible removable media should be marked removable"
        );

        let home_mount = mounts
            .iter()
            .find(|mount| mount.path == "/home")
            .expect("home mount should be kept");
        assert!(
            !home_mount.removable,
            "plain ext4 root mounts should not be marked removable"
        );

        let nfs_mount = mounts
            .iter()
            .find(|mount| mount.path == "/mnt/nfs")
            .expect("nfs mount should be kept");
        assert_eq!(nfs_mount.fs, "nfs4");
        assert!(
            !nfs_mount.removable,
            "network mounts should not inherit removable heuristics"
        );
    }
}
