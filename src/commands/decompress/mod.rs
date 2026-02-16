mod rar_format;
mod seven_z_format;
mod tar_format;
mod util;
mod zip_format;

use std::{
    fs,
    fs::File,
    io::{BufReader, BufWriter, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc, Mutex},
};

use bzip2::read::BzDecoder;
use flate2::read::{GzDecoder, MultiGzDecoder};
use rar_stream::InnerFile as RarInnerFile;
use serde::Serialize;
use xz2::read::XzDecoder;
use zstd::stream::read::Decoder as ZstdDecoder;

use super::tasks::{CancelGuard, CancelState};
use crate::fs_utils::{check_no_symlink_components, sanitize_path_follow, sanitize_path_nofollow};
use crate::undo::{temp_backup_path, Action, UndoState};

use rar_format::{
    extract_rar, parse_rar_entries, rar_uncompressed_total_from_entries, single_root_in_rar,
};
use seven_z_format::{extract_7z, sevenz_uncompressed_total, single_root_in_7z};
use tar_format::{extract_tar_with_reader, single_root_in_tar, tar_uncompressed_total};
use util::{
    available_disk_bytes, copy_with_progress, create_unique_dir_nofollow,
    effective_extract_bytes_cap, ensure_dir_nofollow, map_copy_err, map_io, open_buffered_file,
    open_unique_file, strip_known_suffixes, CreatedPaths, DiskSpaceGuard, ExtractBudget,
    ProgressEmitter, SkipStats, CHUNK, EXTRACT_DISK_CHECK_INTERVAL_BYTES,
    EXTRACT_MIN_FREE_DISK_RESERVE, EXTRACT_TOTAL_BYTES_CAP, EXTRACT_TOTAL_ENTRIES_CAP,
};
use zip_format::{extract_zip, single_root_in_zip, zip_uncompressed_total};

#[derive(Debug, Clone, Copy)]
enum ArchiveKind {
    Zip,
    Tar,
    TarGz,
    TarBz2,
    TarXz,
    TarZstd,
    SevenZ,
    Rar,
    Gz,
    Bz2,
    Xz,
    Zstd,
}

#[derive(Serialize)]
pub struct ExtractResult {
    pub destination: String,
    pub skipped_symlinks: usize,
    pub skipped_entries: usize,
}

#[derive(Serialize)]
pub struct ExtractBatchItem {
    pub path: String,
    pub ok: bool,
    pub result: Option<ExtractResult>,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn extract_archive(
    app: tauri::AppHandle,
    cancel: tauri::State<'_, CancelState>,
    undo: tauri::State<'_, UndoState>,
    path: String,
    progress_event: Option<String>,
) -> Result<ExtractResult, String> {
    let cancel_state = cancel.inner().clone();
    let undo_state = undo.inner().clone();
    let task = tauri::async_runtime::spawn_blocking(move || {
        do_extract(
            app,
            cancel_state,
            undo_state,
            path,
            progress_event,
            None,
            None,
            None,
        )
    });
    task.await
        .map_err(|e| format!("Extraction task failed: {e}"))?
}

#[tauri::command]
pub async fn extract_archives(
    app: tauri::AppHandle,
    cancel: tauri::State<'_, CancelState>,
    undo: tauri::State<'_, UndoState>,
    paths: Vec<String>,
    progress_event: Option<String>,
) -> Result<Vec<ExtractBatchItem>, String> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let cancel_state = cancel.inner().clone();
    let undo_state = undo.inner().clone();

    // Single cancel token for the whole batch if requested.
    let batch_guard = if let Some(evt) = progress_event.clone() {
        Some(
            cancel_state
                .register(evt)
                .map_err(|e| format!("Failed to register cancel: {e}"))?,
        )
    } else {
        None
    };
    let batch_token = batch_guard.as_ref().map(|g| g.token());

