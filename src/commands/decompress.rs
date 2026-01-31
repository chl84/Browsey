use std::{
    fs::{self, File},
    io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use bzip2::read::BzDecoder;
use flate2::read::{GzDecoder, MultiGzDecoder};
use serde::Serialize;
use tar::Archive;
use xz2::read::XzDecoder;
use zip::ZipArchive;
use zstd::stream::read::Decoder as ZstdDecoder;

use super::tasks::{CancelGuard, CancelState};
use crate::fs_utils::{
    check_no_symlink_components, debug_log, sanitize_path_follow, sanitize_path_nofollow,
    unique_path,
};
use crate::undo::{temp_backup_path, Action, UndoState};
use tauri::Emitter;

const CHUNK: usize = 4 * 1024 * 1024;
#[derive(Debug, Clone, Copy)]
enum ArchiveKind {
    Zip,
    Tar,
    TarGz,
    TarBz2,
    TarXz,
    TarZstd,
    Gz,
    Bz2,
    Xz,
    Zstd,
}

#[derive(Default, Clone)]
struct SkipStats {
    symlinks: Arc<AtomicUsize>,
    unsupported: Arc<AtomicUsize>,
}

struct CreatedPaths {
    files: Vec<PathBuf>,
    dirs: Vec<PathBuf>,
    active: bool,
}

impl Default for CreatedPaths {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            dirs: Vec::new(),
            active: true,
        }
    }
}

impl CreatedPaths {
    fn record_file(&mut self, path: PathBuf) {
        self.files.push(path);
    }

    fn record_dir(&mut self, path: PathBuf) {
        self.dirs.push(path);
    }

    fn disarm(&mut self) {
        self.active = false;
    }
}

impl Drop for CreatedPaths {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        // Remove files first, then dirs in reverse to clean up partially extracted content.
        for file in self.files.iter().rev() {
            let _ = fs::remove_file(file);
        }
        for dir in self.dirs.iter().rev() {
            let _ = fs::remove_dir_all(dir);
        }
    }
}

impl SkipStats {
    fn skip_symlink(&self, path: &str) {
        self.symlinks.fetch_add(1, Ordering::Relaxed);
        debug_log(&format!("Skipping symlink entry while extracting: {path}"));
    }

    fn skip_unsupported(&self, path: &str, reason: &str) {
        self.unsupported.fetch_add(1, Ordering::Relaxed);
        debug_log(&format!("Skipping unsupported entry {path}: {reason}"));
    }
}

#[derive(Serialize)]
pub struct ExtractResult {
    pub destination: String,
    pub skipped_symlinks: usize,
    pub skipped_entries: usize,
}

#[derive(Serialize, Clone, Copy)]
struct ExtractProgressPayload {
    bytes: u64,
    total: u64,
    finished: bool,
}

#[derive(Clone)]
struct ProgressEmitter {
    app: tauri::AppHandle,
    event: String,
    total: u64,
    done: Arc<AtomicU64>,
    last_emit: Arc<AtomicU64>,
    last_emit_time_ms: Arc<AtomicU64>,
}

impl ProgressEmitter {
    fn new(app: tauri::AppHandle, event: String, total: u64) -> Self {
        Self {
            app,
            event,
            total,
            done: Arc::new(AtomicU64::new(0)),
            last_emit: Arc::new(AtomicU64::new(0)),
            last_emit_time_ms: Arc::new(AtomicU64::new(0)),
        }
    }

    fn add(&self, delta: u64) {
        let done = self
            .done
            .fetch_add(delta, Ordering::Relaxed)
            .saturating_add(delta);
        let last = self.last_emit.load(Ordering::Relaxed);
        let now_ms = current_millis();
        let last_time = self.last_emit_time_ms.load(Ordering::Relaxed);
        if done != last && now_ms.saturating_sub(last_time) >= 1000 {
            if self
                .last_emit
                .compare_exchange(last, done, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                let _ = self.last_emit_time_ms.compare_exchange(
                    last_time,
                    now_ms,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                );
                let _ = self.app.emit(
                    &self.event,
                    ExtractProgressPayload {
                        bytes: done,
                        total: self.total,
                        finished: false,
                    },
                );
            }
        }
    }

