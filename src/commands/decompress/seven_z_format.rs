use std::{
    borrow::Cow,
    io::{self, BufWriter},
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

use super::password::{is_password_read_error, PasswordReader};
use sevenz_rust2::{
    Archive as SevenZArchive, ArchiveReader, EncoderMethod, Error as SevenZError, Password,
};

fn sevenz_error(error: SevenZError) -> DecompressError {
    match error {
        SevenZError::PasswordRequired => DecompressError::password_required(),
        SevenZError::MaybeBadPassword(_) => DecompressError::password_or_corrupt(),
        SevenZError::Io(ref error, _) if is_password_read_error(error) => {
            DecompressError::password_or_corrupt()
        }
        error => DecompressError::from_external_message(format!("Failed to read 7z: {error}")),
    }
}

fn is_encrypted(archive: &SevenZArchive) -> bool {
    archive.blocks.iter().any(|block| {
        block
            .coders
            .iter()
            .any(|coder| coder.encoder_method_id() == EncoderMethod::ID_AES256_SHA256)
    })
}

fn open_archive(path: &Path, control: ScanControl<'_>) -> DecompressResult<SevenZArchive> {
    let archive = SevenZArchive::open_with_password(
        path,
        &Password::new(control.password.unwrap_or_default()),
    )
    .map_err(sevenz_error)?;
    if control.password.is_none() && is_encrypted(&archive) {
        return Err(DecompressError::password_required());
    }
    Ok(archive)
}

use super::error::{DecompressError, DecompressResult};
use super::util::ScanControl;
use super::util::{
    clean_relative_path, copy_with_progress, ensure_dir_nofollow, first_component, is_cancelled,
    open_unique_file, path_exists_nofollow, restore_file_mode, sevenz_mode, CreatedPaths,
    ExtractBudget, ProgressEmitter, SkipStats, CHUNK, EXTRACT_TOTAL_ENTRIES_CAP,
};
use crate::errors::domain::DomainError;

pub(super) fn single_root_in_7z(
    path: &Path,
    control: ScanControl<'_>,
) -> DecompressResult<Option<PathBuf>> {
    control.check()?;
    let archive = open_archive(path, control)?;
    let mut root: Option<PathBuf> = None;
    let mut entries_seen = 0u64;
    for entry in archive.files {
        control.check()?;
        entries_seen = entries_seen.saturating_add(1);
        if entries_seen > EXTRACT_TOTAL_ENTRIES_CAP {
            return Err(format!(
                "Archive exceeds entry cap ({} entries > {} entries)",
                entries_seen, EXTRACT_TOTAL_ENTRIES_CAP
            )
            .into());
        }
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
        let is_dir = entry.is_directory;
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

#[allow(clippy::too_many_arguments)]
pub(super) fn extract_7z(
    archive_path: &Path,
    dest_dir: &Path,
    strip_prefix: Option<&Path>,
    stats: &SkipStats,
    progress: Option<&ProgressEmitter>,
    created: &mut CreatedPaths,
    cancel: Option<&AtomicBool>,
    budget: &ExtractBudget,
    password: Option<&str>,
) -> DecompressResult<()> {
    let mut buf = vec![0u8; CHUNK];
    let mut archive =
        ArchiveReader::open(archive_path, Password::new(password.unwrap_or_default()))
            .map_err(sevenz_error)?;
    let encrypted = is_encrypted(archive.archive());
    archive
        .for_each_entries(|entry, reader| {
            budget
                .reserve_entry(1)
                .map_err(|e| SevenZError::Io(e, Cow::Borrowed("Extraction entry cap exceeded")))?;
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
            let mode = sevenz_mode(entry.has_windows_attributes, entry.windows_attributes);
            let clean_rel = match clean_relative_path(Path::new(&raw_name)) {
                Ok(p) => p,
                Err(err) => {
                    stats.skip_unsupported(&raw_name, &err.to_string());
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
                if entry.is_directory {
                    created.defer_directory_mode(dest_dir.to_path_buf(), mode);
                }
                return Ok(true);
            }
            let dest_path = dest_dir.join(clean_rel);

            if entry.is_directory {
                created.defer_directory_mode(dest_path.clone(), mode);
                match ensure_dir_nofollow(&dest_path) {
                    Ok(created_dirs) => {
                        for dir in created_dirs {
                            created.record_dir(dir);
                        }
                    }
                    Err(e) => {
                        stats.skip_unsupported(&raw_name, &format!("create dir failed: {e}"));
                    }
                }
                return Ok(true);
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
                        return Ok(true);
                    }
                }
            }

            if !entry.has_stream {
                match path_exists_nofollow(&dest_path) {
                    Ok(true) => return Ok(true),
                    Ok(false) => {}
                    Err(e) => {
                        stats.skip_unsupported(&raw_name, &format!("stat destination failed: {e}"));
                        return Ok(true);
                    }
                }
                let (file, dest_actual) = open_unique_file(&dest_path)
                    .map_err(|e| SevenZError::Other(Cow::Owned(e.message().to_owned())))?;
                created.record_file(dest_actual);
                restore_file_mode(&file, mode)
                    .map_err(|e| SevenZError::Other(Cow::Owned(e.to_string())))?;
                return Ok(true);
            }

            match path_exists_nofollow(&dest_path) {
                Ok(true) => {
                    if let Some(p) = progress {
                        p.add(entry.size.max(1));
                    }
                    return Ok(true);
                }
                Ok(false) => {}
                Err(e) => {
                    stats.skip_unsupported(&raw_name, &format!("stat destination failed: {e}"));
                    return Ok(true);
                }
            }

            let (file, dest_actual) = open_unique_file(&dest_path)
                .map_err(|e| SevenZError::Other(Cow::Owned(e.message().to_owned())))?;
            created.record_file(dest_actual);
            let mut out = BufWriter::with_capacity(CHUNK, file);
            copy_with_progress(
                &mut PasswordReader { reader, encrypted },
                &mut out,
                progress,
                cancel,
                budget,
                &mut buf,
            )
            .map_err(|e| {
                SevenZError::Io(
                    e,
                    Cow::Owned(format!("Failed to write 7z entry {raw_name}")),
                )
            })?;
            restore_file_mode(out.get_ref(), mode)
                .map_err(|e| SevenZError::Other(Cow::Owned(e.to_string())))?;
            Ok(true)
        })
        .map_err(sevenz_error)
}

pub(super) fn sevenz_uncompressed_total(
    path: &Path,
    control: ScanControl<'_>,
) -> DecompressResult<u64> {
    control.check()?;
    let archive = open_archive(path, control)?;
    let mut total = 0u64;
    let mut entries_seen = 0u64;
    for entry in archive.files {
        control.check()?;
        entries_seen = entries_seen.saturating_add(1);
        if entries_seen > EXTRACT_TOTAL_ENTRIES_CAP {
            return Err(format!(
                "Archive exceeds entry cap ({} entries > {} entries)",
                entries_seen, EXTRACT_TOTAL_ENTRIES_CAP
            )
            .into());
        }
        if entry.is_directory || entry.is_anti_item || !entry.has_stream {
            continue;
        }
        total = total.saturating_add(entry.size);
    }
    Ok(total)
}