    let task = tauri::async_runtime::spawn_blocking(move || {
        // Compute batch total hint inside blocking context to avoid nested runtimes.
        let mut batch_total: u64 = 0;
        for path in &paths {
            let path_buf = PathBuf::from(path);
            batch_total = batch_total.saturating_add(estimate_total_hint(&path_buf).unwrap_or(1));
        }
        if batch_total == 0 {
            batch_total = 1;
        }

        let shared_progress = progress_event
            .as_ref()
            .map(|evt| ProgressEmitter::new(app.clone(), evt.clone(), batch_total));

        let mut results = Vec::new();
        let batch_actions: Arc<Mutex<Vec<Action>>> = Arc::new(Mutex::new(Vec::new()));
        for (_idx, path) in paths.into_iter().enumerate() {
            let res = do_extract(
                app.clone(),
                cancel_state.clone(),
                undo_state.clone(),
                path.clone(),
                progress_event.clone(), // only used for cancel registration in single mode
                batch_token.clone(),
                shared_progress.clone(),
                Some(batch_actions.clone()),
            );
            match res {
                Ok(r) => results.push(ExtractBatchItem {
                    path,
                    ok: true,
                    result: Some(r),
                    error: None,
                }),
                Err(e) => {
                    let was_cancel = e.to_lowercase().contains("cancelled");
                    results.push(ExtractBatchItem {
                        path,
                        ok: false,
                        result: None,
                        error: Some(e),
                    });
                    if was_cancel {
                        break;
                    }
                }
            }
        }
        // keep guard alive until loop ends
        drop(batch_guard);
        if let Ok(actions) = batch_actions.lock() {
            if !actions.is_empty() {
                let _ = undo_state.record_applied(Action::Batch(actions.clone()));
            }
        }
        if let Some(p) = shared_progress {
            p.finish();
        }
        Ok(results)
    });
    task.await
        .map_err(|e| format!("Batch extraction task failed: {e}"))?
}

