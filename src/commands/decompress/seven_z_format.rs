use std::{
    borrow::Cow,
    fs,
    io::{self, BufWriter},
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

use sevenz_rust2::{
    decompress_file_with_extract_fn, Archive as SevenZArchive, Error as SevenZError,
};

use super::util::{
    clean_relative_path, copy_with_progress, first_component, is_cancelled, open_unique_file,
    CreatedPaths, ExtractBudget, ProgressEmitter, SkipStats, CHUNK,
};

pub(super) fn single_root_in_7z(path: &Path) -> Result<Option<PathBuf>, String> {
    let archive = SevenZArchive::open(path).map_err(|e| format!("Failed to read 7z: {e}"))?;
    let mut root: Option<PathBuf> = None;
    for entry in archive.files {
        if entry.is_anti_item {
            continue;
        }
        let raw_path = PathBuf::from(entry.name);
        let clean_rel = match clean_relative_path(&raw_path) {
            Ok(p) => p,
            Err(_) => continue,
        };
        if clean_rel.as_os_str().is_empty() {
            continue;
        }
        let Some(first) = first_component(&clean_rel) else {
            continue;
        };
        let rest_is_empty = clean_rel.components().count() == 1;
        let is_dir = entry.is_directory || (!entry.has_stream && rest_is_empty);
        if !is_dir && rest_is_empty {
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

pub(super) fn extract_7z(
    archive_path: &Path,
    dest_dir: &Path,
    strip_prefix: Option<&Path>,
    stats: &SkipStats,
    progress: Option<&ProgressEmitter>,
    created: &mut CreatedPaths,
    cancel: Option<&AtomicBool>,
    budget: &ExtractBudget,
) -> Result<(), String> {
    let mut buf = vec![0u8; CHUNK];
    decompress_file_with_extract_fn(archive_path, dest_dir, |entry, reader, _dest_path| {
        if is_cancelled(cancel) {
            return Err(SevenZError::Io(
                io::Error::new(io::ErrorKind::Interrupted, "cancelled"),
                Cow::Borrowed("Extraction cancelled"),
            ));
        }

        if entry.is_anti_item {
            stats.skip_unsupported(&entry.name, "anti-item entry");
            return Ok(true);
        }

        let raw_name = entry.name.clone();
        let clean_rel = match clean_relative_path(Path::new(&raw_name)) {
            Ok(p) => p,
            Err(err) => {
                stats.skip_unsupported(&raw_name, &err);
                return Ok(true);
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
        if clean_rel.as_os_str().is_empty() {
            return Ok(true);
        }
        let dest_path = dest_dir.join(clean_rel);

        if entry.is_directory {
            if !dest_path.exists() {
                fs::create_dir_all(&dest_path).map_err(|e| {
                    SevenZError::Io(
                        e,
                        Cow::Owned(format!("create dir failed for {}", dest_path.display())),
                    )
                })?;
                created.record_dir(dest_path.clone());
            } else if let Err(e) = fs::create_dir_all(&dest_path) {
                stats.skip_unsupported(&raw_name, &format!("create dir failed: {e}"));
            }
            return Ok(true);
        }

        if let Some(parent) = dest_path.parent() {
            if !parent.exists() {
                if let Err(e) = fs::create_dir_all(parent) {
                    stats.skip_unsupported(&raw_name, &format!("create parent failed: {e}"));
                    return Ok(true);
                }
                created.record_dir(parent.to_path_buf());
            } else if let Err(e) = fs::create_dir_all(parent) {
                stats.skip_unsupported(&raw_name, &format!("create parent failed: {e}"));
                return Ok(true);
            }
        }

        if !entry.has_stream {
            if dest_path.exists() {
                return Ok(true);
            }
            let (_file, dest_actual) =
                open_unique_file(&dest_path).map_err(|e| SevenZError::Other(Cow::Owned(e)))?;
            created.record_file(dest_actual);
            return Ok(true);
        }

        if dest_path.exists() {
            if let Some(p) = progress {
                p.add(entry.size.max(1));
            }
            return Ok(true);
        }

        let (file, dest_actual) =
            open_unique_file(&dest_path).map_err(|e| SevenZError::Other(Cow::Owned(e)))?;
        created.record_file(dest_actual.clone());
        let mut out = BufWriter::with_capacity(CHUNK, file);
        copy_with_progress(reader, &mut out, progress, cancel, budget, &mut buf).map_err(|e| {
            SevenZError::Io(
                e,
                Cow::Owned(format!("Failed to write 7z entry {raw_name}")),
            )
        })?;
        Ok(true)
    })
    .map_err(|e| format!("Failed to extract 7z: {e}"))
}

pub(super) fn sevenz_uncompressed_total(path: &Path) -> Result<u64, String> {
    let archive =
        SevenZArchive::open(path).map_err(|e| format!("Failed to read 7z for total size: {e}"))?;
    let mut total = 0u64;
    for entry in archive.files {
        if entry.is_directory || entry.is_anti_item || !entry.has_stream {
            continue;
        }
        total = total.saturating_add(entry.size);
    }
    Ok(total)
}
