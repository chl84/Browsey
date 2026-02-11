use std::{
    fs::{self, File},
    io::{BufReader, BufWriter, Read},
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

use bzip2::read::BzDecoder;
use flate2::read::GzDecoder;
use tar::Archive;
use xz2::read::XzDecoder;
use zstd::stream::read::Decoder as ZstdDecoder;

use super::util::{
    check_cancel, clean_relative_path, copy_with_progress, first_component, map_copy_err, map_io,
    open_buffered_file, open_unique_file, CreatedPaths, ExtractBudget, ProgressEmitter, SkipStats,
    CHUNK, EXTRACT_TOTAL_ENTRIES_CAP,
};
use super::ArchiveKind;

pub(super) fn single_root_in_tar(
    path: &Path,
    kind: ArchiveKind,
) -> Result<Option<PathBuf>, String> {
    let file = File::open(path).map_err(map_io("open tar for root"))?;
    let reader = BufReader::with_capacity(CHUNK, file);
    let reader: Box<dyn Read> = match kind {
        ArchiveKind::Tar => Box::new(reader),
        ArchiveKind::TarGz => Box::new(GzDecoder::new(reader)),
        ArchiveKind::TarBz2 => Box::new(BzDecoder::new(reader)),
        ArchiveKind::TarXz => Box::new(XzDecoder::new(reader)),
        ArchiveKind::TarZstd => Box::new(
            ZstdDecoder::new(reader).map_err(|e| format!("Failed to create zstd decoder: {e}"))?,
        ),
        _ => return Ok(None),
    };
    let mut archive = Archive::new(reader);
    let mut root: Option<PathBuf> = None;
    let mut entries_seen = 0u64;
    for entry_result in archive
        .entries()
        .map_err(|e| format!("Failed to iterate tar: {e}"))?
    {
        let entry = entry_result.map_err(|e| format!("Failed to read tar entry: {e}"))?;
        entries_seen = entries_seen.saturating_add(1);
        if entries_seen > EXTRACT_TOTAL_ENTRIES_CAP {
            return Err(format!(
                "Archive exceeds entry cap ({} entries > {} entries)",
                entries_seen, EXTRACT_TOTAL_ENTRIES_CAP
            ));
        }
        let entry_type = entry.header().entry_type();
        let raw_path = entry
            .path()
            .map_err(|e| format!("Invalid tar entry path: {e}"))?
            .into_owned();
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
        let is_dir = entry_type.is_dir();
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

pub(super) fn extract_tar_with_reader<F>(
    archive_path: &Path,
    dest_dir: &Path,
    strip_prefix: Option<&Path>,
    stats: &SkipStats,
    progress: Option<&ProgressEmitter>,
    created: &mut CreatedPaths,
    cancel: Option<&AtomicBool>,
    budget: &ExtractBudget,
    wrap: F,
) -> Result<(), String>
where
    F: FnOnce(BufReader<File>) -> Result<Box<dyn Read>, String>,
{
    let reader = open_buffered_file(archive_path, "open tar")?;
    let reader = wrap(reader)?;
    extract_tar(
        reader,
        dest_dir,
        strip_prefix,
        stats,
        progress,
        created,
        cancel,
        budget,
    )?;
    Ok(())
}

pub(super) fn extract_tar<R: Read>(
    reader: R,
    dest_dir: &Path,
    strip_prefix: Option<&Path>,
    stats: &SkipStats,
    progress: Option<&ProgressEmitter>,
    created: &mut CreatedPaths,
    cancel: Option<&AtomicBool>,
    budget: &ExtractBudget,
) -> Result<(), String> {
    let mut archive = Archive::new(reader);
    let mut buf = vec![0u8; CHUNK];
    for entry_result in archive
        .entries()
        .map_err(|e| format!("Failed to iterate tar: {e}"))?
    {
        check_cancel(cancel).map_err(|e| map_copy_err("Extraction cancelled", e))?;
        let mut entry = entry_result.map_err(|e| format!("Failed to read tar entry: {e}"))?;
        budget
            .reserve_entry(1)
            .map_err(|e| map_copy_err("Extraction entry cap exceeded", e))?;
        let entry_type = entry.header().entry_type();
        let raw_path = entry
            .path()
            .map_err(|e| format!("Invalid tar entry path: {e}"))?
            .into_owned();
        let raw_str = raw_path.to_string_lossy().to_string();

        if entry_type.is_symlink() || entry_type.is_hard_link() {
            stats.skip_symlink(&raw_str);
            continue;
        }
        if !(entry_type.is_dir() || entry_type.is_file()) {
            stats.skip_unsupported(&raw_str, "unsupported type");
            continue;
        }
        let clean_rel = match clean_relative_path(&raw_path) {
            Ok(p) => p,
            Err(err) => {
                stats.skip_unsupported(&raw_str, &err);
                continue;
            }
        };
        if clean_rel.as_os_str().is_empty() {
            continue;
        }

        let clean_rel = if let Some(prefix) = strip_prefix {
            match clean_rel.strip_prefix(prefix) {
                Ok(stripped) => stripped.to_path_buf(),
                Err(_) => clean_rel,
            }
        } else {
            clean_rel
        };

        if clean_rel.as_os_str().is_empty() {
            continue;
        }

        let dest_path = dest_dir.join(clean_rel);
        if entry_type.is_dir() {
            if !dest_path.exists() {
                if let Err(e) = fs::create_dir_all(&dest_path) {
                    stats.skip_unsupported(&raw_str, &format!("create dir failed: {e}"));
                } else {
                    created.record_dir(dest_path.clone());
                }
            } else if let Err(e) = fs::create_dir_all(&dest_path) {
                stats.skip_unsupported(&raw_str, &format!("create dir failed: {e}"));
            }
            continue;
        }
        if dest_path.exists() {
            if let Some(p) = progress {
                let sz = entry.size();
                p.add(sz);
            }
            continue;
        }

        if let Some(parent) = dest_path.parent() {
            if !parent.exists() {
                if let Err(e) = fs::create_dir_all(parent) {
                    stats.skip_unsupported(&raw_str, &format!("create parent failed: {e}"));
                    continue;
                }
                created.record_dir(parent.to_path_buf());
            } else if let Err(e) = fs::create_dir_all(parent) {
                stats.skip_unsupported(&raw_str, &format!("create parent failed: {e}"));
                continue;
            }
        }
        let (file, _) = open_unique_file(&dest_path)?;
        created.record_file(dest_path.clone());
        let mut out = BufWriter::with_capacity(CHUNK, file);
        copy_with_progress(&mut entry, &mut out, progress, cancel, budget, &mut buf)
            .map_err(|e| map_copy_err("write tar entry", e))?;
    }
    Ok(())
}

pub(super) fn tar_uncompressed_total(path: &Path) -> Result<u64, String> {
    let file = File::open(path).map_err(map_io("open tar for total"))?;
    let reader = BufReader::with_capacity(CHUNK, file);
    let mut archive = Archive::new(reader);
    let mut total = 0u64;
    let entries = archive
        .entries()
        .map_err(|e| format!("Failed to iterate tar for total: {e}"))?;
    let mut entries_seen = 0u64;
    for entry_result in entries {
        let entry = entry_result.map_err(|e| format!("Failed to read tar entry for total: {e}"))?;
        entries_seen = entries_seen.saturating_add(1);
        if entries_seen > EXTRACT_TOTAL_ENTRIES_CAP {
            return Err(format!(
                "Archive exceeds entry cap ({} entries > {} entries)",
                entries_seen, EXTRACT_TOTAL_ENTRIES_CAP
            ));
        }
        let header = entry.header();
        if header.entry_type().is_dir() {
            continue;
        }
        let size = header
            .size()
            .map_err(|e| format!("Failed to read tar entry size: {e}"))?;
        total = total.saturating_add(size);
    }
    Ok(total)
}