fn do_extract(
    app: tauri::AppHandle,
    cancel_state: CancelState,
    undo: UndoState,
    path: String,
    progress_event: Option<String>,
    shared_cancel: Option<Arc<AtomicBool>>,
    shared_progress: Option<ProgressEmitter>,
    batch_actions: Option<Arc<Mutex<Vec<Action>>>>,
) -> Result<ExtractResult, String> {
    let nofollow = sanitize_path_nofollow(&path, true)?;
    let meta = fs::symlink_metadata(&nofollow)
        .map_err(|e| format!("Failed to read archive metadata: {e}"))?;
    if meta.file_type().is_symlink() {
        return Err("Symlink archives are not supported".into());
    }

    let archive_path = sanitize_path_follow(&path, true)?;
    check_no_symlink_components(&archive_path)?;

    if !archive_path.is_file() {
        return Err("Only files can be extracted".into());
    }

    let parent = archive_path
        .parent()
        .ok_or_else(|| "Cannot extract archive at filesystem root".to_string())?;

    let kind = detect_archive(&archive_path)?;
    let mut rar_entries: Option<Vec<RarInnerFile>> = None;
    let total_hint = match kind {
        ArchiveKind::Zip => zip_uncompressed_total(&archive_path).unwrap_or_else(|_| meta.len()),
        ArchiveKind::Tar => tar_uncompressed_total(&archive_path).unwrap_or_else(|_| meta.len()),
        ArchiveKind::TarGz => gzip_uncompressed_size(&archive_path).unwrap_or_else(|_| meta.len()),
        ArchiveKind::SevenZ => {
            sevenz_uncompressed_total(&archive_path).unwrap_or_else(|_| meta.len())
        }
        ArchiveKind::Rar => {
            let entries = parse_rar_entries(&archive_path)?;
            let total =
                rar_uncompressed_total_from_entries(&entries).unwrap_or_else(|_| meta.len());
            rar_entries = Some(entries);
            total
        }
        ArchiveKind::Gz => gzip_uncompressed_size(&archive_path).unwrap_or_else(|_| meta.len()),
        _ => meta.len(),
    }
    .max(1);
    let available_bytes = available_disk_bytes(parent)?;
    let effective_bytes_cap = effective_extract_bytes_cap(
        EXTRACT_TOTAL_BYTES_CAP,
        available_bytes,
        EXTRACT_MIN_FREE_DISK_RESERVE,
    );
    if effective_bytes_cap == 0 {
        return Err(format!(
            "Insufficient free disk space in {} (available: {} bytes, required reserve: {} bytes)",
            parent.display(),
            available_bytes,
            EXTRACT_MIN_FREE_DISK_RESERVE
        ));
    }
    let budget = ExtractBudget::new(effective_bytes_cap, EXTRACT_TOTAL_ENTRIES_CAP)
        .with_disk_guard(DiskSpaceGuard::new(
            parent.to_path_buf(),
            EXTRACT_MIN_FREE_DISK_RESERVE,
            EXTRACT_DISK_CHECK_INTERVAL_BYTES,
        ));
    if total_hint > budget.max_total_bytes() {
        return Err(format!(
            "Archive exceeds extraction size cap ({} bytes > {} bytes, available disk: {} bytes)",
            total_hint,
            budget.max_total_bytes(),
            available_bytes
        ));
    }
    let progress_id = progress_event.clone();
    let mut owns_progress = false;
    let progress = if let Some(p) = shared_progress {
        Some(p)
    } else if let Some(evt) = progress_id.as_ref() {
        owns_progress = true;
        Some(ProgressEmitter::new(app.clone(), evt.clone(), total_hint))
    } else {
        None
    };
    let mut created = CreatedPaths::default();

    let mut _cancel_guard: Option<CancelGuard> = None;
    let cancel_token_arc: Option<Arc<AtomicBool>> = if let Some(shared) = shared_cancel {
        Some(shared)
    } else if let Some(evt) = progress_id.as_ref() {
        let guard = cancel_state.register(evt.clone())?;
        let token = guard.token();
        _cancel_guard = Some(guard);
        Some(token)
    } else {
        None
    };
    let cancel_token = cancel_token_arc.as_deref();

    let stats = SkipStats::default();
    let destination = match kind {
        ArchiveKind::Zip => {
            let (dest_dir, strip) = choose_destination_dir(&archive_path, kind)?;
            created.record_dir(dest_dir.clone());
            extract_zip(
                &archive_path,
                &dest_dir,
                strip.as_deref(),
                &stats,
                progress.as_ref(),
                &mut created,
                cancel_token.as_deref(),
                &budget,
            )?;
            dest_dir
        }
        ArchiveKind::Tar => {
            let (dest_dir, strip) = choose_destination_dir(&archive_path, kind)?;
            created.record_dir(dest_dir.clone());
            extract_tar_with_reader(
                &archive_path,
                &dest_dir,
                strip.as_deref(),
                &stats,
                progress.as_ref(),
                &mut created,
                cancel_token.as_deref(),
                &budget,
                |reader| Ok(Box::new(reader) as Box<dyn Read>),
            )?;
            dest_dir
        }
        ArchiveKind::TarGz => {
            let (dest_dir, strip) = choose_destination_dir(&archive_path, kind)?;
            created.record_dir(dest_dir.clone());
            extract_tar_with_reader(
                &archive_path,
                &dest_dir,
                strip.as_deref(),
                &stats,
                progress.as_ref(),
                &mut created,
                cancel_token.as_deref(),
                &budget,
                |reader| Ok(Box::new(GzDecoder::new(reader)) as Box<dyn Read>),
            )?;
            dest_dir
        }
        ArchiveKind::TarBz2 => {
            let (dest_dir, strip) = choose_destination_dir(&archive_path, kind)?;
            created.record_dir(dest_dir.clone());
            extract_tar_with_reader(
                &archive_path,
                &dest_dir,
                strip.as_deref(),
                &stats,
                progress.as_ref(),
                &mut created,
                cancel_token.as_deref(),
                &budget,
                |reader| Ok(Box::new(BzDecoder::new(reader)) as Box<dyn Read>),
            )?;
            dest_dir
        }
        ArchiveKind::TarXz => {
            let (dest_dir, strip) = choose_destination_dir(&archive_path, kind)?;
            created.record_dir(dest_dir.clone());
            extract_tar_with_reader(
                &archive_path,
                &dest_dir,
                strip.as_deref(),
                &stats,
                progress.as_ref(),
                &mut created,
                cancel_token.as_deref(),
                &budget,
                |reader| Ok(Box::new(XzDecoder::new(reader)) as Box<dyn Read>),
            )?;
            dest_dir
        }
        ArchiveKind::TarZstd => {
            let (dest_dir, strip) = choose_destination_dir(&archive_path, kind)?;
            created.record_dir(dest_dir.clone());
            extract_tar_with_reader(
                &archive_path,
                &dest_dir,
                strip.as_deref(),
                &stats,
                progress.as_ref(),
                &mut created,
                cancel_token.as_deref(),
                &budget,
                |reader| {
                    ZstdDecoder::new(reader)
                        .map(|r| Box::new(r) as Box<dyn Read>)
                        .map_err(|e| format!("Failed to create zstd decoder: {e}"))
                },
            )?;
            dest_dir
        }
        ArchiveKind::SevenZ => {
            let (dest_dir, strip) = choose_destination_dir(&archive_path, kind)?;
            created.record_dir(dest_dir.clone());
            extract_7z(
                &archive_path,
                &dest_dir,
                strip.as_deref(),
                &stats,
                progress.as_ref(),
                &mut created,
                cancel_token.as_deref(),
                &budget,
            )?;
            dest_dir
        }
        ArchiveKind::Rar => {
            let entries = match rar_entries {
                Some(v) => v,
                None => parse_rar_entries(&archive_path)?,
            };
            let (dest_dir, strip) = choose_destination_dir(&archive_path, kind)?;
            created.record_dir(dest_dir.clone());
            extract_rar(
                entries,
                &dest_dir,
                strip.as_deref(),
                &stats,
                progress.as_ref(),
                &mut created,
                cancel_token.as_deref(),
                &budget,
            )?;
            dest_dir
        }
        ArchiveKind::Gz => decompress_single_with_reader(
            &archive_path,
            parent,
            progress.as_ref(),
            &mut created,
            cancel_token.as_deref(),
            &budget,
            |reader| Ok(Box::new(MultiGzDecoder::new(reader)) as Box<dyn Read>),
        )?,
        ArchiveKind::Bz2 => decompress_single_with_reader(
            &archive_path,
            parent,
            progress.as_ref(),
            &mut created,
            cancel_token.as_deref(),
            &budget,
            |reader| Ok(Box::new(BzDecoder::new(reader)) as Box<dyn Read>),
        )?,
        ArchiveKind::Xz => decompress_single_with_reader(
            &archive_path,
            parent,
            progress.as_ref(),
            &mut created,
            cancel_token.as_deref(),
            &budget,
            |reader| Ok(Box::new(XzDecoder::new(reader)) as Box<dyn Read>),
        )?,
        ArchiveKind::Zstd => decompress_single_with_reader(
            &archive_path,
            parent,
            progress.as_ref(),
            &mut created,
            cancel_token.as_deref(),
            &budget,
            |reader| {
                ZstdDecoder::new(reader)
                    .map(|r| Box::new(r) as Box<dyn Read>)
                    .map_err(|e| format!("Failed to create zstd decoder: {e}"))
            },
        )?,
    };

    if owns_progress {
        if let Some(p) = progress.as_ref() {
            p.finish();
        }
    }
    created.disarm();

    let backup = temp_backup_path(&destination);
    let action = Action::Create {
        path: destination.clone(),
        backup,
    };
    if let Some(list) = batch_actions.as_ref() {
        if let Ok(mut guard) = list.lock() {
            guard.push(action);
        }
    } else {
        let _ = undo.record_applied(action);
    }

    Ok(ExtractResult {
        destination: destination.to_string_lossy().into_owned(),
        skipped_symlinks: stats.symlinks.load(std::sync::atomic::Ordering::Relaxed),
        skipped_entries: stats.unsupported.load(std::sync::atomic::Ordering::Relaxed),
    })
}

