//! On-demand, read-only filesystem usage for local mounted block volumes.

use crate::errors::api_error::ApiResult;
use serde::Serialize;

#[cfg(target_os = "linux")]
use super::error::NetworkErrorCode;
use super::error::{map_api_result, NetworkError, NetworkResult};

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VolumeUsage {
    pub total_bytes: u64,
    pub used_bytes: u64,
    /// Space available to the current user, excluding filesystem reservations.
    pub free_bytes: u64,
    pub reserved_bytes: u64,
}

#[tauri::command]
pub async fn get_volume_usage(path: String) -> ApiResult<Option<VolumeUsage>> {
    let result = tauri::async_runtime::spawn_blocking(move || volume_usage(&path))
        .await
        .map_err(NetworkError::from)
        .and_then(|result| result);
    map_api_result(result)
}

#[cfg(target_os = "linux")]
fn volume_usage(path: &str) -> NetworkResult<Option<VolumeUsage>> {
    // Never auto-mount a device or query MTP/network filesystems for these statistics.
    if !super::mounts::is_local_block_mount(path)? {
        return Ok(None);
    }
    use std::os::unix::fs::OpenOptionsExt;
    let directory = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|error| {
            NetworkError::from_io_error(
                NetworkErrorCode::TaskFailed,
                "Could not inspect the mounted volume",
                error,
            )
        })?;
    filesystem_usage(&directory)
}

#[cfg(not(target_os = "linux"))]
fn volume_usage(_path: &str) -> NetworkResult<Option<VolumeUsage>> {
    Ok(None)
}

#[cfg(target_os = "linux")]
fn filesystem_usage(directory: &std::fs::File) -> NetworkResult<Option<VolumeUsage>> {
    use std::os::fd::AsRawFd;
    let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // The descriptor remains open during the query; only initialize after success.
    if unsafe { libc::fstatvfs(directory.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
        return Err(NetworkError::from_io_error(
            NetworkErrorCode::TaskFailed,
            "Could not read filesystem capacity",
            std::io::Error::last_os_error(),
        ));
    }
    let stat = unsafe { stat.assume_init() };
    usage_from_blocks(stat.f_blocks, stat.f_bfree, stat.f_bavail, stat.f_frsize)
}

#[cfg(target_os = "linux")]
fn usage_from_blocks(
    blocks: u64,
    free: u64,
    available: u64,
    block_size: u64,
) -> NetworkResult<Option<VolumeUsage>> {
    if blocks == 0 || block_size == 0 {
        return Ok(None);
    }
    let total_bytes = blocks.checked_mul(block_size).ok_or_else(|| {
        NetworkError::new(
            NetworkErrorCode::TaskFailed,
            "Filesystem capacity exceeds the supported range",
        )
    })?;
    let free = free.min(blocks);
    let available = available.min(free);
    Ok(Some(VolumeUsage {
        total_bytes,
        used_bytes: (blocks - free) * block_size,
        free_bytes: available * block_size,
        reserved_bytes: (free - available) * block_size,
    }))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn reports_used_available_and_reserved_space_separately() {
        let usage = usage_from_blocks(100, 70, 65, 4096).unwrap().unwrap();
        assert_eq!(usage.total_bytes, 409_600);
        assert_eq!(usage.used_bytes, 122_880);
        assert_eq!(usage.free_bytes, 266_240);
        assert_eq!(usage.reserved_bytes, 20_480);
        assert_eq!(
            usage.used_bytes + usage.free_bytes + usage.reserved_bytes,
            usage.total_bytes
        );
        let json = serde_json::to_value(usage).unwrap();
        assert_eq!(json["totalBytes"], 409_600);
        assert_eq!(json["freeBytes"], 266_240);
    }

    #[test]
    fn handles_empty_inconsistent_and_overflowing_filesystem_counters() {
        assert_eq!(usage_from_blocks(0, 0, 0, 4096).unwrap(), None);
        assert_eq!(usage_from_blocks(10, 0, 0, 0).unwrap(), None);
        let usage = usage_from_blocks(10, 20, 30, 1).unwrap().unwrap();
        assert_eq!(usage.free_bytes, 10);
        assert_eq!(usage.used_bytes, 0);
        assert_eq!(usage.reserved_bytes, 0);
        assert!(usage_from_blocks(u64::MAX, 0, 0, 4096).is_err());
    }

    #[test]
    fn virtual_and_unmounted_locations_are_not_probed_or_mounted() {
        for path in [
            "mtp://phone",
            "usb-volume:///dev/sdb1",
            "smb://nas/share",
            "rclone://remote/path",
            "relative",
        ] {
            assert_eq!(volume_usage(path).unwrap(), None);
        }
    }

    #[test]
    fn reads_capacity_from_an_open_directory_without_scanning_contents() {
        let directory = std::fs::File::open(std::env::temp_dir()).unwrap();
        let usage = filesystem_usage(&directory).unwrap().unwrap();
        assert!(usage.total_bytes > 0);
        assert_eq!(
            usage.used_bytes + usage.free_bytes + usage.reserved_bytes,
            usage.total_bytes
        );
    }
}