    fn finish(&self) {
        let done = self.done.load(Ordering::Relaxed);
        self.last_emit.store(done, Ordering::Relaxed);
        self.last_emit_time_ms
            .store(current_millis(), Ordering::Relaxed);
        let _ = self.app.emit(
            &self.event,
            ExtractProgressPayload {
                bytes: done,
                total: self.total,
                finished: true,
            },
        );
    }
}

fn is_cancelled(cancel: Option<&AtomicBool>) -> bool {
    cancel.map(|c| c.load(Ordering::Relaxed)).unwrap_or(false)
}

fn check_cancel(cancel: Option<&AtomicBool>) -> io::Result<()> {
    if is_cancelled(cancel) {
        Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"))
    } else {
        Ok(())
    }
}

fn map_copy_err(context: &str, err: io::Error) -> String {
    if err.kind() == io::ErrorKind::Interrupted {
        "Extraction cancelled".into()
    } else {
        format!("{context}: {err}")
    }
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
        do_extract(app, cancel_state, undo_state, path, progress_event)
    });
    task.await
        .map_err(|e| format!("Extraction task failed: {e}"))?
}

fn do_extract(
    app: tauri::AppHandle,
    cancel_state: CancelState,
    undo: UndoState,
    path: String,
    progress_event: Option<String>,
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
    let total_hint = match kind {
        ArchiveKind::Zip => zip_uncompressed_total(&archive_path).unwrap_or_else(|_| meta.len()),
        ArchiveKind::Tar => tar_uncompressed_total(&archive_path).unwrap_or_else(|_| meta.len()),
        ArchiveKind::TarGz => gzip_uncompressed_size(&archive_path).unwrap_or_else(|_| meta.len()),
        ArchiveKind::Gz => gzip_uncompressed_size(&archive_path).unwrap_or_else(|_| meta.len()),
        _ => meta.len(),
    }
    .max(1);
    let progress_id = progress_event.clone();
    let progress = progress_id
        .as_ref()
        .map(|evt| ProgressEmitter::new(app.clone(), evt.clone(), total_hint));
    let mut created = CreatedPaths::default();
    let cancel_guard: Option<CancelGuard> = progress_id
        .as_ref()
        .map(|evt| cancel_state.register(evt.clone()))
        .transpose()?;
    let cancel_token = cancel_guard.as_ref().map(|c| c.token());

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
                |reader| {
                    ZstdDecoder::new(reader)
                        .map(|r| Box::new(r) as Box<dyn Read>)
                        .map_err(|e| format!("Failed to create zstd decoder: {e}"))
                },
            )?;
            dest_dir
        }
        ArchiveKind::Gz => decompress_single_with_reader(
            &archive_path,
            parent,
            progress.as_ref(),
            &mut created,
            cancel_token.as_deref(),
            |reader| Ok(Box::new(MultiGzDecoder::new(reader)) as Box<dyn Read>),
        )?,
        ArchiveKind::Bz2 => decompress_single_with_reader(
            &archive_path,
            parent,
            progress.as_ref(),
            &mut created,
            cancel_token.as_deref(),
            |reader| Ok(Box::new(BzDecoder::new(reader)) as Box<dyn Read>),
        )?,
        ArchiveKind::Xz => decompress_single_with_reader(
            &archive_path,
            parent,
            progress.as_ref(),
            &mut created,
            cancel_token.as_deref(),
            |reader| Ok(Box::new(XzDecoder::new(reader)) as Box<dyn Read>),
        )?,
        ArchiveKind::Zstd => decompress_single_with_reader(
            &archive_path,
            parent,
            progress.as_ref(),
            &mut created,
            cancel_token.as_deref(),
            |reader| {
                ZstdDecoder::new(reader)
                    .map(|r| Box::new(r) as Box<dyn Read>)
                    .map_err(|e| format!("Failed to create zstd decoder: {e}"))
            },
        )?,
    };

    if let Some(p) = progress.as_ref() {
        p.finish();
    }
    created.disarm();

    let backup = temp_backup_path(&destination);
    let _ = undo.record_applied(Action::Create {
        path: destination.clone(),
        backup,
    });

    Ok(ExtractResult {
        destination: destination.to_string_lossy().into_owned(),
        skipped_symlinks: stats.symlinks.load(Ordering::Relaxed),
        skipped_entries: stats.unsupported.load(Ordering::Relaxed),
    })
}

