//! Read-only, bounded diagnostics. Never clear markers, unlock sessions or purge data.
use std::fs;
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use serde::Serialize;

use super::backup::{base_undo_dir, validate_undo_dir, RECOVERY_SUFFIX};
use super::{UndoError, UndoResult};

const ENTRY_LIMIT: usize = 10_000;
const TIME_LIMIT: Duration = Duration::from_millis(250);

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoStorageSummary {
    pub directory: String,
    pub exists: bool,
    pub sessions: u64,
    // Includes live operations' markers; do not imply that every marker is a failure.
    pub marked_sessions: u64,
    pub files: u64,
    pub logical_bytes: u64,
    // Filesystem-reported blocks, not exclusive physical usage on CoW filesystems.
    pub allocated_bytes: Option<u64>,
    // Root items, not the number of regular files inside folder backups.
    // Unknown when the recovery inventory cannot finish within this scan budget.
    pub backup_count: Option<u64>,
    pub recovered_backups: Option<u64>,
    pub incomplete: bool,
}

struct ScanBudget {
    remaining: usize,
    deadline: Instant,
}

impl ScanBudget {
    fn take(&mut self) -> bool {
        if self.remaining == 0 || Instant::now() >= self.deadline {
            return false;
        }
        self.remaining -= 1;
        true
    }
}

pub(super) fn inspect_storage() -> UndoResult<UndoStorageSummary> {
    let base = base_undo_dir();
    validate_undo_dir(&base)?;
    inspect_directory(
        &base,
        ScanBudget {
            remaining: ENTRY_LIMIT,
            deadline: Instant::now() + TIME_LIMIT,
        },
    )
}

fn inspect_directory(base: &Path, mut budget: ScanBudget) -> UndoResult<UndoStorageSummary> {
    // No lossy navigable path: an unsupported root encoding is an explicit error.
    let directory = base
        .to_str()
        .ok_or_else(|| UndoError::invalid_input("Undo storage path is not valid UTF-8"))?
        .to_owned();
    crate::path_guard::ensure_no_symlink_components_existing_prefix(base)
        .map_err(|error| UndoError::invalid_input(format!("Unsafe undo storage: {error}")))?;
    let mut summary = UndoStorageSummary {
        directory,
        ..UndoStorageSummary::default()
    };
    let entries = match fs::read_dir(base) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(summary),
        Err(error) => return Err(UndoError::from_io_error("Inspect undo storage", error)),
    };
    summary.exists = true;
    #[cfg(unix)]
    {
        summary.allocated_bytes = Some(0);
    }
    for entry in entries {
        if !budget.take() {
            summary.incomplete = true;
            break;
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                summary.incomplete = true;
                continue;
            }
        };
        if !entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with("session-"))
        {
            continue;
        }
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => {}
            Ok(kind) if kind.is_file() => continue, // Sibling session locks, not backups.
            _ => {
                summary.incomplete = true;
                continue;
            }
        }
        summary.sessions += 1;
        scan_session(&entry.path(), &mut budget, &mut summary);
    }
    if !summary.incomplete {
        if let Ok(Some((total, recovered))) =
            super::recovery::counts_at(base, budget.remaining, budget.deadline)
        {
            summary.backup_count = Some(total);
            summary.recovered_backups = Some(recovered);
        }
    }
    Ok(summary)
}

fn scan_session(root: &Path, budget: &mut ScanBudget, summary: &mut UndoStorageSummary) {
    let mut pending = vec![root.to_path_buf()];
    let mut marked = false;
    while let Some(directory) = pending.pop() {
        if Instant::now() >= budget.deadline || budget.remaining == 0 {
            summary.incomplete = true;
            break;
        }
        // Do not traverse ordinary symlinks, including changed ancestor paths.
        // This is diagnostic metadata, not an atomic security/ownership receipt.
        if crate::path_guard::ensure_existing_dir_nonsymlink(&directory).is_err() {
            summary.incomplete = true;
            continue;
        }
        match fs::symlink_metadata(&directory) {
            Ok(meta) if meta.is_dir() => add_allocation(&meta, summary),
            _ => {
                summary.incomplete = true;
                continue;
            }
        }
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(_) => {
                summary.incomplete = true;
                continue;
            }
        };
        for entry in entries {
            if !budget.take() {
                summary.incomplete = true;
                break;
            }
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => {
                    summary.incomplete = true;
                    continue;
                }
            };
            let is_marker = directory == root
                && entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.ends_with(RECOVERY_SUFFIX));
            marked |= is_marker; // Suspicious marker types pin sessions, too.
            match fs::symlink_metadata(entry.path()) {
                Ok(meta) if meta.is_file() => {
                    add_allocation(&meta, summary);
                    summary.files += 1;
                    if let Some(bytes) = summary.logical_bytes.checked_add(meta.len()) {
                        summary.logical_bytes = bytes;
                    } else {
                        summary.incomplete = true;
                    }
                }
                Ok(meta) if meta.is_dir() && !is_marker => pending.push(entry.path()),
                _ => summary.incomplete = true, // No symlinks, devices, or dubious markers.
            }
        }
    }
    summary.marked_sessions += u64::from(marked);
}

fn add_allocation(metadata: &fs::Metadata, summary: &mut UndoStorageSummary) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        match metadata.blocks().checked_mul(512).and_then(|bytes| {
            summary
                .allocated_bytes
                .and_then(|total| total.checked_add(bytes))
        }) {
            Some(bytes) => summary.allocated_bytes = Some(bytes),
            None => summary.incomplete = true,
        }
    }
    #[cfg(not(unix))]
    let _ = (metadata, summary);
}

#[cfg(test)]
mod tests;
