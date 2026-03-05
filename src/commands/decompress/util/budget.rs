use crate::commands::decompress::error::{DecompressError, DecompressResult};
use std::{
    io,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
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
    while !probe.exists() {
        let Some(parent) = probe.parent() else {
            break;
        };
        probe = parent.to_path_buf();
    }

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
            path.display()
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