fn archive_root_name(path: &Path) -> String {
    path.file_name()
        .and_then(|s| s.to_str())
        .map(strip_known_suffixes)
        .map(|s| {
            if s.is_empty() {
                "extracted".to_string()
            } else {
                s
            }
        })
        .unwrap_or_else(|| "extracted".to_string())
}

fn create_unique_dir(parent: &Path, base: &str) -> Result<PathBuf, String> {
    create_unique_dir_nofollow(parent, base)
}

fn prepare_output_dir(archive_path: &Path) -> Result<PathBuf, String> {
    let parent = archive_path
        .parent()
        .ok_or_else(|| "Cannot extract archive at filesystem root".to_string())?;
    let base = archive_root_name(archive_path);
    create_unique_dir(parent, &base)
}

fn detect_archive(path: &Path) -> Result<ArchiveKind, String> {
    let mut f = File::open(path).map_err(map_io("open archive for detection"))?;
    let mut buf = [0u8; 512];
    let n = f.read(&mut buf).map_err(map_io("read archive header"))?;
    let magic = &buf[..n];

    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_lowercase();

    if magic.starts_with(b"PK\x03\x04")
        || magic.starts_with(b"PK\x05\x06")
        || magic.starts_with(b"PK\x07\x08")
    {
        return Ok(ArchiveKind::Zip);
    }
    if magic.starts_with(b"Rar!\x1A\x07\x00") || magic.starts_with(b"Rar!\x1A\x07\x01\x00") {
        return Ok(ArchiveKind::Rar);
    }
    if magic.starts_with(&[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C]) {
        return Ok(ArchiveKind::SevenZ);
    }
    if magic.starts_with(&[0x1F, 0x8B]) {
        return Ok(if has_suffix(&name, &[".tar.gz", ".tgz"]) {
            ArchiveKind::TarGz
        } else {
            ArchiveKind::Gz
        });
    }
    if magic.starts_with(b"BZh") {
        return Ok(if has_suffix(&name, &[".tar.bz2", ".tbz2"]) {
            ArchiveKind::TarBz2
        } else {
            ArchiveKind::Bz2
        });
    }
    if magic.starts_with(&[0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00]) {
        return Ok(if has_suffix(&name, &[".tar.xz", ".txz"]) {
            ArchiveKind::TarXz
        } else {
            ArchiveKind::Xz
        });
    }
    if magic.starts_with(&[0x28, 0xB5, 0x2F, 0xFD]) {
        return Ok(if has_suffix(&name, &[".tar.zst", ".tzst"]) {
            ArchiveKind::TarZstd
        } else {
            ArchiveKind::Zstd
        });
    }
    if magic.len() >= 262 && &magic[257..262] == b"ustar" {
        return Ok(ArchiveKind::Tar);
    }

    for (suffixes, kind) in [
        (&[".tar.gz", ".tgz"][..], ArchiveKind::TarGz),
        (&[".tar.bz2", ".tbz2"][..], ArchiveKind::TarBz2),
        (&[".tar.xz", ".txz"][..], ArchiveKind::TarXz),
        (&[".tar.zst", ".tzst"][..], ArchiveKind::TarZstd),
        (&[".tar"][..], ArchiveKind::Tar),
        (&[".zip"][..], ArchiveKind::Zip),
        (&[".7z"][..], ArchiveKind::SevenZ),
        (&[".rar"][..], ArchiveKind::Rar),
        (&[".gz"][..], ArchiveKind::Gz),
        (&[".bz2"][..], ArchiveKind::Bz2),
        (&[".xz"][..], ArchiveKind::Xz),
        (&[".zst"][..], ArchiveKind::Zstd),
    ] {
        if has_suffix(&name, suffixes) {
            return Ok(kind);
        }
    }

    Err("Unsupported archive format".to_string())
}

