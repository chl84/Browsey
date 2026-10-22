use crate::commands::decompress::error::{DecompressError, DecompressErrorCode, DecompressResult};
use crate::errors::domain::{classify_io_error, IoErrorHint};
use std::{
    io,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
#[cfg(not(unix))]
use sysinfo::Disks;

#[derive(Clone)]
pub(crate) struct DiskSpaceGuard {
    path: PathBuf,
    min_free_bytes: u64,
    check_interval_bytes: u64,
    bytes_since_check: Arc<AtomicU64>,
}

impl DiskSpaceGuard {
    pub(crate) fn new(path: PathBuf, min_free_bytes: u64, check_interval_bytes: u64) -> Self {
        let interval = check_interval_bytes.max(1);
        Self {
            path,
            min_free_bytes,
            check_interval_bytes: interval,
            // Force an early runtime check on first write.
            bytes_since_check: Arc::new(AtomicU64::new(interval)),
        }
    }

    fn maybe_check(&self, delta: u64) -> io::Result<()> {
        let accumulated = self
            .bytes_since_check
            .fetch_add(delta, Ordering::Relaxed)
            .saturating_add(delta);
        if accumulated < self.check_interval_bytes {
            return Ok(());
        }
        self.bytes_since_check.store(0, Ordering::Relaxed);
        let available = available_disk_bytes(&self.path).map_err(io::Error::other)?;
        let projected = available.saturating_sub(delta);
        if projected < self.min_free_bytes {
            return Err(io::Error::other(format!(
                "Insufficient free disk space for extraction on {} (projected free: {} bytes, required reserve: {} bytes)",
                self.path.display(),
                projected,
                self.min_free_bytes
            )));
        }
        Ok(())
    }
}

pub(crate) fn effective_extract_bytes_cap(
    hard_cap: u64,
    available_bytes: u64,
    reserve_bytes: u64,
) -> u64 {
    hard_cap.min(available_bytes.saturating_sub(reserve_bytes))
}

pub(crate) fn available_disk_bytes(path: &Path) -> DecompressResult<u64> {
    let mut probe = path.to_path_buf();
    while !probe
        .try_exists()
        .map_err(|error| disk_space_error(&probe, error))?
    {
        let Some(parent) = probe.parent() else {
            break;
        };
        probe = parent.to_path_buf();
    }

    available_bytes_at(&probe)
}

fn disk_space_error(path: &Path, error: io::Error) -> DecompressError {
    let code = match classify_io_error(&error) {
        IoErrorHint::NotFound => DecompressErrorCode::NotFound,
        IoErrorHint::PermissionDenied => DecompressErrorCode::PermissionDenied,
        IoErrorHint::InvalidInput => DecompressErrorCode::InvalidPath,
        IoErrorHint::ReadOnlyFilesystem => DecompressErrorCode::ReadOnlyFilesystem,
        _ => DecompressErrorCode::ExtractFailed,
    };
    DecompressError::new(
        code,
        format!(
            "Failed to read available disk space for {}: {error}",
            path.display()
        ),
    )
}

#[cfg(unix)]
fn available_bytes_at(path: &Path) -> DecompressResult<u64> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let encoded = CString::new(path.as_os_str().as_bytes()).map_err(|_| {
        DecompressError::new(DecompressErrorCode::InvalidPath, "Path contains NUL byte")
    })?;
    let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // Query this filesystem directly: inventory APIs can omit tmpfs, FUSE and
    // bind mounts, or otherwise report a parent filesystem's capacity instead.
    // The path is NUL-terminated and the output storage remains valid here.
    if unsafe { libc::statvfs(encoded.as_ptr(), stat.as_mut_ptr()) } != 0 {
        return Err(disk_space_error(path, io::Error::last_os_error()));
    }
    // The successful syscall initialized every field before it is read.
    let stat = unsafe { stat.assume_init() };
    if stat.f_frsize == 0 {
        return Err(DecompressError::new(
            DecompressErrorCode::ExtractFailed,
            "Filesystem block size is unavailable",
        ));
    }
    // f_bavail excludes space reserved for privileged users. Compute in u128
    // before checking the u64 budget range, including on 32-bit Unix targets.
    let bytes = (stat.f_bavail as u128) * (stat.f_frsize as u128);
    u64::try_from(bytes).map_err(|_| {
        DecompressError::new(
            DecompressErrorCode::ExtractFailed,
            "Filesystem capacity exceeds the supported range",
        )
    })
}