fn map_io(action: &'static str) -> impl FnOnce(io::Error) -> String {
    move |e| format!("Failed to {action}: {e}")
}

fn archive_root_name(path: &Path) -> String {
    path.file_name()
        .and_then(|s| s.to_str())
        .map(strip_known_suffixes)
        .map(|s| if s.is_empty() { "extracted".to_string() } else { s })
        .unwrap_or_else(|| "extracted".to_string())
}

fn create_unique_dir(parent: &Path, base: &str) -> Result<PathBuf, String> {
    fs::create_dir_all(parent).map_err(map_io("ensure parent dir"))?;
    let mut candidate = parent.join(base);
    let mut idx = 1;
    loop {
        match fs::create_dir(&candidate) {
            Ok(_) => return Ok(candidate),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                candidate = parent.join(format!("{base}-{idx}"));
                idx += 1;
            }
            Err(e) => {
                return Err(format!(
                    "Failed to create destination folder {}: {e}",
                    candidate.display()
                ))
            }
        }
    }
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

fn first_component(path: &Path) -> Option<PathBuf> {
    path.components()
        .find_map(|c| match c {
            std::path::Component::Normal(p) => Some(PathBuf::from(p)),
            _ => None,
        })
}

fn single_root_in_zip(path: &Path) -> Result<Option<PathBuf>, String> {
    let mut archive = ZipArchive::new(File::open(path).map_err(map_io("open zip for root"))?)
        .map_err(|e| format!("Failed to read zip: {e}"))?;
    let mut root: Option<PathBuf> = None;
    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry {i}: {e}"))?;
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

fn single_root_in_tar(path: &Path, kind: ArchiveKind) -> Result<Option<PathBuf>, String> {
    let file = File::open(path).map_err(map_io("open tar for root"))?;
    let reader = BufReader::with_capacity(CHUNK, file);
    let reader: Box<dyn Read> = match kind {
        ArchiveKind::Tar => Box::new(reader),
        ArchiveKind::TarGz => Box::new(GzDecoder::new(reader)),
        ArchiveKind::TarBz2 => Box::new(BzDecoder::new(reader)),
        ArchiveKind::TarXz => Box::new(XzDecoder::new(reader)),
        ArchiveKind::TarZstd => Box::new(
            ZstdDecoder::new(reader)
                .map_err(|e| format!("Failed to create zstd decoder: {e}"))?,
        ),
        _ => return Ok(None),
    };
    let mut archive = Archive::new(reader);
    let mut root: Option<PathBuf> = None;
    for entry_result in archive
        .entries()
        .map_err(|e| format!("Failed to iterate tar: {e}"))?
    {
        let entry = entry_result.map_err(|e| format!("Failed to read tar entry: {e}"))?;
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

fn open_buffered_file(path: &Path, action: &'static str) -> Result<BufReader<File>, String> {
    let file = File::open(path).map_err(map_io(action))?;
    Ok(BufReader::with_capacity(CHUNK, file))
}

fn extract_tar_with_reader<F>(
    archive_path: &Path,
    dest_dir: &Path,
    strip_prefix: Option<&Path>,
    stats: &SkipStats,
    progress: Option<&ProgressEmitter>,
    created: &mut CreatedPaths,
    cancel: Option<&AtomicBool>,
    wrap: F,
) -> Result<(), String>
where
    F: FnOnce(BufReader<File>) -> Result<Box<dyn Read>, String>,
{
    let reader = open_buffered_file(archive_path, "open tar")?;
    let reader = wrap(reader)?;
    extract_tar(reader, dest_dir, strip_prefix, stats, progress, created, cancel)?;
    Ok(())
}

fn decompress_single_with_reader<F>(
    archive_path: &Path,
    parent: &Path,
    progress: Option<&ProgressEmitter>,
    created: &mut CreatedPaths,
    cancel: Option<&AtomicBool>,
    wrap: F,
) -> Result<PathBuf, String>
where
    F: FnOnce(BufReader<File>) -> Result<Box<dyn Read>, String>,
{
    let reader = open_buffered_file(archive_path, "open compressed file")?;
    let reader = wrap(reader)?;
    decompress_single(reader, archive_path, parent, progress, created, cancel)
}

fn strip_known_suffixes(name: &str) -> String {
    let lower = name.to_lowercase();
    for suffix in [
        ".tar.gz", ".tgz", ".tar.bz2", ".tbz2", ".tar.xz", ".txz", ".tar.zst", ".tzst", ".tar",
    ] {
        if lower.ends_with(suffix) && name.len() > suffix.len() {
            return name[..name.len() - suffix.len()].to_string();
        }
    }
    if let Some((stem, _ext)) = name.rsplit_once('.') {
        if !stem.is_empty() {
            return stem.to_string();
        }
    }
    name.to_string()
}

fn extract_zip(
    path: &Path,
    dest_dir: &Path,
    strip_prefix: Option<&Path>,
    stats: &SkipStats,
    progress: Option<&ProgressEmitter>,
    created: &mut CreatedPaths,
    cancel: Option<&AtomicBool>,
) -> Result<(), String> {
    let mut archive = ZipArchive::new(File::open(path).map_err(map_io("open zip"))?)
        .map_err(|e| format!("Failed to read zip: {e}"))?;
    let mut buf = vec![0u8; CHUNK];

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry {i}: {e}"))?;
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
                stats.skip_unsupported(&raw_name, &err);
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
            if !dest_path.exists() {
                if let Err(e) = fs::create_dir_all(&dest_path) {
                    stats.skip_unsupported(&raw_name, &format!("create dir failed: {e}"));
                    continue;
                }
                created.record_dir(dest_path.clone());
            } else if let Err(e) = fs::create_dir_all(&dest_path) {
                stats.skip_unsupported(&raw_name, &format!("create dir failed: {e}"));
            }
            continue;
        }
        if dest_path.exists() {
            if let Some(p) = progress {
                // Approximate progress for skipped existing file using compressed size.
                let inc = entry.compressed_size().max(1);
                p.add(inc);
            }
            continue;
        }
        if let Some(parent) = dest_path.parent() {
            if !parent.exists() {
                if let Err(e) = fs::create_dir_all(parent) {
                    stats.skip_unsupported(&raw_name, &format!("create parent failed: {e}"));
                    continue;
                }
                created.record_dir(parent.to_path_buf());
            } else if let Err(e) = fs::create_dir_all(parent) {
                stats.skip_unsupported(&raw_name, &format!("create parent failed: {e}"));
                continue;
            }
        }
        let (file, _) = open_unique_file(&dest_path)?;
        created.record_file(dest_path.clone());
        let mut out = BufWriter::with_capacity(CHUNK, file);
        if let Err(e) = copy_with_progress(&mut entry, &mut out, progress, cancel, &mut buf) {
            let msg = map_copy_err(&format!("Failed to write zip entry {raw_name}"), e);
            return Err(msg);
        }
    }

    Ok(())
}

fn extract_tar<R: Read>(
    reader: R,
    dest_dir: &Path,
    strip_prefix: Option<&Path>,
    stats: &SkipStats,
    progress: Option<&ProgressEmitter>,
    created: &mut CreatedPaths,
    cancel: Option<&AtomicBool>,
) -> Result<(), String> {
    let mut archive = Archive::new(reader);
    let mut buf = vec![0u8; CHUNK];
    for entry_result in archive
        .entries()
        .map_err(|e| format!("Failed to iterate tar: {e}"))?
    {
        check_cancel(cancel).map_err(|e| map_copy_err("Extraction cancelled", e))?;
        let mut entry = entry_result.map_err(|e| format!("Failed to read tar entry: {e}"))?;
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
        copy_with_progress(&mut entry, &mut out, progress, cancel, &mut buf)
            .map_err(|e| map_copy_err("write tar entry", e))?;
    }
    Ok(())
}

fn decompress_single<R: Read>(
    mut reader: R,
    archive: &Path,
    parent: &Path,
    progress: Option<&ProgressEmitter>,
    created: &mut CreatedPaths,
    cancel: Option<&AtomicBool>,
) -> Result<PathBuf, String> {
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
        if !parent_dir.exists() {
            fs::create_dir_all(parent_dir).map_err(map_io("create output dir"))?;
            created.record_dir(parent_dir.to_path_buf());
        }
    }
    let (file, dest_path) = open_unique_file(&dest_path)?;
    created.record_file(dest_path.clone());
    let mut out = BufWriter::with_capacity(CHUNK, file);
    let mut buf = vec![0u8; CHUNK];
    copy_with_progress(&mut reader, &mut out, progress, cancel, &mut buf)
        .map_err(|e| map_copy_err("write decompressed file", e))?;
    Ok(dest_path)
}