fn has_suffix(name: &str, suffixes: &[&str]) -> bool {
    suffixes.iter().any(|s| name.ends_with(s))
}

fn choose_destination_dir(
    archive_path: &Path,
    kind: ArchiveKind,
) -> Result<(PathBuf, Option<PathBuf>), String> {
    let parent = archive_path
        .parent()
        .ok_or_else(|| "Cannot extract archive at filesystem root".to_string())?;

    let single_root = match kind {
        ArchiveKind::Zip => single_root_in_zip(archive_path)?,
        ArchiveKind::Tar
        | ArchiveKind::TarGz
        | ArchiveKind::TarBz2
        | ArchiveKind::TarXz
        | ArchiveKind::TarZstd => single_root_in_tar(archive_path, kind)?,
        ArchiveKind::SevenZ => single_root_in_7z(archive_path)?,
        ArchiveKind::Rar => single_root_in_rar(archive_path)?,
        _ => None,
    };

    if let Some(root) = single_root {
        let name = root.to_string_lossy();
        let dir = create_unique_dir(parent, &name)?;
        Ok((dir, Some(root)))
    } else {
        let dir = prepare_output_dir(archive_path)?;
        Ok((dir, None))
    }
}

fn decompress_single_with_reader<F>(
    archive_path: &Path,
    parent: &Path,
    progress: Option<&ProgressEmitter>,
    created: &mut CreatedPaths,
    cancel: Option<&AtomicBool>,
    budget: &ExtractBudget,
    wrap: F,
) -> Result<PathBuf, String>
where
    F: FnOnce(BufReader<File>) -> Result<Box<dyn Read>, String>,
{
    let reader = open_buffered_file(archive_path, "open compressed file")?;
    let reader = wrap(reader)?;
    decompress_single(
        reader,
        archive_path,
        parent,
        progress,
        created,
        cancel,
        budget,
    )
}

