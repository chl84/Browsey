use std::{
    fs::File,
    io::BufWriter,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

use zip::ZipArchive;

use super::error::{DecompressError, DecompressResult};
use super::util::{
    check_cancel, clean_relative_path, copy_with_progress, ensure_dir_nofollow, first_component,
    map_copy_err, map_io, open_unique_file, path_exists_nofollow, CreatedPaths, ExtractBudget,
    ProgressEmitter, SkipStats, CHUNK, EXTRACT_TOTAL_ENTRIES_CAP,
};
use crate::fs_utils::debug_log;

pub(super) fn single_root_in_zip(path: &Path) -> DecompressResult<Option<PathBuf>> {
    let mut archive = ZipArchive::new(File::open(path).map_err(map_io("open zip for root"))?)
        .map_err(|e| format!("Failed to read zip: {e}"))?;
    let mut root: Option<PathBuf> = None;
    let mut entries_seen = 0u64;
    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry {i}: {e}"))?;
        entries_seen = entries_seen.saturating_add(1);
        if entries_seen > EXTRACT_TOTAL_ENTRIES_CAP {
            return Err(format!(
                "Archive exceeds entry cap ({} entries > {} entries)",
                entries_seen, EXTRACT_TOTAL_ENTRIES_CAP
            )
            .into());
        }
        let raw_name = entry.name().to_string();
        let enclosed = entry.enclosed_name().ok_or_else(|| {
            debug_log(&format!("Skipping zip entry with unsafe name: {raw_name}"));
            "skipped".to_string()
        });
        let enclosed = match enclosed {
            Ok(p) => p.to_path_buf(),
            Err(_) => continue,
        };
        let clean_rel = match clean_relative_path(&enclosed) {
            Ok(p) => p,
            Err(_) => continue,
        };
        if clean_rel.as_os_str().is_empty() {
            continue;
        }
        let Some(first) = first_component(&clean_rel) else {
            continue;
        };
        let is_dir = entry.is_dir() || raw_name.ends_with('/');
        let rest_is_empty = clean_rel.components().count() == 1;
        if !is_dir && rest_is_empty {
            // File i roten -> ikke enkel rotmappe.
            return Ok(None);
        }
        match &root {
            Some(r) if r != &first => return Ok(None),
            None => root = Some(first),
            _ => {}
        }
    }
    Ok(root)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn extract_zip(
    path: &Path,
    dest_dir: &Path,
    strip_prefix: Option<&Path>,
    stats: &SkipStats,
    progress: Option<&ProgressEmitter>,
    created: &mut CreatedPaths,
    cancel: Option<&AtomicBool>,
    budget: &ExtractBudget,
) -> DecompressResult<()> {
    let mut archive = ZipArchive::new(File::open(path).map_err(map_io("open zip"))?)
        .map_err(|e| format!("Failed to read zip: {e}"))?;
    let mut buf = vec![0u8; CHUNK];

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry {i}: {e}"))?;
        budget
            .reserve_entry(1)
            .map_err(|e| map_copy_err("Extraction entry cap exceeded", e))?;
        let raw_name = entry.name().to_string();
        let enclosed = entry.enclosed_name().ok_or_else(|| {
            stats.skip_unsupported(&raw_name, "path traversal");
            "skipped".to_string()
        });
        let enclosed = match enclosed {
            Ok(p) => p.to_path_buf(),
            Err(_) => continue,
        };
        let clean_rel = match clean_relative_path(&enclosed) {
            Ok(p) => p,
            Err(err) => {
                stats.skip_unsupported(&raw_name, &err.to_string());
                continue;
            }
        };
        let clean_rel = if let Some(prefix) = strip_prefix {
            match clean_rel.strip_prefix(prefix) {
                Ok(stripped) => stripped.to_path_buf(),
                Err(_) => clean_rel,
            }
        } else {
            clean_rel
        };
        check_cancel(cancel).map_err(|e| map_copy_err("Extraction cancelled", e))?;
        if clean_rel.as_os_str().is_empty() {
            continue;
        }
        let is_symlink = entry
            .unix_mode()
            .map(|mode| (mode & 0o170000) == 0o120000)
            .unwrap_or(false);
        if is_symlink {
            stats.skip_symlink(&raw_name);
            continue;
        }
        let dest_path = dest_dir.join(clean_rel);
        if entry.is_dir() || raw_name.ends_with('/') {
            match ensure_dir_nofollow(&dest_path) {
                Ok(created_dirs) => {
                    for dir in created_dirs {
                        created.record_dir(dir);
                    }
                }
                Err(e) => {
                    stats.skip_unsupported(&raw_name, &format!("create dir failed: {e}"));
                    continue;
                }
            }
            continue;
        }
        match path_exists_nofollow(&dest_path) {
            Ok(true) => {
                if let Some(p) = progress {
                    let inc = entry.compressed_size().max(1);
                    p.add(inc);
                }
                continue;
            }
            Ok(false) => {}
            Err(e) => {
                stats.skip_unsupported(&raw_name, &format!("stat destination failed: {e}"));
                continue;
            }
        }
        if let Some(parent) = dest_path.parent() {
            match ensure_dir_nofollow(parent) {
                Ok(created_dirs) => {
                    for dir in created_dirs {
                        created.record_dir(dir);
                    }
                }
                Err(e) => {
                    stats.skip_unsupported(&raw_name, &format!("create parent failed: {e}"));
                    continue;
                }
            }
        }
        let (file, actual_path) = open_unique_file(&dest_path)?;
        created.record_file(actual_path);
        let mut out = BufWriter::with_capacity(CHUNK, file);
        if let Err(e) = copy_with_progress(&mut entry, &mut out, progress, cancel, budget, &mut buf)
        {
            let msg = map_copy_err(&format!("Failed to write zip entry {raw_name}"), e);
            return Err(DecompressError::from_external_message(msg));
        }
    }

    Ok(())
}

pub(super) fn zip_uncompressed_total(path: &Path) -> DecompressResult<u64> {
    let mut archive = ZipArchive::new(File::open(path).map_err(map_io("open zip for total"))?)
        .map_err(|e| format!("Failed to read zip: {e}"))?;
    let mut total = 0u64;
    let mut entries_seen = 0u64;
    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry {i}: {e}"))?;
        entries_seen = entries_seen.saturating_add(1);
        if entries_seen > EXTRACT_TOTAL_ENTRIES_CAP {
            return Err(format!(
                "Archive exceeds entry cap ({} entries > {} entries)",
                entries_seen, EXTRACT_TOTAL_ENTRIES_CAP
            )
            .into());
        }
        if entry.is_dir() {
            continue;
        }
        total = total.saturating_add(entry.size());
    }
    Ok(total)
}