fn clean_relative_path(path: &Path) -> Result<PathBuf, String> {
    let mut cleaned = PathBuf::new();
    for comp in path.components() {
        match comp {
            std::path::Component::Normal(p) => cleaned.push(p),
            std::path::Component::CurDir => {}
            _ => return Err("Refusing path with traversal or absolute components".into()),
        }
    }
    Ok(cleaned)
}

fn copy_with_progress<R: Read, W: Write>(
    mut reader: R,
    mut writer: W,
    progress: Option<&ProgressEmitter>,
    cancel: Option<&AtomicBool>,
    buf: &mut [u8],
) -> io::Result<u64> {
    let mut written: u64 = 0;
    loop {
        check_cancel(cancel)?;
        let n = reader.read(buf)?;
        if n == 0 {
            break;
        }
        writer.write_all(&buf[..n])?;
        written = written.saturating_add(n as u64);
        if let Some(p) = progress {
            p.add(n as u64);
        }
    }
    Ok(written)
}

fn open_unique_file(dest_path: &Path) -> Result<(File, PathBuf), String> {
    let mut candidate = dest_path.to_path_buf();
    loop {
        match File::options()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(f) => return Ok((f, candidate)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                candidate = unique_path(&candidate);
                continue;
            }
            Err(e) => {
                return Err(format!(
                    "Failed to create file {}: {e}",
                    candidate.display()
                ))
            }
        }
    }
}

fn current_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn zip_uncompressed_total(path: &Path) -> Result<u64, String> {
    let mut archive = ZipArchive::new(File::open(path).map_err(map_io("open zip for total"))?)
        .map_err(|e| format!("Failed to read zip: {e}"))?;
    let mut total = 0u64;
    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry {i}: {e}"))?;
        if entry.is_dir() {
            continue;
        }
        total = total.saturating_add(entry.size());
    }
    Ok(total)
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

fn tar_uncompressed_total(path: &Path) -> Result<u64, String> {
    let file = File::open(path).map_err(map_io("open tar for total"))?;
    let reader = BufReader::with_capacity(CHUNK, file);
    let mut archive = Archive::new(reader);
    let mut total = 0u64;
    let entries = archive
        .entries()
        .map_err(|e| format!("Failed to iterate tar for total: {e}"))?;
    for entry_result in entries {
        let entry = entry_result.map_err(|e| format!("Failed to read tar entry for total: {e}"))?;
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