fn decompress_single<R: Read>(
    mut reader: R,
    archive: &Path,
    parent: &Path,
    progress: Option<&ProgressEmitter>,
    created: &mut CreatedPaths,
    cancel: Option<&AtomicBool>,
    budget: &ExtractBudget,
) -> Result<PathBuf, String> {
    budget
        .reserve_entry(1)
        .map_err(|e| map_copy_err("Extraction entry cap exceeded", e))?;
    let mut dest_name = archive
        .file_name()
        .and_then(|s| s.to_str())
        .map(strip_known_suffixes)
        .unwrap_or_else(|| "extracted".to_string());
    if dest_name.is_empty() {
        dest_name = "extracted".to_string();
    }
    let dest_path = parent.join(dest_name);
    if let Some(parent_dir) = dest_path.parent() {
        let created_dirs = ensure_dir_nofollow(parent_dir)
            .map_err(|e| format!("Failed to create output dir {}: {e}", parent_dir.display()))?;
        for dir in created_dirs {
            created.record_dir(dir);
        }
    }
    let (file, dest_path) = open_unique_file(&dest_path)?;
    created.record_file(dest_path.clone());
    let mut out = BufWriter::with_capacity(CHUNK, file);
    let mut buf = vec![0u8; CHUNK];
    copy_with_progress(&mut reader, &mut out, progress, cancel, budget, &mut buf)
        .map_err(|e| map_copy_err("write decompressed file", e))?;
    Ok(dest_path)
}

fn gzip_uncompressed_size(path: &Path) -> Result<u64, String> {
    let mut file = File::open(path).map_err(map_io("open gzip for total"))?;
    let len = file.metadata().map_err(map_io("read gzip metadata"))?.len();
    if len < 4 {
        return Err("gzip too small to contain size footer".into());
    }
    file.seek(SeekFrom::End(-4))
        .map_err(map_io("seek gzip footer"))?;
    let mut buf = [0u8; 4];
    file.read_exact(&mut buf)
        .map_err(map_io("read gzip footer"))?;
    let size = u32::from_le_bytes(buf) as u64;
    Ok(size.max(1))
}

fn estimate_total_hint(path: &Path) -> Result<u64, String> {
    let meta = fs::metadata(path).map_err(|e| format!("Failed to read archive metadata: {e}"))?;
    let kind = detect_archive(path)?;
    let mut rar_entries: Option<Vec<RarInnerFile>> = None;
    let total = match kind {
        ArchiveKind::Zip => zip_uncompressed_total(path).unwrap_or_else(|_| meta.len()),
        ArchiveKind::Tar => tar_uncompressed_total(path).unwrap_or_else(|_| meta.len()),
        ArchiveKind::TarGz => gzip_uncompressed_size(path).unwrap_or_else(|_| meta.len()),
        ArchiveKind::SevenZ => sevenz_uncompressed_total(path).unwrap_or_else(|_| meta.len()),
        ArchiveKind::Rar => {
            let entries = parse_rar_entries(path)?;
            let total =
                rar_uncompressed_total_from_entries(&entries).unwrap_or_else(|_| meta.len());
            rar_entries = Some(entries);
            total
        }
        ArchiveKind::Gz => gzip_uncompressed_size(path).unwrap_or_else(|_| meta.len()),
        _ => meta.len(),
    }
    .max(1);
    let _ = rar_entries; // parsed only for size
    Ok(total)
}