#[cfg(not(unix))]
fn available_bytes_at(probe: &Path) -> DecompressResult<u64> {
    let disks = Disks::new_with_refreshed_list();
    let mut best: Option<(usize, u64)> = None;
    for disk in disks.iter() {
        let mount = disk.mount_point();
        if !probe.starts_with(mount) {
            continue;
        }
        let depth = mount.components().count();
        let available = disk.available_space();
        match best {
            Some((best_depth, _)) if best_depth >= depth => {}
            _ => best = Some((depth, available)),
        }
    }

    best.map(|(_, bytes)| bytes).ok_or_else(|| {
        DecompressError::from_external_message(format!(
            "Failed to determine available disk space for {}",
            probe.display()
        ))
    })
}

#[derive(Clone)]
pub(crate) struct ExtractBudget {
    max_total_bytes: u64,
    max_total_entries: u64,
    written_total: Arc<AtomicU64>,
    entries_total: Arc<AtomicU64>,
    disk_guard: Option<DiskSpaceGuard>,
}

impl ExtractBudget {
    pub(crate) fn new(max_total_bytes: u64, max_total_entries: u64) -> Self {
        Self {
            max_total_bytes,
            max_total_entries,
            written_total: Arc::new(AtomicU64::new(0)),
            entries_total: Arc::new(AtomicU64::new(0)),
            disk_guard: None,
        }
    }

    pub(crate) fn with_disk_guard(mut self, guard: DiskSpaceGuard) -> Self {
        self.disk_guard = Some(guard);
        self
    }

    pub(crate) fn max_total_bytes(&self) -> u64 {
        self.max_total_bytes
    }

    pub(crate) fn reserve_bytes(&self, delta: u64) -> io::Result<()> {
        if let Some(guard) = self.disk_guard.as_ref() {
            guard.maybe_check(delta)?;
        }
        loop {
            let current = self.written_total.load(Ordering::Relaxed);
            let projected = current.saturating_add(delta);
            if projected > self.max_total_bytes {
                return Err(io::Error::other(format!(
                    "Extraction exceeds size cap ({} bytes > {} bytes)",
                    projected, self.max_total_bytes
                )));
            }
            if self
                .written_total
                .compare_exchange(current, projected, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                return Ok(());
            }
        }
    }

    pub(crate) fn reserve_entry(&self, delta: u64) -> io::Result<()> {
        loop {
            let current = self.entries_total.load(Ordering::Relaxed);
            let projected = current.saturating_add(delta);
            if projected > self.max_total_entries {
                return Err(io::Error::other(format!(
                    "Extraction exceeds entry cap ({} entries > {} entries)",
                    projected, self.max_total_entries
                )));
            }
            if self
                .entries_total
                .compare_exchange(current, projected, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                return Ok(());
            }
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn capacity_query_does_not_substitute_a_parent_disk_for_an_omitted_mount() {
        // procfs is deliberately omitted from sysinfo's disk inventory. Its
        // own available capacity is zero, regardless of free space on `/`.
        assert_eq!(available_disk_bytes(Path::new("/proc")).unwrap(), 0);
    }

    #[test]
    fn reserve_is_still_enforced_before_the_first_extracted_write() {
        let guard = DiskSpaceGuard::new(std::env::temp_dir(), u64::MAX, 256 * 1024 * 1024);
        let error = guard.maybe_check(1).unwrap_err();
        assert!(error.to_string().contains("Insufficient free disk space"));
    }
}
